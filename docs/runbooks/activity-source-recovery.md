# Activity source recovery

For normal failure recovery, open **Imports**, choose the import, select the
failed stage, and use **Replay from here**. Check the displayed start stage:
older or stale checkpoints may require replay from Raw stored. Earlier
attempts remain available in the attempt selector.

Restore missing or corrupt retained bytes before replay. The DAG reaching its
last stage establishes normalized processing; verify fitness and heatmap
publication separately. This runbook covers the exceptional source repairs
described in the [ingestion contract](../specs/activity-ingestion.md).

## Generated Strava TCX retirement and source backfill

The 2026-10-06 recovery decision replaces indefinite generated-TCX replay with
backfill to retained authentic sources. Bike must stop generating TCX during
new Strava deliveries and updates. Persist the versioned provider summary and
streams JSON as the primary import artifact and parse it directly. This is
Bike-owned work; the gateway payload format stays unchanged. Original TCX
uploaded by a rider remains a supported input format.

The parser rejects generated artifact labels and the exact retired Bike cycling
export header even if a manual upload or archive entry labels those bytes as
an original. Genuine TCX inputs remain supported. Historical mislabeled copies
must be relabeled as generated and withheld from heatmaps while their authentic
source is recovered; preserve their summaries, normalized GPS, and sole retained
bytes until a verified replacement or an explicit deletion decision exists.
Withholding a source also invalidates its projection generation, removes its
published chunks, and advances the owner's tile revision in the same transaction.

Repair old Bike-generated TCX imports in bounded owner-scoped batches:

1. Recover a verified original from the rider's retained archive data.
   Prefer FIT. Match owned counterparts using absolute GPS time and track
   agreement, not titles or exact summary timestamps. Reject ambiguous matches
   and verify the retained bytes before attaching them to a different import.
2. Otherwise promote a retained Strava provider payload. Existing local raw
   payloads can repair any age of cycling activity without consuming provider
   quota. Historical non-cycling inputs can be promoted as retained sources
   without cycling parsing or replay jobs, preserving their existing data.
   This repair is distinct from ACT05's future metadata-only ingestion path.
3. If neither exists, classify gaps within the last 30 days for incremental
   gateway refresh. Older gaps require archive recovery; never start a full
   historical API sync automatically. Missing or ambiguous originals are an
   explicit unresolved outcome, not permission to keep generated TCX as the
   permanent source of truth.

Use a dry-run inventory before applying repairs. Preserve provider/Bike IDs,
ownership, user classifications, and original artifacts. Reprocess from the
replacement source through the normal graph and rebuild dependent state.
Repeated repair is idempotent. Do not delete the only retained bytes during
recovery; retiring generated-TCX parsing is distinct from deleting historical
evidence. Delete obsolete generated files and their artifact records after a
verified replacement has completed replay. Originals, IDs, summaries, and
normalized detail remain retained. Track unrecovered records rather than
inventing metadata or treating a generated file as an original.

The existing gateway `incremental` mode uses a rolling 30-day window. It cannot
request a selected list of missing IDs through the current Bike command
contract, so a recent refresh also revisits already retained recent activities.
Archive recovery and retained JSON use no Strava requests. Provider refresh is
explicit and uses the gateway's existing quota/retry mechanisms. Strava streams
are provider-derived data, not an original FIT download.

Both first-time connections (`initial`) and ordinary refresh (`incremental`)
use the last 30 days. The inactive legacy Bike sync path also clamps its
cursor to this window so re-enabling it cannot restore full-history fetching.
The recovery command only requests incremental refresh;
it cannot request `initial` or `full`. Older history must come from archives.
An explicit operator-only `full` gateway mode still exists and is not used by
Bike connection or recovery flows. Production pipeline 186 deployed source
recovery and v5 admission, and pipeline 187 deployed the scalar recording gate.
The 30-day initial connection behavior is covered at the gateway boundary;
production backfill used retained archives and payloads with zero Strava requests.

The `recover-generated-imports` binary is included in the API image. A dry run
requires the intended `DATABASE_URL` and mounted `UPLOADS_DIR`, performs no
writes or provider requests, and reports one owned page (default/max 32):

