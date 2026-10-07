# Bike Auth Configuration Spec

## Purpose

Bike's target account-auth model is local development auto-login and production
SSO/OIDC. Password login, registration, confirmation, and recovery are not part
of the target shared-UI experience. Existing password data is retained during
the migration; disabling these routes does not delete account data.

The Rust API mounts Bike-owned session and OAuth routes from
`bike-rs/bike-core/src/auth/`; password routes are absent. Bike-owned account and
session follow-up is tracked as AUTH01–AUTH04 in
[the single backlog](../TODO.md). The shared UI owns session state in `bike-ui/lib/auth.tsx` and provider buttons
in `bike-ui/components/Providers.tsx`.

## Route Shape

Bike exposes these session routes and its configured OAuth providers:

- `/api/auth/current`
- `/api/auth/refresh`
- `/api/auth/logout`
- `/api/oauth/providers`
- `/api/oauth/{provider}`
- `/api/oauth/{provider}/callback`

## API Configuration

Set these base settings explicitly in production; `bike-rs/bike-core/src/config.rs`
provides development defaults. The shared UI has its own runtime settings below:

| Variable               | Production | Purpose                                                                                                                                                                   |
| ---------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `FRONTEND_URL`         | yes        | External frontend origin. OAuth success redirects to `{FRONTEND_URL}/auth/callback`.                                                                                      |
| `API_URL`              | yes        | External API origin without `/api`. Bike builds callbacks as `{API_URL}/api/oauth/{provider}/callback`.                                                                   |
| `JWT_SECRET`           | yes        | Stable signing secret for sessions.                                                                                                                                       |
| `CORS_ALLOWED_ORIGINS` | yes        | Must include Bike's frontend origin.                                                                                                                                      |
| `APP_ENV`              | no         | Defaults to `local`; `production` and `prod` disable local-admin defaults.                                                                                                |
| `LOCAL_ADMIN_ENABLED`  | no         | Defaults to `true` outside production. When enabled, protected API requests use the seeded local development user without a session cookie. It is rejected in production. |

Local development:

```yaml
FRONTEND_URL: http://localhost:3001
API_URL: http://localhost:3000
JWT_SECRET: change_me_in_dev
CORS_ALLOWED_ORIGINS: http://localhost:3001
LOCAL_ADMIN_ENABLED: "true"
```

## OAuth Provider Configuration

OAuth providers are discovered from API env vars. The UI does not need an OAuth feature flag.

Use `OAUTH_PROVIDERS` only to control order:

```yaml
OAUTH_PROVIDERS: dev,acme
```

### Local Dev Provider

Use `dev` for local Docker development only:

```yaml
OAUTH_DEV_ENABLED: "true"
OAUTH_DEV_EMAIL: developer@bike.local
OAUTH_DEV_NAME: Local Developer
OAUTH_DEV_SUBJECT: bike-local-dev
```

When `LOCAL_ADMIN_ENABLED` is true, the API creates or reuses the local dev
provider user, grants that user admin access, and resolves protected requests
as that user without requiring a browser session. This is intended for local
development and parity testing only. Set `LOCAL_ADMIN_ENABLED=false` when
testing real session authentication.

### OIDC Discovery Provider

```yaml
OAUTH_ACME_LABEL: Acme SSO
OAUTH_ACME_CLIENT_ID: ...
OAUTH_ACME_CLIENT_SECRET: ...
OAUTH_ACME_ISSUER_URL: https://idp.example.com
```

Register this callback URL with the provider:

```text
{API_URL}/api/oauth/acme/callback
```

### Explicit Endpoint Provider

```yaml
OAUTH_INTERNAL_CLIENT_ID: ...
OAUTH_INTERNAL_CLIENT_SECRET: ...
OAUTH_INTERNAL_AUTH_URL: https://login.example.com/oauth/authorize
OAUTH_INTERNAL_TOKEN_URL: https://login.example.com/oauth/token
OAUTH_INTERNAL_USERINFO_URL: https://login.example.com/oauth/userinfo
```

## UI Configuration

