# Strava SDK

- `vendor-swagger.json` is the Strava-owned Swagger snapshot from
  https://developers.strava.com/swagger/swagger.json (2026-10-11).
- OpenAPI Generator generates the SDK in `bike-rs/strava-sdk` using
  its standard templates and supported `supportMiddleware` option. The image
  and digest are pinned in `mise.toml`.
- The SDK is generated output: never hand-edit it, override its templates, or
  rewrite its operations or models. `strava:sdk:check` compares the complete
  source and manifest against a fresh generation.
- `vendor/` freezes the vendor's referenced schemas beside the Swagger snapshot.
  Generation resolves these local snapshots without downloading mutable models.
- `prepare.jq` selects the vendor operations Bike uses, preserves their request
  and response definitions, merges the additional documented operations, and
  resolves local schema paths. Generator name mappings escape Rust identifiers
  without changing JSON field names or wire encoding.
- Optional generator schema normalization is disabled to preserve the vendor
  schemas. This also avoids its invalid self-reference check on primitive array
  items; generator warnings and errors fail both generation and freshness checks.
- Bike's formatting tasks explicitly select its four owned crates and leave the
  generated SDK bytes untouched.
- `oauth-webhooks.json` describes the additional endpoints documented at
  https://developers.strava.com/docs/authentication/ and
  https://developers.strava.com/docs/webhooks/; these are absent from Strava's
  published Swagger document.
- Bike's wrapper reserves quota and reuses saved responses before invoking the
  SDK. Its supported middleware records the encoded request and response before
  SDK decoding, preserves the response status, version, headers, extensions,
  and exact body bytes, and leaves the outbound request untouched.
- Full exchange evidence is encrypted because requests contain access tokens
  or client secrets. Activity response bodies also have immutable raw objects
  for replay; all transforms have separate objects.
- Run `mise run strava:sdk` to regenerate; `mise run strava:sdk:check` checks
  that committed bindings match these inputs. Both run the pinned generator in
  a disposable Docker container.