```sh
mise --cd bike-rs run imports:recover -- --user-id <user-id> --limit 32
```

Repeat with the reported `next_after_id` as `--after-id`. Review every outcome
and error before applying the same page with `--apply`. Promotion retains a
separate copy of an archive original and atomically queues the existing
single-activity replay job with up to three attempts. Replay uses the normal
processing graph and heatmap invalidation. Monitor those jobs to completion;
source promotion alone is not a completed heatmap repair. CAS and activity
locking reject stale plans; rejected stale plans remove their newly copied file.
Infrastructure failures retain the copy because commit completion may be
uncertain; inspect the persisted import and queued job before retrying.
Reading one source is bounded to 100 MiB, matching the archive entry limit.

An operator can recover additional originals from an owned Strava archive's
`activities.csv` provider-ID/filename mapping without API usage. Extract exact
original bytes into Bike uploads storage (decompress archive gzip wrappers,
not the underlying activity format), then pass a private JSON manifest to
`--register-manifest <path>`. Each entry contains `import_id`,
`provider_activity_id`, `original_filename`, `storage_path`, `format`,
`checksum_sha256`, and optional `recording_label` from the archive activity type.
The command accepts at most 32 entries and a 1 MiB manifest, is read-only by
default, and registers sources only with `--apply`. It verifies ownership,
provider identity, checksums, format, start time, and cycling GPS agreement.
Repeated registration is idempotent. Rejecting archive type evidence is merged
transactionally and survives replay. Do not commit manifests or export CSVs.

Authentic Strava phone-recording JSON from an archive is retained unchanged as
an `original` with quality `strava_archive_json`. Its metadata and field/value
GPS series are parsed directly; epoch sample timestamps become elapsed seconds
relative to the retained activity start. Native normalization is reused in
memory without writing an intermediate export. Original bytes, including
accuracy and pause fields that are not yet normalized, remain retained.
Non-cycling originals and retained provider JSON require no cycling replay jobs.

After source recovery, run `--cleanup` first as a dry run, then with `--apply`.
This mode pages by **artifact ID**, using the reported cursor independently of
the import recovery cursor. It removes only obsolete generated artifacts whose
replacement bytes verify, whose import is processed, and whose path is no
longer used by any import or other artifact. Failed/pending replay and missing
originals block cleanup. The filesystem deletion precedes metadata removal;
retry accepts an already missing generated file after a database interruption.
The verified original is never removed. Historical migration labels and
synthetic retirement regression tests remain; there is no generated-file writer
or generated-file parser fallback.

`--refresh-recording` reads an owned page of supported cycling activities from
their checksum-verified retained originals/provider JSON and restores missing
recording evidence without replacing normalized GPS or running the ride graph.
It is read-only by default; `--apply` persists only changed context with an owner
and source-version guard. This also repairs old original FIT imports whose
normalized data predates recorder metadata support. Existing virtual/indoor
evidence cannot be cleared, and changed evidence invalidates heatmap geometry.
This mode uses an **activity ID** cursor and makes no provider requests.

`--apply --withhold-unrecovered` marks original-source gaps as unavailable for
heatmap publication while preserving summaries, existing normalized GPS,
provider IDs, and source-recovery records. It deletes no files and makes no
provider requests. Tiles, bounds, zones, and counts exclude unavailable sources;
preparation publishes no chunks. Authentic-source replay restores the primary
format and eligibility subject to the normal recording checks. Removing the
last generated file without a replacement requires an explicit retirement
decision or an updated archive; ordinary cleanup cannot erase the sole source.

`--apply --refresh-recent` explicitly requests one incremental gateway sync if
this page has recent gaps. It revisits all activities in that window rather
than selected IDs, so use it once after collecting the dry-run inventory.
Never substitute a full sync for unresolved historical gaps. Revisit failed
import IDs before advancing the recovery audit; errors produce nonzero exit
status while other records in the page are still examined. Native sources,
missing archives, and ambiguous originals are reported without promotion.
Do not commit recovery reports or original activity files.
