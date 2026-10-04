# Bike platform and service history

Bike's active architecture is described in [architecture.md](architecture.md).
This record preserves the Rust platform and operational milestones from the
2026-10-01 consolidation. Earlier detailed cutover records remain recoverable
in Git through `043445f`; they describe their original deployments and dates.
The [backlog](TODO.md) is the only active task checklist.

## Rust platform consolidation

The Rust application owns its API, worker, migrations, and core platform modules.
The Next.js frontend is maintained once in `bike-ui`. Rust's Utoipa output is
used to distribute the HTTP contract and generate the frontend TypeScript
client. Original repository histories and schema migrations were preserved.

CI and release ownership follow the Rust backend, UI, renderer, and gateway
boundaries. Release images use immutable commit tags; backend migration Jobs
complete before API/worker promotion. Persistent resource identities stay in
Pulumi. Local root Compose starts the application and renderer together.

## Two-disk backups (2026-10-02, REC08)

Pulumi revisions `831c1a0` and `dd6a898` added complete restore points on both
host disks, a 02:00 `America/Detroit` schedule, 14-generation retention,
read-only source mounts, disk identity checks, an overlap lock, writer pauses,
independent publication, encrypted recovery configuration, and monitoring.
The database server and original backup volume remained unchanged.

`pg-backup-manual-20261002143334` published and validated restore generation
`20261002T143335Z` under `/mnt/sda/bike-backups/` and
`/mnt/sdb/bike-backups/`. All paused writer deployments resumed. The
[operator record](../../pulumi-iac/docs/Bike-Backup-Runbook.md#2026-10-02-first-backup)
records exact completion times, revisions, checks, and initial wiring fixes.

## Isolated restore (2026-10-02, REC09)

The saved database and file set restored into disposable PostgreSQL/storage in
`bike-recovery`, with pod traffic blocked and no background writer or delivery
process. SQL restore passed with `ON_ERROR_STOP=1`. The saved real FIT ride
`1588`, owner `1`, retained its full row, 9,212 route points, metrics, and
source-file hash. The restored Rust API served list, detail, and matching FIT
source download with HTTP 200.

Gateway connection/queue/event IDs, an artifact, and the saved encryption key
were recovered without logging the key. Correcting upload-directory traverse
permission enabled the non-root API to serve the file. Live data and source
snapshots were unchanged. The
[restore record](../../pulumi-iac/docs/Bike-Backup-Runbook.md#2026-10-02-isolated-restore)
contains commands, isolation, IDs, digests, and cleanup.

## Independent product verification (2026-10-02)

Authentication/session behavior, provider exchange and refresh, downstream
import, and UI list/detail checks were verified independently. Identity and
provider calls used fakes; the real-ride fixture retains explicit provenance.
The browser checks used deployed UI assets with stubbed application APIs.
These results are separate from live-provider and backup recovery evidence.
See [auth evidence](../bike-rs/docs/specs/auth-configuration.md#independent-auth-happy-path-evidence-2026-10-02)
and [provider fixture evidence](../strava-gateway/internal/provider/testdata/README.md).
