# Bike

![all vibes bike analytic platform](ui-next/public/social-preview.jpg)

Bike is an app that is Strava-esque, but focuses specifically on cross-country (XC) and down-hill (DH) riding.

## Quickstart

Run the development environment:

```sh
mise run compose:up
```

View the [UI](http://localhost:3001/) in a browser.

The Rust Compose project is `bike-rust`. Its default host bindings are API
`3000`, UI `3001`, and Postgres `5432`; optional Jaeger bindings are `16686`,
`4317`, and `4318`. The Go and C# stacks use separate defaults so all three
projects can run together. The task starts the shared `bike-services` map
renderer before the Bike stack; its image cache is shared with Go and C#.

## Local Development

This project uses mise to manage dependencies and act as the task runner. Available tasks can be discovered using:

```sh
mise tasks
```

`mise run generate:protobuf` rebuilds the Strava gateway Rust client bindings
in Cargo's build directory. The checked-in OpenAPI and TypeScript contracts
have `generate:openapi` and `generate:typescript` tasks.

Span export is disabled by default for local development with `OTEL_TRACES_EXPORTER=none`. To inspect traces without the full Grafana stack, start Jaeger:

```sh
mise run tracing:dev
```

For direct `cargo run` development, set `OTEL_TRACES_EXPORTER=otlp` and `OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318`. For Docker Compose API/worker containers, set `OTEL_TRACES_EXPORTER=otlp`, use `OTEL_EXPORTER_OTLP_ENDPOINT=http://jaeger:4318`, and include `--profile tracing`. Jaeger UI will be available at [localhost:16686](http://localhost:16686/).

## Specifications

Natural-language product specifications live in [docs/specs](docs/specs). Specifications are the source of truth for intended behavior. They must be kept in sync with any code changes.
