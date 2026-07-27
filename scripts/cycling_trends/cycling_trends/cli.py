from __future__ import annotations

import argparse
from pathlib import Path

from .runner import analyze_directory


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Analyze a directory of FIT files and track training trends.")
    parser.add_argument("fit_directory", type=Path)
    parser.add_argument("--output", type=Path, default=Path("trend_output"))
    parser.add_argument("--plan", type=Path, default=Path("training_plan.json"))
    parser.add_argument("--labels", type=Path, default=Path("ride_labels.csv"))
    parser.add_argument("--lthr", type=int, default=170)
    parser.add_argument("--max-hr", type=int, default=None)
    parser.add_argument("--recursive", action="store_true")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if not args.fit_directory.is_dir():
        raise SystemExit(f"FIT directory does not exist: {args.fit_directory}")
    if not args.plan.exists():
        raise SystemExit(f"Training plan does not exist: {args.plan}")
    rows = analyze_directory(
        fit_dir=args.fit_directory,
        output_dir=args.output,
        plan_path=args.plan,
        labels_path=args.labels if args.labels.exists() else None,
        lthr=args.lthr,
        max_hr=args.max_hr,
        recursive=args.recursive,
    )
    print(f"\nAnalyzed {len(rows)} rides")
    print(f"Outputs: {args.output.resolve()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
