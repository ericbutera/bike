# CI checks

- Local Docker checks, prek hooks, and CI call the owning mise tasks with pinned
  tooling. Formatter, compiler, and linter warnings fail checks.
- `lint:maps` and `format:check` cover the renderer and shared JavaScript under
  `map-renderer/` and `scripts/`. Require every selected source pattern to exist;
  do not suppress unmatched paths or skip validation when files are removed.
- The retired standalone k6 synthetic suite has no tooling pins, tasks, hooks,
  lint patterns, workflow path filters, or image targets.
- Native API integration tests remain in `bike-rs/api/tests`; browser regressions
  remain in `bike-ui/tests/e2e` and run through their owning tasks.
- The internal synthetics monitor is retired: scenario discovery, startup
  provisioning, special authentication, UI credential forwarding, dedicated
  tests, and monitor tasks are removed. The API exposes no synthetics routes.
- Authentication uses ordinary user sessions, API-client tokens, or explicitly
  enabled local-admin mode. Disabled accounts follow the normal rejection rules.
- Historical migrations remain append-only, including the former scenario table
  creation. The application no longer reads or writes that table.
- Ordinary generated test fixtures continue to cover authentication, uploads,
  activities, segments, maps, and races in the owning suites.
- Preserve the 32 GiB native-cache guard. When it stops compilation, wait for
  coordinated idle retention to reclaim stale targets before retrying CI. Do not
  remove active targets or raise the threshold to make a run pass.
- Validate this retirement by running the actual renderer lint and formatting
  tasks, native Woodpecker validation, and release image-plan resolution in
  Docker. CI owns the full suites, coverage, builds, and browser tests.
