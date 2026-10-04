# Bike HTTP contract

`openapi.yaml` and `openapi.json` are the shared HTTP contract used to generate
the Next.js client. Rust's Utoipa output in `../../bike-rs/docs/openapi/` is the
canonical producer.

From the repository root, run `mise --cd bike-rs run generate:openapi` to
refresh these distribution copies, then run
`mise --cd bike-ui run generate:typescript` to update the client. Keep the six
legacy password-auth operations excluded; record an explicit compatibility
decision before removing or renaming another operation.
