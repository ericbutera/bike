# Bike documentation

Specifications in `specs/` define system behavior. Shared guides explain
development and operations; component READMEs describe local commands and link
to the owning specification.

- [Development](development.md): setup, tooling, configuration, and tracing.
- [Architecture](architecture.md): service boundaries, data flow, and ownership.
- [Current worker flows](plans/worker-current-flows.md): Mermaid diagrams of triggers, inline work, processor handoffs, and Strava delivery.
- [Pipeline visibility plan](plans/pipeline-visibility.md): current-worker DAGs, received-to-available timing, activity task history, Grafana histograms/health metrics, and excessive-work/rebuild anomaly detection.
- [Product specifications](specs/README.md): API, UI, maps, imports, and training behavior.
- [Maps specification](specs/maps.md): private images, caching, the gRPC snapshot
  boundary, tracing, and release acceptance.
- [Personal heatmaps](specs/heatmaps.md): preparation, filters, and private tiles.
- [Deployment](deployment.md): CI, image promotion, and operational verification.
- [Observability metrics](observability-metrics.md): historical metric design and the infrastructure catalog.
- [Worker pipeline diagnostics](production-failures.md): anomalies, processor budgets, capacity and deployment acceptance.
- [Active backlog](TODO.md): task status and remaining verification.
- [HTTP contract](../contracts/openapi/README.md): canonical generated YAML and JSON, plus client generation.
