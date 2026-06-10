# Bike <-> Garmin IQ Integration Plan

## Goal

Provide low-latency segment feedback while riding:

- approaching segment warning
- auto-start and auto-complete detection
- realtime delta versus target split

## Recommended bike API endpoint

- Method: `GET`
- Path: `/api/garmin-iq/segments/sync`
- Auth: `Authorization: Bearer <bike_iq_token>`
- Response: see `bike-garmin-iq-sync-contract.json`

## Suggested backend shaping

For each user segment in `segments`:

- Include `id`, `title`, `distance_meters`
- Include `route_points` from persisted route geometry
- Include `pr_seconds` from `segment_user_summaries.personal_best_duration_seconds`
- Include optional `goal_seconds` from a future per-segment goal table or user preference
- Include `approach_meters` defaulted to 60-120m depending on expected speed

## Token model

Use dedicated watch tokens instead of session cookies:

- table: `user_device_tokens`
- fields:
  - `id`
  - `user_id`
  - `label`
  - `token_hash`
  - `created_at`
  - `expires_at`
  - `last_used_at`
  - `revoked_at`

Rules:

- never store raw token
- hash with a strong password hash or keyed HMAC
- support multiple active tokens per user/device
- support token revoke and rotation

## Sync and freshness

- Watch sync on app open and then every 5-15 minutes.
- API should return cache-friendly JSON under ~250KB for reliable phone relay.
- Keep segment count small for watch UX, for example:
  - starred segments only
  - or nearest N segments to home trailhead

## Realtime matching guidance

On watch:

- Enter `approaching` when nearest route-point distance <= `approach_meters`
- Enter `active` when distance to first route point <= 20m
- Track progress by nearest route point cumulative distance
- Mark complete when progress >= `distance_meters - finish_tolerance`

Defaults:

- `start_tolerance_m`: 20
- `finish_tolerance_m`: 12
- `approach_meters`: 80

## Risk controls

- GPS jitter can produce false starts on tight switchbacks; consider heading checks and minimum forward progress.
- Keep watch logic tolerant to temporary GPS loss.
- Avoid excessive web sync frequency to preserve battery.
