# Account And Integrations Specification

The account area manages rider preferences and external connections. Preferences change how Bike interprets training data. Integrations control how new data enters or leaves the Bike system.

## Product Intent

The rider should understand which services are connected, which devices are linked, and which personal settings affect training analysis. Connection state should be explicit and recoverable.

## User Preferences

User preferences include:

- unit system: `imperial`, `metric`, or `mixed`;
- estimated FTP in watts;
- heart-rate zone bounds;
- active XC event target fields;
- XC backfill status when the target start date changes.

Unit preferences affect display. Training computations should store canonical metric values and format for the rider at the edge.

Heart-rate zone and FTP values are user-provided training context. They should be validated before save and used consistently by activity summaries, zone displays, and training analysis.

## XC Event Target Preferences

The current XC event target is stored on preferences. The event target is described in detail by [XC Event Readiness](xc-event-readiness.md).

Saving a new XC training start date queues a historical backfill. Clearing the start date clears backfill state.

## Strava

Strava connection is initiated from the account page. The API returns an authorization URL, and the callback exchanges the authorization code for connection state.

Bike requires enough Strava scope to import private "Only You" activities. Existing connections with insufficient scope should reconnect before private rides are expected to sync.

After connection, Bike queues an initial sync. The rider can also manually queue a re-sync. Disconnecting removes the connection unless a user-scoped Strava sync is queued or running.

Strava integration events should record meaningful connection, sync, webhook, and error history for both user feedback and admin debugging.

Runtime configuration must set `STRAVA_CLIENT_ID` and `STRAVA_CLIENT_SECRET` for both the API and worker. `STRAVA_OAUTH_SCOPES` defaults to `activity:read_all`. The Strava application callback URL should point at `API_URL/api/strava/callback`; in the default local compose setup, that is `http://localhost:3000/api/strava/callback`.

## Garmin IQ

Garmin IQ linking uses a device pairing flow:

1. the watch requests a pairing code with its install id;
2. Bike shows a verification URL and short-lived code;
3. the rider approves the code from the account page;
4. the watch polls for linked state and receives refresh/access tokens once;
5. the watch refreshes access tokens and syncs segment data with Bike.

Pairing codes are short-lived. Reset and unlink flows revoke device credentials and clear token material.

Garmin IQ segment sync returns a bounded set of segment data with route points and timing goals such as PR, KOM, and last attempt when available. Sync should favor compact, watch-appropriate payloads.

## Security And Ownership

Account preferences, Strava connection state, and Garmin IQ devices are scoped to the authenticated user. Device install ids, access tokens, refresh tokens, and pairing codes must not create cross-user access.

Secrets and token material are configuration or database concerns, not frontend environment values.

## Code Anchors

- Preferences API: `api/src/controllers/user_preferences.rs`
- Training profile validation: `api/src/training_profile.rs`
- Strava API: `api/src/controllers/strava.rs`
- Strava service: `api/src/strava.rs`
- Garmin IQ API: `api/src/controllers/garmin_iq.rs`
- Account UI: `ui-next/app/account/page.tsx`
- Garmin IQ watch contract: `garmin-iq/docs/bike-garmin-iq-sync-contract.json`

## Open Gaps

- Decide whether user-created event templates belong in preferences or a separate event-target table.
- Keep reconnect guidance clear when Strava scopes are insufficient.
- Keep Garmin IQ payload limits explicit as segment sync grows.
