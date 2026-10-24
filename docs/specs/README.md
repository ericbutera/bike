# Bike Specifications

This folder is the natural-language source of truth for Bike behavior, including
user workflows, background processing, service boundaries, and operational
contracts. Code implements these specifications; tests verify that implementation.
Update the owning spec in the same change as any intended behavior change.

- [Durable worker and Strava integration](background-work.md): the single worker
  specification, including the complete task inventory, client submission,
  domain behavior, native durability, storage, and provider protocol.
- [Activity processing DAG](performance.md): immutable progressive refinement,
  saved stage inputs/outputs, shared segments, ordered efforts, activity awards
  and downstream consumers.
- [TODO](../TODO.md): outstanding work only; it does not duplicate the worker spec.
- [Fix data loading TODO](../TODO-fix-data-loading.md): canonical N+1 and repeated
  read/write checklist, including screen recalculation; consult its related
  items when implementing DAG stages or features using the same data.
- [Activities](activities.md): activity list and activity details specifications.
- [Achievements](achievements.md): personal segment podiums and saved stage output.
- [Authentication and user data](authentication.md): ownership boundaries and browser session isolation.

## Spec Rules

- Specs describe the intended behavior first. Code anchors are supporting references, not substitutes for the product contract.
- Keep each behavioral contract in one owning spec. Guides, plans, component
  READMEs, and generated HTTP/protobuf schemas link to it rather than maintaining
  competing requirements. Resolve a mismatch explicitly against the intended
  contract; passing tests or current code do not silently override the spec.
- Deterministic rules should be documented before an LLM or narrative layer summarizes them.
- If a change fixes a bug by changing intended behavior, update the spec so future revisions do not restore the old behavior.
- If design details are unresolved, record the relevant TODO ID in the spec and keep the technical options here as design context. Do not maintain a second task or status checklist in a spec.
- Before creating a new spec, check whether an existing domain spec can own the behavior. Prefer expanding the owning spec over creating a narrower overlapping file.
- Avoid reviving root-level feature backlog files for Bike. Add new feature specs here and link them from this index.
