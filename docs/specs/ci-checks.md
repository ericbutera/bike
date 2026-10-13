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
- Preserve the 32 GiB native-cache guard. When it stops compilation, wait for
  coordinated idle retention to reclaim stale targets before retrying CI. Do not
  remove active targets or raise the threshold to make a run pass.
- Validate this retirement by running the actual renderer lint and formatting
  tasks, native Woodpecker validation, and release image-plan resolution in
  Docker. CI owns the full suites, coverage, builds, and browser tests.
