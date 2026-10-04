from __future__ import annotations

from dataclasses import dataclass, asdict
from datetime import datetime
from typing import Any, Optional


@dataclass
class RideTrendRow:
    filename: str
    start: str
    date: str
    month: int
    iso_week: str
    ride_type: str
    classification_confidence: float
    classification_source: str
    classification_explanation: str
    elapsed_hours: float
    moving_hours: float
    distance_miles: float
    ascent_ft: float
    climbing_density_ft_per_hour: float
    avg_hr: Optional[float]
    max_hr: Optional[float]
    z1_pct: Optional[float]
    z2_pct: Optional[float]
    z3_pct: Optional[float]
    z4_pct: Optional[float]
    z5_pct: Optional[float]
    decoupling_pct: Optional[float]
    late_speed_change_pct: Optional[float]
    detected_climbs: int
    median_climb_gain_m: Optional[float]
    median_vertical_rate_m_per_h: Optional[float]
    climb_vertical_rate_change_pct: Optional[float]
    median_recovery_60s_bpm: Optional[float]
    stopped_minutes: float
    source_mtime: float

    def as_dict(self) -> dict[str, Any]:
        return asdict(self)
