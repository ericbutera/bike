INSERT INTO integration_events (user_id,provider,event_type,level,message,payload,created_at)
SELECT binding.user_id,'strava','gateway.delivery.completed','info',
       'Completed a retained Strava gateway receipt.',
       jsonb_build_object('delivery_id',receipt.delivery_id,'athlete_id',receipt.athlete_id,
                          'strava_activity_id',CASE WHEN receipt.operation<>'deauthorize' THEN receipt.activity_id END,
                          'operation',receipt.operation,'event_time',receipt.event_time,
                          'historical_receipt',true),
       receipt.completed_at
FROM strava_gateway_receipts receipt
JOIN strava_gateway_bindings binding ON binding.athlete_id=receipt.athlete_id
WHERE receipt.status='completed' AND receipt.completed_at IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM integration_events event
      WHERE event.provider='strava'
        AND event.event_type IN ('gateway.delivery.applied','gateway.delivery.completed','gateway.delivery.ignored')
        AND event.payload->>'delivery_id'=receipt.delivery_id
  );
