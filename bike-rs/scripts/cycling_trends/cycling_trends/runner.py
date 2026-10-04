from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path
from typing import Any, Optional

from .aggregation import aggregate, plan_compliance
from .analyzer import build_report, parse_fit
from .classification import classify_ride, load_label_overrides
from .config import load_plan
from .models import RideTrendRow
from .output import write_dict_rows_csv, write_json, write_rows_csv


def _number(value: Any, default: float = 0.0) -> float:
    return float(value) if value is not None else default


def _row_from_report(
    fit_path: Path,
    report: dict[str, Any],
    classification: Any,
) -> RideTrendRow:
    summary = report["ride_summary"]
    metrics = summary["metrics"]
    zones = summary["heart_rate_zones"]["percent"]
    climbs = report["climb_analysis"]
    climb_summary = climbs["summary"]
    start = datetime.fromisoformat(summary["activity_start"])
    iso = start.isocalendar()
    return RideTrendRow(
        filename=fit_path.name,
        start=start.isoformat(),
        date=start.date().isoformat(),
        month=start.month,
        iso_week=f"{iso.year}-W{iso.week:02d}",
        ride_type=classification.ride_type,
        classification_confidence=classification.confidence,
        classification_source=classification.source,
        classification_explanation=classification.explanation,
        elapsed_hours=_number(metrics.get("elapsed_hours")),
        moving_hours=_number(metrics.get("moving_hours")),
        distance_miles=_number(metrics.get("distance_miles")),
        ascent_ft=_number(metrics.get("fit_total_ascent_ft")),
        climbing_density_ft_per_hour=_number(metrics.get("climbing_density_ft_per_hour")),
        avg_hr=metrics.get("avg_hr"),
        max_hr=metrics.get("max_hr"),
        z1_pct=zones.get("Z1"),
        z2_pct=zones.get("Z2"),
        z3_pct=zones.get("Z3"),
        z4_pct=zones.get("Z4"),
        z5_pct=zones.get("Z5"),
        decoupling_pct=summary["aerobic_decoupling"].get("aerobic_decoupling_pct"),
        late_speed_change_pct=summary["late_ride_comparison"].get("speed_change_pct"),
        detected_climbs=int(climb_summary.get("detected_climbs") or 0),
        median_climb_gain_m=climb_summary.get("median_climb_gain_m"),
        median_vertical_rate_m_per_h=climb_summary.get("median_vertical_rate_m_per_h"),
        climb_vertical_rate_change_pct=climbs["first_vs_second_half"].get("vertical_rate_change_pct"),
        median_recovery_60s_bpm=climb_summary.get("median_60s_hr_recovery_bpm"),
        stopped_minutes=_number(metrics.get("stopped_minutes")),
        source_mtime=fit_path.stat().st_mtime,
    )


def analyze_directory(
    fit_dir: Path,
    output_dir: Path,
    plan_path: Path,
    labels_path: Optional[Path],
    lthr: Optional[int],
    max_hr: Optional[int],
    recursive: bool = False,
) -> list[RideTrendRow]:
    output_dir.mkdir(parents=True, exist_ok=True)
    cache_dir = output_dir / "ride_reports"
    cache_dir.mkdir(exist_ok=True)
    plan = load_plan(plan_path)
    overrides = load_label_overrides(labels_path)
    pattern = "**/*.fit" if recursive else "*.fit"
    fit_files = sorted(fit_dir.glob(pattern))
    rows: list[RideTrendRow] = []

    for fit_path in fit_files:
        cache_path = cache_dir / f"{fit_path.stem}.json"
        points, session = parse_fit(fit_path)
        report, _, _ = build_report(fit_path, points, session, max_hr, lthr)
        month_plan = [rx for rx in plan if rx.month == points[0].timestamp.month]
        classification = classify_ride(
            filename=fit_path.name,
            points=points,
            elapsed_hours=report["ride_summary"]["metrics"]["elapsed_hours"],
            prescriptions=month_plan,
            override=overrides.get(fit_path.name),
        )
        report["trend_classification"] = classification.__dict__
        write_json(cache_path, report)
        rows.append(_row_from_report(fit_path, report, classification))
        print(f"Analyzed {fit_path.name}: {classification.ride_type} ({classification.confidence:.2f})")

    rows.sort(key=lambda row: row.start)
    write_rows_csv(output_dir / "rides.csv", rows)
    trends = aggregate(rows)
    compliance = plan_compliance(rows, plan)
    write_json(output_dir / "trends.json", {"trends": trends, "plan_compliance": compliance})
    write_dict_rows_csv(output_dir / "plan_compliance.csv", compliance)
    return rows
