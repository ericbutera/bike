# Bike documentation

Shared guides and product specifications live here. Component READMEs describe
their local commands and link to the owning guide or specification.

- [Development](development.md): setup, tooling, configuration, and tracing.
- [Architecture](architecture.md): service boundaries, data flow, and ownership.
- [Current worker flows](plans/worker-current-flows.md): Mermaid diagrams of triggers, inline work, processor handoffs, and Strava delivery.
- [Pipeline visibility plan](plans/pipeline-visibility.md): current-worker DAGs, received-to-available timing, activity task history, Grafana histograms/health metrics, and excessive-work/rebuild anomaly detection.
- [Product specifications](specs/README.md): API, UI, maps, imports, and training behavior.
- [Maps](maps.md): rendering and personal heatmaps.
- [Deployment](deployment.md): CI, image promotion, and operational verification.
- [Observability metrics](observability-metrics.md): historical metric design and the infrastructure catalog.
- [Worker pipeline diagnostics](production-failures.md): anomalies, processor budgets, capacity and deployment acceptance.
- [Active backlog](TODO.md): task status and remaining verification.
- [HTTP contract](../contracts/openapi/README.md): canonical generated YAML and JSON, plus client generation.
