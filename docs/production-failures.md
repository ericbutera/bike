# Worker pipeline diagnostics

## BikeWorkerPipelineBlocked

Open the alert's Bike overview dashboard and anomalous-run table. Follow its task
ID into admin tasks, then inspect the attempt, reason, observed value, budget,
heartbeat and meaningful progress. A fresh heartbeat does not prove progress.
From an activity's processing action, select the run and check its output revision.

Queue age excludes future schedules. Runtime measures one claimed execution;
logical completion includes waits and retries. Receipt-to-available uses committed
matching publications and a successful task tree. Failed downstream output stays
unready. Admin rerun creates a new origin; automatic retry preserves the receipt.

Default runtime budgets are processor-specific; progress is capped at 900 seconds,
queue eligibility at 1800 seconds and heartbeat age at 120 seconds.
`WORKER_PROCESSOR_BUDGETS` accepts per-processor JSON overrides, for example
`{"prepare_heatmap":{"runtime_seconds":2700,"progress_seconds":900,"queue_seconds":1800}}`.
Unknown processor/signal names or values outside 1–86400 fail startup.
The actual registry and metrics expose these budgets.

## BikeWorkerAnomalousRuns

The anomalous-run table and admin detail show the same versioned evidence.
Duration outliers require at least 30 earlier completed runs with the same
processor, processing version and actual workload cohort. The threshold is three
times their p90 with a processor-specific floor. Warm-up has no relative flag.
Repeated revision means published work repeated on the same accepted input;
stale publication means a superseded computation was prevented from publishing.
Clock skew stays explicit rather than becoming a plausible zero duration.

Compare actual domain counts by processor/kind/mode: inputs read, units computed,
rows written and outputs published. Counts cover measured stage operations,
not physical disk I/O or CPU instructions. A shared task appears under each
origin but contributes execution metrics once.

## BikeWorkerDiagnosticsUnavailable

Confirm the actual processor registry is present when the worker scrape is up.
Check migration/startup logs and the deployed image before trusting empty charts.
Legacy tasks, missing spans and cold histograms remain unknown.
`GRAFANA_URL` enables admin log/trace links.

## BikeWorkerCapacityPressure

Compare sustained busy wall time with eligible queue age, pod CPU, CPU throttling,
memory and storage. Busy wall time is not CPU utilization. Rising input/compute/
write counts without matching new outputs can indicate redundant rebuilds.
Scheduled tasks and gateway quota waits need separate interpretation.

Closed diagnostic history expires after 30 days in bounded batches. Active and
unready runs remain available for investigation. Authoritative imports, stages
and source artifacts follow their owning lifecycle. Temporary accepted delivery
source is cleared after durable execution. Bulk archive source is removed after
the child barrier closes, with restartable cleanup.

The draft has not been deployed. After integration, verify live worker/gateway
images, migrations, actual trace/log links, one upload/webhook through
receipt-to-available and the four provisioned dashboards. Synthetic fixture
footprint does not establish production memory or disk requirements.
