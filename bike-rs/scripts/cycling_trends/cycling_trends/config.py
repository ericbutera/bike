from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class RidePrescription:
    month: int
    ride_type: str
    frequency_per_week: float
    hr_min: int
    hr_max: int
    duration_min_hours: float
    duration_max_hours: float
    notes: str = ""


def load_plan(path: Path) -> list[RidePrescription]:
    raw: dict[str, Any] = json.loads(path.read_text(encoding="utf-8"))
    return [RidePrescription(**item) for item in raw["prescriptions"]]