Bike passes runtime config through `bike-ui/components/Providers.tsx`:

```tsx
const authConfig = {
  OAuthProviderButtons: createOAuthProviderButtons(config.API_URL),
};
```

The shared UI's Bike-owned OAuth buttons fetch `/api/oauth/providers` and render whatever the API detects.

## Environment Split

Bike uses two API URL meanings:

- API service `API_URL`: public API origin without `/api`, used for OAuth callback generation.
- UI service `API_URL`: public API route prefix with `/api`, used by browser clients and OAuth buttons.

Example local setup:

```yaml
api:
  API_URL: http://localhost:3000

bike-ui:
  API_URL: http://localhost:3000/api
```

## Production configuration

Each site owns its frontend/API origin, JWT signing secret, OIDC client and exact
callback binding. Set `APP_ENV=production`, `LOCAL_ADMIN_ENABLED=false`, and
disable `OAUTH_DEV_ENABLED`. The API rejects production startup with local-admin
bypass enabled. Set UI `LOCAL_ADMIN_AUTO_LOGIN=false`; normal sign-in uses the
discovered SSO provider. `AUTH_PASSWORD_ENABLED` and
`AUTH_REGISTRATION_ENABLED` are obsolete and are not consumed by the shared UI.

Use the existing component workflows and independently owned site stacks in the
[deployment guide](../../../pulumi-iac/BIKE-VARIANTS.md). AUTH04 verifies auth
and session behavior independently with fakes and Playwright as recorded below;
a complete live SSO-to-Strava chain is not required.

## Disabled accounts

AUTH01 (`2694699`, 2026-10-01) uses the existing `users.disabled` column in
the Rust model and canonical admin/current-user response schemas. Admin lists
filter before pagination. The disable endpoint defaults to disabling, persists
an explicit enable/disable value, and rejects self-disable. Disabled users cannot
authenticate through local-admin, sessions, or SSO, and SSO cannot change their
provider binding. Protected requests return 403 with `this account is disabled`;
refresh returns 401. Re-enabling restores access, including unexpired sessions.

The existing Rust auth suite reproduced the missing `disabled` response
field, then passed all six auth cases. Strict bike-core Clippy passed. These
checks cover persisted status, filtering, issuance, refresh, provider binding,
disabled API access, and re-enabling. The canonical OpenAPI copies and shared
TypeScript client were regenerated.

## Refresh-token consumption

AUTH02 (`ad6f4f2`, 2026-10-01) requires a single successful database delete of an
unexpired token before issuing its successor. A concurrent loser or replay
returns an authentication failure. Disabled users are checked before consumption.
Expired tokens remain unusable, and a successful successor can rotate again.

The added Rust auth case reproduced two concurrent successes before the fix
and one afterward; all ten auth cases and strict bike-core Clippy passed.
No schema change or additional fixture harness was required.

## OAuth callback validation

AUTH03 (`64c7de0`, 2026-10-01) stores a SHA-256 state hash, provider, random nonce,
and ten-minute expiry. Callbacks require the matching HttpOnly browser cookie
(`Path=/api/oauth`, `SameSite=Lax`, secure on HTTPS), matching provider, and an
unexpired stored challenge. A conditional delete permits one consumer and rejects
replay, including concurrent use. Successful callbacks clear the challenge cookie.

OIDC providers return a signed ID token with the expected issuer, client audience,
lifetime, and nonce, following
[OpenID Connect validation](https://openid.net/specs/openid-connect-core-1_0.html#IDTokenValidation).
Rust uses `openidconnect`. Plain OAuth2 providers use code/userinfo exchange with
browser-bound state. The local development shortcut is restricted to explicitly
enabled development configuration.

## Verification

Rust auth tests use fake identity providers and in-memory fixtures. The UI
`auth-happy-path.spec.mjs` checks sign-in return, session reload, and logout
with a fake application API boundary. These are independent of live SSO.
Run the owning tests through mise; browser instructions live with the UI suite.

## Non-Goals

- Bike does not store OAuth client secrets in the database.
- Strava OAuth is an activity integration and is configured separately from Bike account auth.
