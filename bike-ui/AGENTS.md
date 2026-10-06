# Bike UI

This is the shared Next.js and React Query frontend for Bike Rust. Make UI,
route, presentation, and generated-client changes here; keep business behavior
in the Rust backend and avoid backend-specific UI branches.

Generate the TypeScript client from `../contracts/openapi/openapi.yaml` with
`mise run generate:typescript`. Run tools through mise as required by the
parent `AGENTS.md`.

CI/CD must reuse these owning mise tasks. Do not push feature work until the
user has signed off its completed behavior and reviewed commit grouping;
consolidate and amend locally during review, following the root Git rules.
