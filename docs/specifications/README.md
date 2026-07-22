# Bike Specifications

This folder is the natural-language source of truth for Bike product behavior. When a code change modifies user-visible behavior, background processing semantics, or training recommendations, update the relevant spec in the same change.

The specs are intentionally written as product contracts instead of implementation logs. Open gaps and future decisions are tracked, but the main body of each file describes the behavior the code should preserve.

## Coverage

| Feature area | Specification |
| --- | --- |
| Current product boundaries, architecture, data strategy, and non-goals | [Project Overview](project-overview.md) |
| Manual uploads, archive imports, Strava import jobs, and processing state | [Activity Ingestion](activity-ingestion.md) |
| Activity list/detail, route preview, telemetry, activity editing, and regeneration | [Activity Experience](activity-experience.md) |
| Segments, segment builder, matching, modes, summaries, and comparison views | [Segment Specification](segment-processing.md) |
| XC/DH analytics, deterministic training metrics, fitness freshness, reports, and recommendations | [Training Analytics](training-analytics.md) |
| XC event target, readiness gates, missing-work ranking, and next-ride guidance | [XC Event Readiness](xc-event-readiness.md) |
| Account preferences, heart-rate zones, FTP, units, Strava, and Garmin IQ | [Account And Integrations](account-integrations.md) |
| Admin metrics, backfills, manual tasks, integration event history, and operational tools | [Admin Operations](admin-operations.md) |

## Spec Rules

- Specs describe the intended behavior first. Code anchors are supporting references, not substitutes for the product contract.
- Deterministic rules should be documented before an LLM or narrative layer summarizes them.
- If a change fixes a bug by changing intended behavior, update the spec so future revisions do not restore the old behavior.
- If implementation details are uncertain, keep them in an "Open Gaps" or "Open Decisions" section instead of weakening the product contract.
- Avoid reviving root-level feature backlog files for Bike. Add new feature specs here and link them from this index.
