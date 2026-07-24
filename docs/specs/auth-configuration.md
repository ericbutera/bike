# Bike Auth Configuration Spec

## Purpose

Bike uses Kaleido auth in mixed mode. User/password auth stays enabled for now, and OAuth providers are added when their API environment variables are configured.

This lets Bike test OAuth without breaking existing password users.

## Route Shape

Bike mounts Kaleido auth as:

- `/api/auth/register`
- `/api/auth/login`
- `/api/auth/current`
- `/api/auth/refresh`
- `/api/auth/logout`
- `/api/auth/resend-confirmation`
- `/api/auth/verify/{token}`
- `/api/auth/forgot`
- `/api/auth/reset`
- `/api/oauth/providers`
- `/api/oauth/{provider}`
- `/api/oauth/{provider}/callback`

## API Configuration

Required base settings:

| Variable | Required | Purpose |
| --- | --- | --- |
| `FRONTEND_URL` | yes | External frontend origin. OAuth success redirects to `{FRONTEND_URL}/auth/callback`. |
| `API_URL` | yes | External API origin without `/api`. Kaleido builds callbacks as `{API_URL}/api/oauth/{provider}/callback`. |
| `JWT_SECRET` | yes | Stable signing secret for sessions. |
| `CORS_ALLOWED_ORIGINS` | yes | Must include Bike's frontend origin. |
| `AUTH_PASSWORD_ENABLED` | no | Defaults to `true`. Set `false` to disable password login/recovery. |
| `AUTH_REGISTRATION_ENABLED` | no | Defaults to `true`. Set `false` to disable new password registrations. |

Local development:

```yaml
FRONTEND_URL: http://localhost:3001
API_URL: http://localhost:3000
JWT_SECRET: change_me_in_dev
CORS_ALLOWED_ORIGINS: http://localhost:3001
AUTH_PASSWORD_ENABLED: "true"
AUTH_REGISTRATION_ENABLED: "true"
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

Bike passes runtime config through `ui-next/components/Providers.tsx`:

```tsx
const authConfig = {
  passwordAuthEnabled: config.AUTH_PASSWORD_ENABLED,
  registrationEnabled: config.AUTH_REGISTRATION_ENABLED,
  OAuthProviderButtons: auth.createOAuthProviderButtons(config.API_URL),
};
```

Kaleido's shared OAuth buttons fetch `/api/oauth/providers` and render whatever the API detects.

## Environment Split

Bike uses two API URL meanings:

- API service `API_URL`: public API origin without `/api`, used for OAuth callback generation.
- UI service `API_URL`: public API route prefix with `/api`, used by browser clients and OAuth buttons.

Example local setup:

```yaml
api:
  API_URL: http://localhost:3000

ui-next:
  API_URL: http://localhost:3000/api
```

## Deployment Checklist

- Set `FRONTEND_URL` to the public Bike UI origin.
- Set backend `API_URL` to the public Bike API origin without `/api`.
- Set frontend `API_URL` to the public Bike API prefix with `/api`.
- Set `JWT_SECRET` to a stable secret.
- Keep `AUTH_PASSWORD_ENABLED=true` for the first OAuth rollout.
- Configure at least one `OAUTH_<PROVIDER>_*` provider in the API environment.
- Confirm `GET /api/oauth/providers` returns the expected provider list.
- Register the exact provider callback URL with each external provider.
- Confirm password login still works.
- Confirm `/api/oauth/{provider}` redirects to the provider or completes local dev login.

## Non-Goals

- Bike does not store OAuth client secrets in the database.
- Strava OAuth is an activity integration and is configured separately from Kaleido account auth.
