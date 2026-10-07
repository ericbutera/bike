# Bike backend

The Rust application backend for [Bike](../README.md), built with Axum, SeaORM,
and PostgreSQL. It owns activity ingestion, segment matching, training analytics,
accounts, admin operations, and durable background processing.

| Workspace member | Responsibility                                                |
| ---------------- | ------------------------------------------------------------- |
| `api`            | HTTP API, authentication integration, rider/admin endpoints   |
| `bike-core`      | Domain models, queries, services, parsers, platform modules   |
| `worker`         | Import processing, analytics rebuilds, background maintenance |
| `migration`      | Append-only SeaORM schema migrations                          |

## Run locally

Start the complete stack from the repository root:

```sh
cd ..
mise run compose:up
```

Open [localhost:3001](http://localhost:3001). The root stack includes the frontend,
PostgreSQL, API, worker, and renderer. See the
[development guide](../docs/development.md) for configuration and tracing.

## Backend checks

From this directory:

```sh
mise install
mise run fmt:check
mise run lint
mise run test
```

Use `mise exec -- cargo test -p bike-core <test_filter>` for a focused regression.
Run direct API/worker processes with `api:dev` and `worker:dev` after configuring
the database and other values from [`.env.example`](.env.example).

## Contracts and specifications

`mise run generate:openapi` generates the canonical HTTP contract directly in
`../contracts/openapi/`. `mise run openapi:check` verifies it against fresh Rust
output. `mise run generate:typescript` regenerates the frontend
client. `mise run generate:protobuf` checks the core crate and rebuilds the gateway
Rust client bindings in Cargo's build directory.

[Product specifications](../docs/specs/README.md) describe intended behavior and
should change with it. Track unfinished work in the root [backlog](../docs/TODO.md).
