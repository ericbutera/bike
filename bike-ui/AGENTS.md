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

<!-- BEGIN:nextjs-agent-rules -->

## This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->
