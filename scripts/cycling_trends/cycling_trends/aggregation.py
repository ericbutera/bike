from __future__ import annotations

from collections import Counter, defaultdict
from statistics import fmean, median
from typing import Any, Iterable, Optional

from .config import RidePrescription
from .models import RideTrendRow


def _values(rows: Iterable[RideTrendRow], field: str) -> list[float]:
    result: list[float] = []
    for row in rows:
        value = getattr(row, field)
        if value is not None:
            result.append(float(value))
    return result


def summarize_group(rows: list[RideTrendRow]) -> dict[str, Any]:
    return {
        "rides": len(rows),
        "hours": round(sum(r.elapsed_hours for r in rows), 2),
        "distance_miles": round(sum(r.distance_miles for r in rows), 2),
        "ascent_ft": round(sum(r.ascent_ft for r in rows), 2),
        "median_decoupling_pct": _round_median(_values(rows, "decoupling_pct")),
        "median_climb_rate_m_per_h": _round_median(_values(rows, "median_vertical_rate_m_per_h")),
        "median_climb_fade_pct": _round_median(_values(rows, "climb_vertical_rate_change_pct")),
        "mean_avg_hr": _round_mean(_values(rows, "avg_hr")),
        "ride_types": dict(Counter(r.ride_type for r in rows)),
    }


def _round_mean(values: list[float]) -> Optional[float]:
    return round(fmean(values), 2) if values else None


def _round_median(values: list[float]) -> Optional[float]:
    return round(median(values), 2) if values else None


def aggregate(rows: list[RideTrendRow]) -> dict[str, Any]:
    by_week: dict[str, list[RideTrendRow]] = defaultdict(list)
    by_month: dict[int, list[RideTrendRow]] = defaultdict(list)
    by_type: dict[str, list[RideTrendRow]] = defaultdict(list)
    for row in rows:
        by_week[row.iso_week].append(row)
        by_month[row.month].append(row)
        by_type[row.ride_type].append(row)
    return {
        "overall": summarize_group(rows),
        "weekly": {key: summarize_group(value) for key, value in sorted(by_week.items())},
        "monthly": {str(key): summarize_group(value) for key, value in sorted(by_month.items())},
        "by_ride_type": {key: summarize_group(value) for key, value in sorted(by_type.items())},
    }


def plan_compliance(rows: list[RideTrendRow], plan: list[RidePrescription]) -> list[dict[str, Any]]:
    by_week: dict[tuple[int, str], list[RideTrendRow]] = defaultdict(list)
    for row in rows:
        by_week[(row.month, row.iso_week)].append(row)

    output: list[dict[str, Any]] = []
    for (month, week), week_rows in sorted(by_week.items()):
        month_plan = [rx for rx in plan if rx.month == month]
        counts = Counter(row.ride_type for row in week_rows)
        for rx in month_plan:
            actual = counts.get(rx.ride_type, 0)
            target = rx.frequency_per_week
            output.append(
                {
                    "month": month,
                    "iso_week": week,
                    "ride_type": rx.ride_type,
                    "target_per_week": target,
                    "actual": actual,
                    "met": actual >= target,
                    "difference": round(actual - target, 2),
                }
            )
    return output
