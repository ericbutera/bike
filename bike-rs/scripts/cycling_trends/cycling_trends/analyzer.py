#!/usr/bin/env python3
"""
analyze_fit_v2.py

Endurance MTB FIT analyzer with:
- Ride summary
- HR zones
- Hour-by-hour durability metrics
- First-half vs second-half aerobic decoupling
- Valley-to-crest climb detection
- Climb pacing consistency
- Post-summit HR recovery
- Coasting and stop-time estimates
- Late-ride fade
- Marji-specific readiness heuristics
- JSON, CSV, and human-readable text outputs

Install:
    python3 -m pip install fitparse

Run:
    python3 analyze_fit_v2.py activity.fit --lthr 170
    python3 analyze_fit_v2.py activity.fit --max-hr 185
    python3 analyze_fit_v2.py activity.fit --lthr 170 --output ride_report

Output:
    ride_report.json

The JSON contains four top-level sections:
    1. ride_summary
    2. hourly_durability
    3. climb_analysis
    4. marji_readiness

Notes:
- Heart-rate zones are most useful when --lthr is accurate.
- Power/cadence metrics are included only if present in the FIT file.
- MTB speed-based decoupling is terrain-sensitive. Hourly comparisons and
  climb-specific metrics are usually more useful than a single ride-wide number.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import statistics
import sys
from dataclasses import asdict, dataclass
from datetime import datetime
from pathlib import Path
from typing import Any, Iterable, Optional

try:
    from fitparse import FitFile
except ImportError:
    print(
        "Missing dependency: fitparse\n"
        "Install it with:\n"
        "  python3 -m pip install fitparse",
        file=sys.stderr,
    )
    raise SystemExit(2)


# ---------------------------------------------------------------------------
# Data models
# ---------------------------------------------------------------------------

@dataclass
class Point:
    timestamp: datetime
    elapsed_s: float
    lat: Optional[float]
    lon: Optional[float]
    altitude_m: Optional[float]
    distance_m: Optional[float]
    speed_mps: Optional[float]
    heart_rate: Optional[int]
    cadence: Optional[int]
    power: Optional[int]


@dataclass
class Climb:
    number: int
    start_s: float
    summit_s: float
    duration_s: float
    distance_m: float
    gain_m: float
    avg_grade_pct: Optional[float]
    vertical_rate_m_per_h: float
    avg_speed_kph: Optional[float]
    avg_hr: Optional[float]
    peak_hr: Optional[int]
    avg_cadence: Optional[float]
    avg_power: Optional[float]
    recovery_reference_hr: Optional[float]
    hr_30s_post: Optional[float]
    hr_60s_post: Optional[float]
    recovery_30s_bpm: Optional[float]
    recovery_60s_bpm: Optional[float]
    seconds_to_drop_10_bpm: Optional[float]
    seconds_to_drop_15_bpm: Optional[float]
    seconds_to_zone2: Optional[float]
    immediate_descent: bool
    first_or_second_half: str


@dataclass
class Hourly:
    hour: int
    elapsed_start_s: float
    elapsed_end_s: float
    distance_km: Optional[float]
    avg_speed_kph: Optional[float]
    avg_hr: Optional[float]
    max_hr: Optional[int]
    avg_cadence: Optional[float]
    avg_power: Optional[float]
    ascent_m: float
    moving_minutes: float
    stopped_minutes: float
    coasting_minutes: float
    efficiency_mps_per_bpm: Optional[float]


# ---------------------------------------------------------------------------
# Basic helpers
# ---------------------------------------------------------------------------

def finite(value: Any) -> bool:
    try:
        return value is not None and math.isfinite(float(value))
    except (TypeError, ValueError):
        return False


def safe_mean(values: Iterable[Optional[float]]) -> Optional[float]:
    vals = [float(v) for v in values if finite(v)]
    return statistics.fmean(vals) if vals else None


def safe_median(values: Iterable[Optional[float]]) -> Optional[float]:
    vals = [float(v) for v in values if finite(v)]
    return statistics.median(vals) if vals else None


def safe_max(values: Iterable[Optional[float]]) -> Optional[float]:
    vals = [float(v) for v in values if finite(v)]
    return max(vals) if vals else None


def percent_change(late: Optional[float], early: Optional[float]) -> Optional[float]:
    if late is None or early is None or early == 0:
        return None
    return 100.0 * (late - early) / early


def semicircles_to_degrees(value: Any) -> Optional[float]:
    if value is None:
        return None
    try:
        return float(value) * (180.0 / 2**31)
    except (TypeError, ValueError):
        return None


def field_value(message: Any, *names: str) -> Any:
    for name in names:
        value = message.get_value(name)
        if value is not None:
            return value
    return None


def round_nested(value: Any) -> Any:
    if isinstance(value, float):
        return round(value, 2) if math.isfinite(value) else None
    if isinstance(value, dict):
        return {k: round_nested(v) for k, v in value.items()}
    if isinstance(value, list):
        return [round_nested(v) for v in value]
    return value


# ---------------------------------------------------------------------------
# FIT parsing
# ---------------------------------------------------------------------------

def parse_fit(path: Path) -> tuple[list[Point], dict[str, Any]]:
    fit = FitFile(str(path))
    raw: list[dict[str, Any]] = []

    for message in fit.get_messages("record"):
        timestamp = field_value(message, "timestamp")
        if not isinstance(timestamp, datetime):
            continue

        distance = field_value(message, "distance")
        speed = field_value(message, "enhanced_speed", "speed")
        altitude = field_value(message, "enhanced_altitude", "altitude")
        hr = field_value(message, "heart_rate")
        cadence = field_value(message, "cadence")
        power = field_value(message, "power")

        raw.append(
            {
                "timestamp": timestamp,
                "lat": semicircles_to_degrees(field_value(message, "position_lat")),
                "lon": semicircles_to_degrees(field_value(message, "position_long")),
                "altitude_m": float(altitude) if altitude is not None else None,
                "distance_m": float(distance) if distance is not None else None,
                "speed_mps": float(speed) if speed is not None else None,
                "heart_rate": int(hr) if hr is not None else None,
                "cadence": int(cadence) if cadence is not None else None,
                "power": int(power) if power is not None else None,
            }
        )

    if not raw:
        raise ValueError("No FIT record messages were found.")

    raw.sort(key=lambda r: r["timestamp"])
    start = raw[0]["timestamp"]

    points = [
        Point(
            timestamp=r["timestamp"],
            elapsed_s=(r["timestamp"] - start).total_seconds(),
            lat=r["lat"],
            lon=r["lon"],
            altitude_m=r["altitude_m"],
            distance_m=r["distance_m"],
            speed_mps=r["speed_mps"],
            heart_rate=r["heart_rate"],
            cadence=r["cadence"],
            power=r["power"],
        )
        for r in raw
    ]

    session_data: dict[str, Any] = {}
    for message in fit.get_messages("session"):
        for name in (
            "total_elapsed_time",
            "total_timer_time",
            "total_distance",
            "total_ascent",
            "total_descent",
            "avg_speed",
            "max_speed",
            "avg_heart_rate",
            "max_heart_rate",
            "avg_cadence",
            "max_cadence",
            "avg_power",
            "max_power",
            "normalized_power",
            "training_stress_score",
        ):
            value = message.get_value(name)
            if value is not None:
                session_data[name] = value
        break

    return points, session_data


# ---------------------------------------------------------------------------
# Time-series helpers
# ---------------------------------------------------------------------------

def interpolate_missing(values: list[Optional[float]]) -> list[Optional[float]]:
    result = values[:]
    valid = [i for i, v in enumerate(result) if v is not None]
    if not valid:
        return result

    first, last = valid[0], valid[-1]
    for i in range(first):
        result[i] = result[first]
    for i in range(last + 1, len(result)):
        result[i] = result[last]

    prev = first
    for nxt in valid[1:]:
        if nxt - prev > 1:
            a = float(result[prev])
            b = float(result[nxt])
            for i in range(prev + 1, nxt):
                f = (i - prev) / (nxt - prev)
                result[i] = a + (b - a) * f
        prev = nxt

    return result


def rolling_median_by_time(
    points: list[Point],
    attr: str,
    window_s: float,
) -> list[Optional[float]]:
    values = interpolate_missing([getattr(p, attr) for p in points])
    output: list[Optional[float]] = []
    left = 0

    for right, point in enumerate(points):
        while left < right and points[left].elapsed_s < point.elapsed_s - window_s:
            left += 1

        window = [
            float(v)
            for v in values[left : right + 1]
            if v is not None and finite(v)
        ]
        output.append(statistics.median(window) if window else None)

    return output


def segment_points(points: list[Point], start_s: float, end_s: float) -> list[Point]:
    return [p for p in points if start_s <= p.elapsed_s <= end_s]


def time_weighted_mean(points: list[Point], attr: str) -> Optional[float]:
    numerator = 0.0
    denominator = 0.0

    for a, b in zip(points, points[1:]):
        value = getattr(a, attr)
        dt = (b.timestamp - a.timestamp).total_seconds()
        if value is not None and 0 < dt <= 30:
            numerator += float(value) * dt
            denominator += dt

    return numerator / denominator if denominator else None


def segment_distance(points: list[Point]) -> Optional[float]:
    distances = [p.distance_m for p in points if p.distance_m is not None]
    if len(distances) >= 2:
        return max(0.0, distances[-1] - distances[0])

    total = 0.0
    found = False
    for a, b in zip(points, points[1:]):
        dt = (b.timestamp - a.timestamp).total_seconds()
        if a.speed_mps is not None and 0 < dt <= 30:
            total += a.speed_mps * dt
            found = True
    return total if found else None


def positive_gain(altitudes: list[Optional[float]], noise_threshold_m: float = 1.0) -> float:
    gain = 0.0
    previous = None

    for altitude in altitudes:
        if altitude is None:
            continue
        altitude = float(altitude)
        if previous is not None:
            delta = altitude - previous
            if delta >= noise_threshold_m:
                gain += delta
        previous = altitude

    return gain


def moving_points(points: list[Point], min_speed_mps: float = 0.5) -> list[Point]:
    return [
        p
        for p in points
        if p.speed_mps is None or p.speed_mps >= min_speed_mps
    ]


def efficiency(points: list[Point]) -> Optional[float]:
    active = moving_points(points)
    speed = time_weighted_mean(active, "speed_mps")
    hr = time_weighted_mean(active, "heart_rate")
    if speed is None or hr is None or hr <= 0:
        return None
    return speed / hr


def activity_time_breakdown(points: list[Point]) -> dict[str, float]:
    moving = 0.0
    stopped = 0.0
    coasting = 0.0

    for a, b in zip(points, points[1:]):
        dt = (b.timestamp - a.timestamp).total_seconds()
        if not (0 < dt <= 30):
            continue

        speed = a.speed_mps or 0.0
        cadence = a.cadence

        if speed < 0.5:
            stopped += dt
        else:
            moving += dt
            if cadence is not None and cadence == 0 and speed >= 2.0:
                coasting += dt

    return {
        "moving_s": moving,
        "stopped_s": stopped,
        "coasting_s": coasting,
    }


# ---------------------------------------------------------------------------
# HR zones and recovery
# ---------------------------------------------------------------------------

def hr_zone_bounds(
    max_hr: Optional[int],
    lthr: Optional[int],
) -> tuple[str, list[tuple[str, float, float]]]:
    if lthr:
        return (
            "LTHR",
            [
                ("Z1", 0.00 * lthr, 0.81 * lthr),
                ("Z2", 0.81 * lthr, 0.89 * lthr),
                ("Z3", 0.89 * lthr, 0.94 * lthr),
                ("Z4", 0.94 * lthr, 1.00 * lthr),
                ("Z5", 1.00 * lthr, float("inf")),
            ],
        )

    if max_hr:
        return (
            "Max HR",
            [
                ("Z1", 0.00 * max_hr, 0.60 * max_hr),
                ("Z2", 0.60 * max_hr, 0.70 * max_hr),
                ("Z3", 0.70 * max_hr, 0.80 * max_hr),
                ("Z4", 0.80 * max_hr, 0.90 * max_hr),
                ("Z5", 0.90 * max_hr, float("inf")),
            ],
        )

    return ("Unavailable", [])


def zone2_ceiling(max_hr: Optional[int], lthr: Optional[int]) -> Optional[float]:
    if lthr:
        return 0.89 * lthr
    if max_hr:
        return 0.70 * max_hr
    return None


def zone_time(
    points: list[Point],
    max_hr: Optional[int],
    lthr: Optional[int],
) -> dict[str, Any]:
    method, bounds = hr_zone_bounds(max_hr, lthr)
    if not bounds:
        return {"method": method, "seconds": {}, "percent": {}}

    seconds = {name: 0.0 for name, _, _ in bounds}
    total = 0.0

    for a, b in zip(points, points[1:]):
        if a.heart_rate is None:
            continue

        dt = (b.timestamp - a.timestamp).total_seconds()
        if not (0 < dt <= 30):
            continue

        total += dt
        for name, low, high in bounds:
            if low <= a.heart_rate < high:
                seconds[name] += dt
                break

    return {
        "method": method,
        "seconds": seconds,
        "percent": {
            name: 100.0 * value / total if total else 0.0
            for name, value in seconds.items()
        },
    }


def mean_hr_in_window(
    points: list[Point],
    start_s: float,
    end_s: float,
) -> Optional[float]:
    vals = [
        p.heart_rate
        for p in points
        if p.heart_rate is not None and start_s <= p.elapsed_s <= end_s
    ]
    return safe_mean(vals)


# ---------------------------------------------------------------------------
# Better climb detection
# ---------------------------------------------------------------------------

def detect_valley_to_crest_climbs(
    points: list[Point],
    smoothed_alt: list[Optional[float]],
    max_hr: Optional[int],
    lthr: Optional[int],
    min_gain_m: float = 20.0,
    min_duration_s: float = 90.0,
    min_distance_m: float = 300.0,
    summit_confirmation_drop_m: float = 5.0,
    valley_reset_drop_m: float = 8.0,
) -> list[Climb]:
    """
    Detects climbs from a local valley to a confirmed crest.

    The detector:
    - tracks the lowest altitude since the prior confirmed descent,
    - tracks the highest altitude after that valley,
    - confirms the summit only after altitude falls enough,
    - requires minimum gain, duration, and distance.

    This is still heuristic, but it avoids ending a climb before the crest and
    reduces nonsensical negative HR-recovery values.
    """
    climbs: list[Climb] = []
    n = len(points)
    z2_top = zone2_ceiling(max_hr, lthr)
    total_elapsed = points[-1].elapsed_s

    i = 0
    while i < n - 1:
        while i < n and smoothed_alt[i] is None:
            i += 1
        if i >= n - 1:
            break

        valley_i = i
        valley_alt = float(smoothed_alt[i])
        peak_i = i
        peak_alt = valley_alt
        j = i + 1
        confirmed = False

        while j < n:
            alt = smoothed_alt[j]
            if alt is None:
                j += 1
                continue
            alt = float(alt)

            if alt < valley_alt:
                valley_alt = alt
                valley_i = j
                peak_alt = alt
                peak_i = j

            if alt > peak_alt:
                peak_alt = alt
                peak_i = j

            gain = peak_alt - valley_alt
            duration = points[peak_i].elapsed_s - points[valley_i].elapsed_s
            candidate = points[valley_i : peak_i + 1]
            distance = segment_distance(candidate) or 0.0

            if (
                gain >= min_gain_m
                and duration >= min_duration_s
                and distance >= min_distance_m
                and peak_alt - alt >= summit_confirmation_drop_m
            ):
                confirmed = True
                break

            # If we descend materially below the active rise before it qualifies,
            # reset the valley candidate.
            if peak_alt - alt >= valley_reset_drop_m and gain < min_gain_m:
                valley_i = j
                valley_alt = alt
                peak_i = j
                peak_alt = alt

            j += 1

        if not confirmed:
            break

        segment = points[valley_i : peak_i + 1]
        distance = segment_distance(segment) or 0.0
        gain = peak_alt - valley_alt
        duration = points[peak_i].elapsed_s - points[valley_i].elapsed_s
        avg_grade = 100.0 * gain / distance if distance > 0 else None
        vertical_rate = 3600.0 * gain / duration if duration > 0 else 0.0

        avg_speed = time_weighted_mean(moving_points(segment), "speed_mps")
        avg_hr = time_weighted_mean(segment, "heart_rate")
        peak_hr_raw = safe_max(p.heart_rate for p in segment)
        peak_hr = int(round(peak_hr_raw)) if peak_hr_raw is not None else None
        avg_cadence = time_weighted_mean(segment, "cadence")
        avg_power = time_weighted_mean(segment, "power")

        summit_s = points[peak_i].elapsed_s

        # Use the last 20 seconds of the climb as the reference HR, not a single
        # summit sample, to reduce sensor lag and random noise.
        recovery_reference_hr = mean_hr_in_window(
            points,
            max(points[valley_i].elapsed_s, summit_s - 20),
            summit_s,
        )

        hr_30 = mean_hr_in_window(points, summit_s + 25, summit_s + 35)
        hr_60 = mean_hr_in_window(points, summit_s + 55, summit_s + 65)

        recovery_30 = (
            recovery_reference_hr - hr_30
            if recovery_reference_hr is not None and hr_30 is not None
            else None
        )
        recovery_60 = (
            recovery_reference_hr - hr_60
            if recovery_reference_hr is not None and hr_60 is not None
            else None
        )

        post = [
            p
            for p in points[peak_i:]
            if p.elapsed_s <= summit_s + 300
        ]

        seconds_to_drop_10 = None
        seconds_to_drop_15 = None
        seconds_to_z2 = None

        for p in post:
            if p.heart_rate is None:
                continue
            after = p.elapsed_s - summit_s

            if (
                seconds_to_drop_10 is None
                and recovery_reference_hr is not None
                and p.heart_rate <= recovery_reference_hr - 10
            ):
                seconds_to_drop_10 = after

            if (
                seconds_to_drop_15 is None
                and recovery_reference_hr is not None
                and p.heart_rate <= recovery_reference_hr - 15
            ):
                seconds_to_drop_15 = after

            if (
                seconds_to_z2 is None
                and z2_top is not None
                and p.heart_rate <= z2_top
            ):
                seconds_to_z2 = after

        immediate_descent = False
        descent_window = [
            idx
            for idx in range(peak_i, min(n, peak_i + 120))
            if smoothed_alt[idx] is not None
            and points[idx].elapsed_s <= summit_s + 60
        ]
        if descent_window:
            lowest = min(float(smoothed_alt[idx]) for idx in descent_window)
            immediate_descent = peak_alt - lowest >= 5.0

        climbs.append(
            Climb(
                number=len(climbs) + 1,
                start_s=points[valley_i].elapsed_s,
                summit_s=summit_s,
                duration_s=duration,
                distance_m=distance,
                gain_m=gain,
                avg_grade_pct=avg_grade,
                vertical_rate_m_per_h=vertical_rate,
                avg_speed_kph=avg_speed * 3.6 if avg_speed is not None else None,
                avg_hr=avg_hr,
                peak_hr=peak_hr,
                avg_cadence=avg_cadence,
                avg_power=avg_power,
                recovery_reference_hr=recovery_reference_hr,
                hr_30s_post=hr_30,
                hr_60s_post=hr_60,
                recovery_30s_bpm=recovery_30,
                recovery_60s_bpm=recovery_60,
                seconds_to_drop_10_bpm=seconds_to_drop_10,
                seconds_to_drop_15_bpm=seconds_to_drop_15,
                seconds_to_zone2=seconds_to_z2,
                immediate_descent=immediate_descent,
                first_or_second_half=(
                    "first" if summit_s < total_elapsed / 2 else "second"
                ),
            )
        )

        i = max(j, peak_i + 1)

    return climbs


# ---------------------------------------------------------------------------
# Hourly and durability analysis
# ---------------------------------------------------------------------------

def hourly_metrics(
    points: list[Point],
    smoothed_alt: list[Optional[float]],
) -> list[Hourly]:
    total = points[-1].elapsed_s
    hours = math.ceil(total / 3600)
    rows: list[Hourly] = []

    for hour in range(1, hours + 1):
        start_s = (hour - 1) * 3600
        end_s = min(hour * 3600, total)
        segment = segment_points(points, start_s, end_s)

        if len(segment) < 2:
            continue

        distance = segment_distance(segment)
        active = moving_points(segment)
        avg_speed = time_weighted_mean(active, "speed_mps")
        avg_hr = time_weighted_mean(active, "heart_rate")
        max_hr_raw = safe_max(p.heart_rate for p in segment)
        avg_cadence = time_weighted_mean(active, "cadence")
        avg_power = time_weighted_mean(active, "power")

        indices = [
            i
            for i, p in enumerate(points)
            if start_s <= p.elapsed_s <= end_s
        ]
        ascent = positive_gain(
            [smoothed_alt[i] for i in indices],
            noise_threshold_m=1.0,
        )

        breakdown = activity_time_breakdown(segment)
        eff = None
        if avg_speed is not None and avg_hr is not None and avg_hr > 0:
            eff = avg_speed / avg_hr

        rows.append(
            Hourly(
                hour=hour,
                elapsed_start_s=start_s,
                elapsed_end_s=end_s,
                distance_km=distance / 1000.0 if distance is not None else None,
                avg_speed_kph=avg_speed * 3.6 if avg_speed is not None else None,
                avg_hr=avg_hr,
                max_hr=int(round(max_hr_raw)) if max_hr_raw is not None else None,
                avg_cadence=avg_cadence,
                avg_power=avg_power,
                ascent_m=ascent,
                moving_minutes=breakdown["moving_s"] / 60.0,
                stopped_minutes=breakdown["stopped_s"] / 60.0,
                coasting_minutes=breakdown["coasting_s"] / 60.0,
                efficiency_mps_per_bpm=eff,
            )
        )

    return rows


def first_second_half(points: list[Point]) -> dict[str, Optional[float]]:
    total = points[-1].elapsed_s
    first = segment_points(points, 0, total / 2)
    second = segment_points(points, total / 2, total)

    first_eff = efficiency(first)
    second_eff = efficiency(second)
    decoupling = None
    if first_eff and second_eff:
        decoupling = 100.0 * (first_eff - second_eff) / first_eff

    first_speed = time_weighted_mean(moving_points(first), "speed_mps")
    second_speed = time_weighted_mean(moving_points(second), "speed_mps")

    return {
        "first_half_speed_kph": first_speed * 3.6 if first_speed is not None else None,
        "second_half_speed_kph": second_speed * 3.6 if second_speed is not None else None,
        "first_half_avg_hr": time_weighted_mean(moving_points(first), "heart_rate"),
        "second_half_avg_hr": time_weighted_mean(moving_points(second), "heart_rate"),
        "first_half_efficiency_mps_per_bpm": first_eff,
        "second_half_efficiency_mps_per_bpm": second_eff,
        "aerobic_decoupling_pct": decoupling,
    }


def late_ride_metrics(points: list[Point]) -> dict[str, Optional[float]]:
    total = points[-1].elapsed_s
    early = segment_points(points, total * 0.10, total * 0.35)
    late = segment_points(points, total * 0.75, total)

    def metric(segment: list[Point], attr: str) -> Optional[float]:
        return time_weighted_mean(moving_points(segment), attr)

    early_speed = metric(early, "speed_mps")
    late_speed = metric(late, "speed_mps")
    early_hr = metric(early, "heart_rate")
    late_hr = metric(late, "heart_rate")
    early_cadence = metric(early, "cadence")
    late_cadence = metric(late, "cadence")
    early_power = metric(early, "power")
    late_power = metric(late, "power")

    return {
        "early_speed_kph": early_speed * 3.6 if early_speed is not None else None,
        "late_speed_kph": late_speed * 3.6 if late_speed is not None else None,
        "speed_change_pct": percent_change(late_speed, early_speed),
        "early_avg_hr": early_hr,
        "late_avg_hr": late_hr,
        "hr_change_pct": percent_change(late_hr, early_hr),
        "early_avg_cadence": early_cadence,
        "late_avg_cadence": late_cadence,
        "cadence_change_pct": percent_change(late_cadence, early_cadence),
        "early_avg_power": early_power,
        "late_avg_power": late_power,
        "power_change_pct": percent_change(late_power, early_power),
    }


def climb_consistency(climbs: list[Climb]) -> dict[str, Any]:
    first = [c for c in climbs if c.first_or_second_half == "first"]
    second = [c for c in climbs if c.first_or_second_half == "second"]

    def summarize(group: list[Climb]) -> dict[str, Optional[float]]:
        return {
            "count": len(group),
            "median_vertical_rate_m_per_h": safe_median(
                c.vertical_rate_m_per_h for c in group
            ),
            "median_avg_hr": safe_median(c.avg_hr for c in group),
            "median_avg_speed_kph": safe_median(c.avg_speed_kph for c in group),
            "median_recovery_60s_bpm": safe_median(
                c.recovery_60s_bpm
                for c in group
                if c.recovery_60s_bpm is not None and c.recovery_60s_bpm >= 0
            ),
            "median_seconds_to_drop_10_bpm": safe_median(
                c.seconds_to_drop_10_bpm for c in group
            ),
        }

    first_summary = summarize(first)
    second_summary = summarize(second)

    return {
        "first_half": first_summary,
        "second_half": second_summary,
        "vertical_rate_change_pct": percent_change(
            second_summary["median_vertical_rate_m_per_h"],
            first_summary["median_vertical_rate_m_per_h"],
        ),
        "climb_hr_change_pct": percent_change(
            second_summary["median_avg_hr"],
            first_summary["median_avg_hr"],
        ),
        "recovery_60s_change_pct": percent_change(
            second_summary["median_recovery_60s_bpm"],
            first_summary["median_recovery_60s_bpm"],
        ),
    }


# ---------------------------------------------------------------------------
# Readiness heuristics
# ---------------------------------------------------------------------------

def score_band(value: Optional[float], bands: list[tuple[float, int]]) -> Optional[int]:
    if value is None:
        return None
    for threshold, score in bands:
        if value <= threshold:
            return score
    return bands[-1][1]


def marji_readiness(
    summary: dict[str, Any],
    decoupling: dict[str, Any],
    late: dict[str, Any],
    climb_compare: dict[str, Any],
    zones: dict[str, Any],
    time_breakdown: dict[str, float],
) -> dict[str, Any]:
    checks: list[dict[str, Any]] = []

    dec = decoupling.get("aerobic_decoupling_pct")
    if dec is not None:
        if dec < 5:
            status, note = "strong", "Ride-wide aerobic durability was strong."
        elif dec < 8:
            status, note = "acceptable", "Aerobic durability was adequate but not exceptional."
        else:
            status, note = "focus", "Aerobic drift was high enough to investigate pacing, heat, hydration, or fueling."
        checks.append({"metric": "aerobic_decoupling", "value": dec, "status": status, "note": note})

    speed_fade = late.get("speed_change_pct")
    hr_change = late.get("hr_change_pct")
    if speed_fade is not None:
        if speed_fade >= -8:
            status = "strong"
        elif speed_fade >= -15:
            status = "acceptable"
        else:
            status = "focus"

        note = "Late speed held reasonably well."
        if status == "acceptable":
            note = "Late speed faded moderately."
        elif status == "focus":
            note = "Late speed faded substantially."

        if hr_change is not None and hr_change < -3:
            note += " Falling HR suggests reduced output or conservative pacing rather than cardiovascular drift."

        checks.append({
            "metric": "late_ride_speed_change_pct",
            "value": speed_fade,
            "status": status,
            "note": note,
        })

    climb_fade = climb_compare.get("vertical_rate_change_pct")
    if climb_fade is not None:
        if climb_fade >= -8:
            status, note = "strong", "Climbing rate remained stable between ride halves."
        elif climb_fade >= -15:
            status, note = "acceptable", "Climbing rate declined moderately in the second half."
        else:
            status, note = "focus", "Second-half climbing rate declined materially."
        checks.append({
            "metric": "climb_vertical_rate_change_pct",
            "value": climb_fade,
            "status": status,
            "note": note,
        })

    z4 = zones.get("percent", {}).get("Z4")
    z5 = zones.get("percent", {}).get("Z5")
    if z4 is not None and z5 is not None:
        high = z4 + z5
        if high <= 10:
            status, note = "strong", "High-intensity exposure was restrained."
        elif high <= 20:
            status, note = "acceptable", "High-intensity exposure was meaningful but controlled."
        else:
            status, note = "focus", "A large share of the ride was above threshold-oriented endurance intensity."
        checks.append({
            "metric": "z4_plus_z5_pct",
            "value": high,
            "status": status,
            "note": note,
        })

    elapsed_h = summary.get("elapsed_hours")
    if elapsed_h is not None:
        if elapsed_h >= 9:
            status, note = "strong", "The ride demonstrated long-duration durability."
        elif elapsed_h >= 6:
            status, note = "acceptable", "Duration was useful but shorter than a full Marji-specific field test."
        else:
            status, note = "focus", "Duration was too short to assess all-day durability."
        checks.append({
            "metric": "elapsed_hours",
            "value": elapsed_h,
            "status": status,
            "note": note,
        })

    stopped_pct = None
    total_considered = time_breakdown["moving_s"] + time_breakdown["stopped_s"]
    if total_considered > 0:
        stopped_pct = 100.0 * time_breakdown["stopped_s"] / total_considered
        if stopped_pct <= 3:
            status, note = "strong", "Stopped time was low."
        elif stopped_pct <= 8:
            status, note = "acceptable", "Stopped time was moderate."
        else:
            status, note = "focus", "Stopped time was high enough to affect race execution."
        checks.append({
            "metric": "stopped_time_pct",
            "value": stopped_pct,
            "status": status,
            "note": note,
        })

    counts = {"strong": 0, "acceptable": 0, "focus": 0}
    for check in checks:
        counts[check["status"]] += 1

    return {
        "checks": checks,
        "counts": counts,
        "important_limitations": [
            "Speed-based endurance metrics are affected by terrain, traffic, stops, and trail difficulty.",
            "Heart-rate recovery after MTB climbs is affected by immediate descents and continued pedaling.",
            "This is a heuristic readiness screen, not a physiological test or race-time prediction.",
        ],
    }


# ---------------------------------------------------------------------------
# Report construction
# ---------------------------------------------------------------------------

def build_report(
    fit_path: Path,
    points: list[Point],
    session: dict[str, Any],
    max_hr: Optional[int],
    lthr: Optional[int],
) -> tuple[dict[str, Any], list[Climb], list[Hourly]]:
    smooth_alt = rolling_median_by_time(points, "altitude_m", window_s=20.0)

    elapsed_s = points[-1].elapsed_s
    distance_m = (
        float(session["total_distance"])
        if session.get("total_distance") is not None
        else segment_distance(points)
    )

    active = moving_points(points)
    avg_speed = time_weighted_mean(active, "speed_mps")
    avg_hr = time_weighted_mean(active, "heart_rate")
    avg_cadence = time_weighted_mean(active, "cadence")
    avg_power = time_weighted_mean(active, "power")
    peak_hr_raw = safe_max(p.heart_rate for p in points)

    fit_gain = (
        float(session["total_ascent"])
        if session.get("total_ascent") is not None
        else None
    )
    smoothed_gain = positive_gain(smooth_alt, noise_threshold_m=1.0)

    breakdown = activity_time_breakdown(points)
    zones = zone_time(points, max_hr, lthr)
    decouple = first_second_half(points)
    late = late_ride_metrics(points)

    climbs = detect_valley_to_crest_climbs(
        points,
        smooth_alt,
        max_hr=max_hr,
        lthr=lthr,
    )
    climb_compare = climb_consistency(climbs)
    hourly = hourly_metrics(points, smooth_alt)

    summary = {
        "elapsed_hours": elapsed_s / 3600.0,
        "distance_km": distance_m / 1000.0 if distance_m is not None else None,
        "distance_miles": distance_m / 1609.344 if distance_m is not None else None,
        "avg_speed_kph": avg_speed * 3.6 if avg_speed is not None else None,
        "avg_speed_mph": avg_speed * 2.236936 if avg_speed is not None else None,
        "fit_total_ascent_m": fit_gain,
        "fit_total_ascent_ft": fit_gain * 3.28084 if fit_gain is not None else None,
        "smoothed_ascent_m": smoothed_gain,
        "smoothed_ascent_ft": smoothed_gain * 3.28084,
        "climbing_density_ft_per_hour": (
            (fit_gain if fit_gain is not None else smoothed_gain)
            * 3.28084
            / (elapsed_s / 3600.0)
            if elapsed_s > 0
            else None
        ),
        "avg_hr": avg_hr,
        "max_hr": peak_hr_raw,
        "avg_cadence": avg_cadence,
        "avg_power": avg_power,
        "normalized_power": session.get("normalized_power"),
        "moving_hours": breakdown["moving_s"] / 3600.0,
        "stopped_minutes": breakdown["stopped_s"] / 60.0,
        "coasting_minutes": breakdown["coasting_s"] / 60.0,
    }

    valid_recovery_60 = [
        c.recovery_60s_bpm
        for c in climbs
        if c.recovery_60s_bpm is not None and c.recovery_60s_bpm >= 0
    ]

    climb_summary = {
        "detected_climbs": len(climbs),
        "total_detected_climb_gain_m": sum(c.gain_m for c in climbs),
        "median_climb_gain_m": safe_median(c.gain_m for c in climbs),
        "median_vertical_rate_m_per_h": safe_median(
            c.vertical_rate_m_per_h for c in climbs
        ),
        "median_60s_hr_recovery_bpm": safe_median(valid_recovery_60),
        "median_seconds_to_drop_10_bpm": safe_median(
            c.seconds_to_drop_10_bpm for c in climbs
        ),
    }

    readiness = marji_readiness(
        summary,
        decouple,
        late,
        climb_compare,
        zones,
        breakdown,
    )

    report = {
        "ride_summary": {
            "source_file": fit_path.name,
            "activity_start": points[0].timestamp.isoformat(),
            "activity_end": points[-1].timestamp.isoformat(),
            "record_count": len(points),
            "inputs": {
                "max_hr": max_hr,
                "lthr": lthr,
            },
            "metrics": summary,
            "heart_rate_zones": zones,
            "aerobic_decoupling": decouple,
            "late_ride_comparison": late,
        },
        "hourly_durability": {
            "hours": [asdict(h) for h in hourly],
            "notes": [
                "Use hour-by-hour speed, HR, ascent, stops, and efficiency to identify when fatigue begins.",
                "Mountain-bike terrain changes can affect speed and efficiency independently of fitness."
            ],
        },
        "climb_analysis": {
            "summary": climb_summary,
            "first_vs_second_half": climb_compare,
            "climbs": [asdict(c) for c in climbs],
            "method_notes": {
                "detection": (
                    "Climbs are detected from a smoothed altitude trace using a local "
                    "valley-to-confirmed-crest method. Defaults require at least 20 m gain, "
                    "90 seconds, and 300 m distance."
                ),
                "recovery": (
                    "Recovery HR uses the mean HR from the final 20 seconds before the summit "
                    "and compares it with 30- and 60-second post-summit windows. Immediate "
                    "descending or continued hard pedaling can still distort this metric."
                ),
            },
        },
        "marji_readiness": {
            **readiness,
            "analysis_notes": [
                "Speed/HR decoupling is terrain-sensitive on mountain-bike rides.",
                "Compare similar routes and prioritize hourly and climb-specific trends.",
                "This section is a training heuristic, not a physiological test or race-time prediction."
            ],
        },
    }

    return round_nested(report), climbs, hourly


# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------

def write_csv(path: Path, rows: list[Any], fields: list[str]) -> None:
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        for row in rows:
            writer.writerow(round_nested(asdict(row)))


def fmt(value: Any, suffix: str = "") -> str:
    if value is None:
        return "n/a"
    if isinstance(value, float):
        return f"{value:.2f}{suffix}"
    return f"{value}{suffix}"


def write_text_report(path: Path, report: dict[str, Any]) -> None:
    s = report["summary"]
    d = report["aerobic_decoupling"]
    l = report["late_ride_comparison"]
    c = report["climb_summary"]
    cc = report["climb_consistency"]
    r = report["marji_readiness"]

    lines = [
        f"FIT ANALYSIS: {report['source_file']}",
        "=" * 72,
        "",
        "SUMMARY",
        f"Distance: {fmt(s.get('distance_miles'), ' mi')}",
        f"Elapsed: {fmt(s.get('elapsed_hours'), ' h')}",
        f"Average speed: {fmt(s.get('avg_speed_mph'), ' mph')}",
        f"Ascent: {fmt(s.get('fit_total_ascent_ft') or s.get('smoothed_ascent_ft'), ' ft')}",
        f"Climbing density: {fmt(s.get('climbing_density_ft_per_hour'), ' ft/h')}",
        f"Average HR: {fmt(s.get('avg_hr'), ' bpm')}",
        f"Maximum HR: {fmt(s.get('max_hr'), ' bpm')}",
        f"Stopped time: {fmt(s.get('stopped_minutes'), ' min')}",
        f"Coasting time: {fmt(s.get('coasting_minutes'), ' min')}",
        "",
        "AEROBIC DURABILITY",
        f"First-half speed: {fmt(d.get('first_half_speed_kph'), ' km/h')}",
        f"Second-half speed: {fmt(d.get('second_half_speed_kph'), ' km/h')}",
        f"First-half HR: {fmt(d.get('first_half_avg_hr'), ' bpm')}",
        f"Second-half HR: {fmt(d.get('second_half_avg_hr'), ' bpm')}",
        f"Aerobic decoupling: {fmt(d.get('aerobic_decoupling_pct'), '%')}",
        "",
        "LATE-RIDE COMPARISON",
        f"Speed change: {fmt(l.get('speed_change_pct'), '%')}",
        f"HR change: {fmt(l.get('hr_change_pct'), '%')}",
        f"Cadence change: {fmt(l.get('cadence_change_pct'), '%')}",
        f"Power change: {fmt(l.get('power_change_pct'), '%')}",
        "",
        "CLIMBS",
        f"Detected climbs: {fmt(c.get('detected_climbs'))}",
        f"Median vertical rate: {fmt(c.get('median_vertical_rate_m_per_h'), ' m/h')}",
        f"Median 60-second HR recovery: {fmt(c.get('median_60s_hr_recovery_bpm'), ' bpm')}",
        f"Median time to drop 10 bpm: {fmt(c.get('median_seconds_to_drop_10_bpm'), ' s')}",
        f"Second-half climbing-rate change: {fmt(cc.get('vertical_rate_change_pct'), '%')}",
        "",
        "MARJI READINESS HEURISTICS",
    ]

    for check in r["checks"]:
        lines.append(
            f"- {check['metric']}: {fmt(check['value'])} "
            f"[{check['status'].upper()}] — {check['note']}"
        )

    lines += [
        "",
        "LIMITATIONS",
        *[f"- {item}" for item in r["important_limitations"]],
        "",
    ]

    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Analyze Garmin FIT files for endurance MTB and climbing durability."
    )
    parser.add_argument("fit_file", type=Path)
    parser.add_argument("--max-hr", type=int, default=None)
    parser.add_argument("--lthr", type=int, default=None)
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="Output JSON path or filename prefix; defaults to FIT filename with .json",
    )
    args = parser.parse_args()

    if not args.fit_file.exists():
        print(f"File not found: {args.fit_file}", file=sys.stderr)
        return 2

    if args.output is None:
        json_path = args.fit_file.with_suffix(".json")
    elif args.output.suffix.lower() == ".json":
        json_path = args.output
    else:
        json_path = Path(f"{args.output}.json")

    try:
        points, session = parse_fit(args.fit_file)
        report, _, _ = build_report(
            args.fit_file,
            points,
            session,
            args.max_hr,
            args.lthr,
        )
    except Exception as exc:
        print(f"Failed to analyze FIT file: {exc}", file=sys.stderr)
        return 1

    json_path.write_text(json.dumps(report, indent=2), encoding="utf-8")

    summary = report["ride_summary"]["metrics"]
    decoupling = report["ride_summary"]["aerobic_decoupling"]
    climbs = report["climb_analysis"]["summary"]
    readiness = report["marji_readiness"]["counts"]

    print(f"Wrote one uploadable report: {json_path}")
    print()
    print(f"Distance: {fmt(summary.get('distance_miles'), ' mi')}")
    print(f"Elapsed: {fmt(summary.get('elapsed_hours'), ' h')}")
    print(f"Ascent: {fmt(summary.get('fit_total_ascent_ft') or summary.get('smoothed_ascent_ft'), ' ft')}")
    print(f"Aerobic decoupling: {fmt(decoupling.get('aerobic_decoupling_pct'), '%')}")
    print(f"Detected climbs: {fmt(climbs.get('detected_climbs'))}")
    print(
        "Readiness checks: "
        f"{readiness.get('strong', 0)} strong, "
        f"{readiness.get('acceptable', 0)} acceptable, "
        f"{readiness.get('focus', 0)} focus"
    )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
