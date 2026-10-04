# Cycling Trends

Modular trend analysis for a directory of Garmin FIT files.

## Install

```bash
python3 -m venv .venv
source .venv/bin/activate
python3 -m pip install -r requirements.txt
```

## Run

Put FIT files in a directory, then run from this project directory:

```bash
PYTHONPATH=. python3 -m cycling_trends.cli /path/to/fit-files \
  --lthr 170 \
  --output trend_output
```

Use `--recursive` when FIT files are in subdirectories.

## Outputs

- `trend_output/rides.csv`: one row per ride, suitable for a spreadsheet or pandas.
- `trend_output/trends.json`: overall, weekly, monthly, ride-type, and plan-compliance summaries.
- `trend_output/plan_compliance.csv`: weekly target-versus-actual counts.
- `trend_output/ride_reports/*.json`: the full four-section report for every ride.

## Project structure

- `analyzer.py`: existing single-ride calculations.
- `classification.py`: automatic training-plan classification and manual overrides.
- `config.py`: training-plan loading.
- `models.py`: stable trend-row data model.
- `aggregation.py`: weekly, monthly, and ride-type trend calculations.
- `output.py`: CSV and JSON writers.
- `runner.py`: directory orchestration.
- `cli.py`: command-line interface only.

Each module has one responsibility. New metrics should normally be added to `RideTrendRow`, `_row_from_report`, and `summarize_group`, rather than expanding the CLI or runner.

## Manual ride labels

Automatic classification is necessarily approximate, particularly on rugged terrain. Add overrides to `ride_labels.csv`:

```csv
filename,ride_type
2026 city loop.fit,Density
2026 valley of giants.fit,Long ride
```

The filename must exactly match the FIT filename.

## Classification behavior

For each monthly prescription, the classifier combines:

- percentage of recorded HR time inside the prescribed range: 65% weight
- closeness to prescribed duration: 35% weight

Combo rides additionally compare the first 40% of the ride with the remaining 60% and favor a harder opening followed by Z2.

The selected label, confidence, and explanation are included in `rides.csv` and each cached ride report. Treat low-confidence classifications as review items.

## Important limitations

- Speed-based trends must be compared within similar route or ride types.
- Climb-rate trends are distorted when climb grade and length differ substantially.
- FIT elevation spikes can create false climbs. Consider adding elevation cleanup in `analyzer.py` before relying on individual-climb records.
- Plan compliance counts frequency, not training quality or recovery status.
