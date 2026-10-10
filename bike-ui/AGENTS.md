# Bike UI

This is the shared Next.js and React Query frontend for Bike Rust. Make UI,
route, presentation, and generated-client changes here; keep business behavior
in the Rust backend and avoid backend-specific UI branches.

Generate the TypeScript client from `../contracts/openapi/openapi.yaml` with
`mise run generate:typescript`. Run tools through mise as required by the
parent `AGENTS.md`.

CI/CD must reuse these owning mise tasks. Follow the root Git authorization
rules: permission to commit includes publication, integration, and cleanup.
Keep each complete feature in one coherent commit.

Run only focused tests and lints for changed components/hooks locally, in Docker.
Use Vitest file filters and ESLint file paths through mise; keep required prek
checks active. CI/CD is the primary full test/lint gate through `ci:ui:unit` and
the image/E2E workflows. Do not repeat full UI coverage or production builds
locally. Put tests in the existing Vitest test paths so CI discovers them, update
the draft PR continuously, and report focused local results separately from CI.

<!-- BEGIN:nextjs-agent-rules -->

## This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->
