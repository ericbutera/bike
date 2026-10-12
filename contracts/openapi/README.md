# Bike HTTP contract

`openapi.yaml` and `openapi.json` are the canonical HTTP contract in YAML and
JSON formats. Rust's Utoipa definitions generate both files directly here.
The Next.js client, Rust API tests, and browser tests consume these files.

From the repository root, run `mise --cd bike-rs run generate:openapi` to
refresh the contract, then run `mise --cd bike-ui run generate:typescript` to
update the client. Regeneration is explicit developer work. CI and prek do not
rebuild the Rust OpenAPI generator or regenerate the TypeScript client as a
freshness check; normal TypeScript checking and API tests remain enabled.
Keep the six legacy password-auth operations excluded; record an explicit compatibility
decision before removing or renaming another operation.
