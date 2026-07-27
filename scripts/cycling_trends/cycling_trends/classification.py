from __future__ import annotations

import csv
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Optional

from .analyzer import Point, safe_mean
from .config import RidePrescription


@dataclass(frozen=True)
class Classification:
    ride_type: str
    confidence: float
    source: str
    matched_hr_pct: float
    duration_score: float
    explanation: str


def load_label_overrides(path: Optional[Path]) -> dict[str, str]:
    if path is None or not path.exists():
        return {}
    with path.open(newline="", encoding="utf-8") as handle:
        return {
            row["filename"].strip(): row["ride_type"].strip()
            for row in csv.DictReader(handle)
            if row.get("filename") and row.get("ride_type")
        }


def _time_in_hr_range(points: list[Point], low: int, high: int) -> tuple[float, float]:
    matched = 0.0
    total = 0.0
    for left, right in zip(points, points[1:]):
        dt = max(0.0, min(10.0, right.elapsed_s - left.elapsed_s))
        if left.heart_rate is None:
            continue
        total += dt
        if low <= left.heart_rate <= high:
            matched += dt
    return matched, total


def _duration_score(hours: float, minimum: float, maximum: float) -> float:
    if minimum <= hours <= maximum:
        return 1.0
    distance = minimum - hours if hours < minimum else hours - maximum
    scale = max(0.5, maximum - minimum)
    return max(0.0, 1.0 - distance / scale)


def _combo_score(points: list[Point], rx: RidePrescription) -> tuple[float, str]:
    if not points:
        return 0.0, "No point data"
    split = points[-1].elapsed_s * 0.4
    early = [p.heart_rate for p in points if p.elapsed_s <= split and p.heart_rate is not None]
    late = [p.heart_rate for p in points if p.elapsed_s > split and p.heart_rate is not None]
    early_hr = safe_mean(early)
    late_hr = safe_mean(late)
    if early_hr is None or late_hr is None:
        return 0.0, "Insufficient HR data for combo pattern"
    # Expected pattern: harder opening block followed by lower-intensity endurance.
    score = 0.0
    if early_hr >= 141:
        score += 0.5
    if 128 <= late_hr <= 141:
        score += 0.35
    if early_hr > late_hr:
        score += 0.15
    return score, f"early HR {early_hr:.1f}, late HR {late_hr:.1f}"


def classify_ride(
    filename: str,
    points: list[Point],
    elapsed_hours: float,
    prescriptions: Iterable[RidePrescription],
    override: Optional[str] = None,
) -> Classification:
    if override:
        return Classification(
            ride_type=override,
            confidence=1.0,
            source="override",
            matched_hr_pct=0.0,
            duration_score=1.0,
            explanation="Explicit label from ride_labels.csv",
        )

    scored: list[tuple[float, RidePrescription, float, float, str]] = []
    for rx in prescriptions:
        matched, total = _time_in_hr_range(points, rx.hr_min, rx.hr_max)
        hr_pct = 100.0 * matched / total if total else 0.0
        hr_score = min(1.0, hr_pct / 65.0)
        duration = _duration_score(elapsed_hours, rx.duration_min_hours, rx.duration_max_hours)
        detail = f"{hr_pct:.1f}% in {rx.hr_min}-{rx.hr_max} bpm"
        if rx.ride_type.lower().startswith("combo"):
            combo, combo_detail = _combo_score(points, rx)
            hr_score = 0.35 * hr_score + 0.65 * combo
            detail = f"{detail}; {combo_detail}"
        score = 0.65 * hr_score + 0.35 * duration
        scored.append((score, rx, hr_pct, duration, detail))

    if not scored:
        return Classification("Unclassified", 0.0, "automatic", 0.0, 0.0, "No prescription candidates")

    score, rx, hr_pct, duration, detail = max(scored, key=lambda item: item[0])
    return Classification(
        ride_type=rx.ride_type,
        confidence=round(score, 3),
        source="automatic",
        matched_hr_pct=round(hr_pct, 2),
        duration_score=round(duration, 3),
        explanation=detail,
    )
