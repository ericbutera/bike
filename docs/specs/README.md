# Bike Specifications

This folder is the natural-language source of truth for Bike product behavior. When a code change modifies user-visible behavior, background processing semantics, or training recommendations, update the relevant spec in the same change.

The specs are intentionally written as product contracts instead of implementation logs. Task status and priority live only in the [Bike TODO](../TODO.md); specs keep the behavior, technical constraints, and evidence needed to complete those items.

## Specs

- [Project overview](project-overview.md)
- [Supported activities](supported-activities.md)
- [Activity ingestion](activity-ingestion.md)
- [Personal heatmaps](heatmaps.md)
- [Activity experience](activity-experience.md)
- [Segment processing](segment-processing.md)
- [Segment race viewer](segment-race-viewer.md)
- [UI components](ui-components.md)
- [Training analytics](training-analytics.md)
- [Cycling trends reports](cycling-trends-reports.md)
- [XC event readiness](xc-event-readiness.md)
- [Reassessment report](reassessment-report.md)
- [Account integrations](account-integrations.md)
- [Strava integration](strava-integration.md)
- [Strava gateway fanout](strava-fanout.md)
- [Admin operations](admin-operations.md)
- [Auth configuration](auth-configuration.md)
- [Production synthetics](production-synthetics.md)

## Spec Rules

- Specs describe the intended behavior first. Code anchors are supporting references, not substitutes for the product contract.
- Deterministic rules should be documented before an LLM or narrative layer summarizes them.
- If a change fixes a bug by changing intended behavior, update the spec so future revisions do not restore the old behavior.
- If design details are unresolved, record the relevant TODO ID in the spec and keep the technical options here as design context. Do not maintain a second task or status checklist in a spec.
- Before creating a new spec, check whether an existing domain spec can own the behavior. Prefer expanding the owning spec over creating a narrower overlapping file.
- Avoid reviving root-level feature backlog files for Bike. Add new feature specs here and link them from this index.
