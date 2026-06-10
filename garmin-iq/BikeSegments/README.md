# Bike Segments Connect IQ App

This is a small Garmin Connect IQ watch app prototype that:

- syncs segment geometry and goals from bike
- warns when you are approaching a segment
- auto-detects segment start and completion
- shows realtime progress and time delta versus your goal/PR while riding

## What this does today

- Polls GPS once per second.
- Pulls segments from bike using a device link flow with refresh/access tokens.
- Chooses the nearest segment and enters states:
  - `idle`
  - `approaching`
  - `active`
  - `completed`
- During `active`, computes:
  - elapsed time
  - progress meters
  - realtime delta seconds against goal/PR pacing
- During `completed`, shows a 15-second result card with time, PR/KOM delta,
  and rank when the sync payload includes top-10 or leaderboard data.

## Expected bike endpoint

The app expects:

- `GET /api/garmin-iq/link/begin`
- `POST /api/garmin-iq/link/complete`
- `GET /api/garmin-iq/link/poll`
- `GET /api/garmin-iq/auth/refresh`
- `GET /api/garmin-iq/segments/sync`
- response shape documented in `docs/bike-garmin-iq-sync-contract.json`

## Configure endpoint

The checked-in app defaults to production:

- API: `https://bike.nibelheim.dev`
- approval flow: `https://bike.nibelheim.dev/account`

The VS Code debug launch swaps in `resources/strings/strings.debug.xml` for the
build so the emulator uses:

- API: `http://localhost:3000`
- approval flow: `http://localhost:3001/account`

If the local API is unavailable, the app falls back to production for the
current session.

## Login/auth flow for watch sync

1. Build and deploy the app.
2. Open the app on watch/emulator and wait for a pairing code.
3. Open the Bike account page shown by the app.
4. In the **Garmin IQ linking** section, enter the pairing code and approve.
5. Return to the watch and wait for the link to complete.
6. Segment sync then runs automatically.

For local development, start the Bike compose stack so the API is available on
`http://localhost:3000` and the account UI is available on
`http://localhost:3001/account` before launching the debug profile.

The API stores only hashed refresh/access credentials. Linked devices can be
revoked from the account page.

For best on-device ranking, segment sync items may include either
`leaderboard_seconds` or `top_10_seconds` as an array of sorted leaderboard
durations. The app also accepts scalar `top10_seconds` as a top-10 cutoff.

## Build

Prereqs:

- Garmin Connect IQ SDK installed
- `monkeyc` available in PATH

From this folder:

```bash
monkeyc -f monkey.jungle -o build/BikeSegments.prg -y /path/to/developer_key.der
```

Run in simulator:

```bash
monkeydo build/BikeSegments.prg fenix7
```

## Notes

- This app is focused on realtime feedback loops while riding.
- Consider snapping progress using along-route projection for tighter timing on dense switchbacks.
