# Image storage capacity

- Keep all local compiler output and BuildKit state on the existing shared
  200 GiB filesystem quota. Databases and uploads stay on their existing volumes.
- Local BuildKit GC targets 40 GiB used, reserves 4 GiB, and seeks 64 GiB free.
  Include an age-independent rule so frequent builds cannot defeat cleanup.
- Before local image builds, remove only unused Bike-owned images older than
  24 hours through Docker's native prune API. Label every Bike build output.
  Never prune containers or volumes. Docker protects images referenced by any
  container, including stopped containers; unrelated images remain untouched.
- Refuse local builds when total loaded image sizes exceed 32 GiB or filesystem
  free space is below 64 GiB. Count unique IDs, conservatively including shared
  layers; report the reason and remedy. Missing/invalid measurements fail closed.
  Run the same check at completion to expose unexpected growth immediately.
- Local image storage is outside the 200 GiB quota. Admission checks and GC
  thresholds are not hard quotas on in-flight image loads. A filesystem-enforced
  image cap requires isolating Docker image storage from persistent volumes.
- Before CI checks and again before publication, require fresh successful storage
  measurements: at most 80 GiB each for BuildKit and native CI cache, 40 GiB for
  registry data, and at least 80 GiB free on their backing disks. Missing, stale,
  ambiguous, or failed observations refuse work; never silently skip the gate.
- CI uses the existing storage collector and Prometheus. Its owning infrastructure
  sets byte-based BuildKit GC and hourly registry retention of five recent release
  roots per Bike repository, plus every deployed/rollback root and its OCI children.
  CI must be drained during registry GC; activate that policy before these gates.
- Gate thresholds reserve room for ordinary builds; they do not interrupt a
  running build or provide a filesystem quota. Any hard-quota migration needs
  separate storage planning, downtime, and verification of persistent-data safety.
- Inspect the resolved native BuildKit policy and run a representative image
  build before rollout. Storage maintenance has no application unit test suite.
