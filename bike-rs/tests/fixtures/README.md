# Shared Rust recording fixtures

- Activity recordings and segment routes used across Rust crates live here.
- Rust tests access reusable recording bytes through
  `bike_core::test_support::recordings`, enabled by unit tests or the existing
  `test-fixtures` feature. Add a named constant there when another recording
  needs to be shared, rather than repeating file paths in consuming tests.
- `recording/` contains synthetic FIT/TCX recording variants; its README describes
  their provenance and scenarios.
- `segment_matching/` retains the real FIT/GPX regression recordings and routes;
  its README lists their expected matches.
- API browser scenario uploads remain in `api/tests/fixtures/platform/uploads/`.
