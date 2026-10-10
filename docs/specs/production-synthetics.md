# Synthetic monitoring and browser test authentication

Bike uses one shared synthetic credential on the existing API and UI services.
It does not add an identity provider, a token exchange, a second UI deployment,
or per-user authentication to the renderer.

## Authentication boundary

`X-Bike-Synthetic-Key` is checked in the API authentication extractor against
`BIKE_SYNTHETIC_KEY` using constant-time HMAC verification. Configuration is
opt-in and requires a credential of at least 32 characters. Missing configuration
or an invalid credential fails authentication.

The public ingress controller removes this header before forwarding requests,
including requests to the UI. Pulumi owns the header ConfigMap and the patch to
the existing ingress controller configuration. The header name is specific to
Bike; ordinary user authorization is preserved. API authentication also rejects
synthetic credentials on requests containing `X-Forwarded-For` or `Forwarded`.
The configured ingress always supplies its own forwarding header; callers
cannot suppress it by sending an empty value. Next.js adds `X-Forwarded-For`
even to direct internal requests, so the UI relies on ingress header removal
and rejects the explicit `Forwarded` header. Its API fetch carries only the
credential and ordinary user/trace headers, not proxy forwarding headers.

Synthetic authentication selects the scenario's server-resolved owner. It
accepts only the explicit GET routes needed for current-user discovery,
preferences, activities, import status, segments, comparison, and the manifest.
The read-only activity-map image GET is included and checks the resolved
owner before cache reads or conditional responses, as specified in
[the maps contract](maps.md#private-http-image-contract).
Logout is excluded despite using GET. Existing ownership filters continue to
apply. Admin, verified-user, API-client, and write routes reject this identity.

The owner remains disabled and uses the reserved `bike-synthetics` provider
marker. Ordinary user extractors, OAuth account linking, and session creation
reject it, including if an administrator accidentally enables the account.
Internal synthetic access also fails if the owner becomes enabled or an admin.

## Fixture lifecycle

`synthetic_scenarios` maps `platform-smoke/v1` to its local owner, activity,
segment, and two effort IDs. API startup provisions the scenario transactionally
when the credential is configured. PostgreSQL serializes concurrent starts with
an advisory transaction lock. Repeated starts validate ownership and reuse the
same records. Invalid or missing registered records fail rather than being
replaced with another owner's data.

The synthetic dataset contains two small rides, one segment, two race efforts,
and current segment summaries. It has no provider connection, upload jobs, or
intentional failed tasks. Its identities and database sequences are independent
in every environment. A conflict with the reserved synthetic email fails
provisioning without adopting that account.

The authenticated manifest endpoint is outside the public OpenAPI contract.
Callers discover IDs immediately before testing. They do not supply an owner ID
or select a real user's account. Normal segment reads may refresh stale
analytics; ownership filters keep such maintenance within the synthetic dataset.

## Rendering and verification

The UI server forwards the internal credential only to Bike's API. Rust checks
the synthetic identity and activity ownership before reading its PNG cache or
returning 304. On a miss, Rust sends allowed geometry to the Go snapshot worker
over gRPC using the map service token and W3C trace context. The UI has no worker
token. Synthetic credentials and user identity are never forwarded to the worker.
The [maps specification](maps.md) owns this transport and visibility contract.

The k6 availability image makes only API health and UI HTML requests. It does
not use this credential or dataset. It runs inside the VPC with explicit URLs
and no Kubernetes access. The standalone Playwright image owns authenticated
browser journeys, using an explicitly prepared environment and injected
configuration. Native HTTP tests own authorization regression coverage.

The [TEST11 gate](../E2E-TODO.md) uses disposable local/CI environments and normal
session/token authentication for connected write and admin journeys. This
production synthetic credential remains read-only; it is not the authentication
mechanism for those mutating E2E scenarios.

The connected map regression requests a PNG through the real UI proxy, Rust
API, and Go/Chromium worker with frozen external basemaps. It verifies a fresh
miss, an identical cache hit, and ETag/304. Mocked browser/API scenarios do not
replace this boundary check. Each disposable baseline reset restores upload
and map-cache ownership for the nonroot API without restarting services.

Disposable PostgreSQL readiness waits for the permanent server's native ready
state before issuing an authenticated TCP probe. Migration and API startup
depend on that readiness. Initialization, browser, and service warnings/errors
fail the gate; startup races must be fixed rather than filtered from diagnostics.

Playwright remains a separate browser check. For an internal production run it
discovers the same manifest and directs the deployed UI's API transport to the
existing internal API, preserving real application responses. External browser
basemaps are fixtures. Credential-bearing browser traces are disabled. That run
does not establish public ingress, SSO, or provider behavior.

Operational instructions live in [the integration-test guide](../../integration-tests/README.md).
Remaining work is tracked in [the backlog](../TODO.md).
