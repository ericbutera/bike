# Bike HTTP contract

`openapi.yaml` and `openapi.json` are the canonical HTTP contract in YAML and
JSON formats. Rust's Utoipa definitions generate both files directly here.
The Next.js client, Rust API tests, and browser tests consume these files.

From the repository root, run `mise --cd bike-rs run generate:openapi` to
refresh the contract, then run `mise --cd bike-ui run generate:typescript` to
update the client. `mise --cd bike-rs run openapi:check` checks both files
against fresh Rust output in a temporary directory; CI and prek run this check.
Keep the six legacy password-auth operations excluded; record an explicit compatibility
decision before removing or renaming another operation.
