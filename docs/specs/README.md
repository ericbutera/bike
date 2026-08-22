# Bike Specifications

This folder is the natural-language source of truth for Bike product behavior. When a code change modifies user-visible behavior, background processing semantics, or training recommendations, update the relevant spec in the same change.

The specs are intentionally written as product contracts instead of implementation logs. Open gaps and future decisions are tracked, but the main body of each file describes the behavior the code should preserve.

## Specs

- [Project overview](project-overview.md)
- [Activity ingestion](activity-ingestion.md)
- [Activity experience](activity-experience.md)
- [Segment processing](segment-processing.md)
- [Segment race viewer](segment-race-viewer.md)
- [UI components](ui-components.md)
- [Training analytics](training-analytics.md)
- [Cycling trends reports](cycling-trends-reports.md)
- [XC event readiness](xc-event-readiness.md)
- [Reassessment report](reassessment-report.md)
- [Account integrations](account-integrations.md)
- [Admin operations](admin-operations.md)
- [Auth configuration](auth-configuration.md)

## Spec Rules

- Specs describe the intended behavior first. Code anchors are supporting references, not substitutes for the product contract.
- Deterministic rules should be documented before an LLM or narrative layer summarizes them.
- If a change fixes a bug by changing intended behavior, update the spec so future revisions do not restore the old behavior.
- If implementation details are uncertain, keep them in an "Open Gaps" or "Open Decisions" section instead of weakening the product contract.
- Before creating a new spec, check whether an existing domain spec can own the behavior. Prefer expanding the owning spec over creating a narrower overlapping file.
- Avoid reviving root-level feature backlog files for Bike. Add new feature specs here and link them from this index.
