package storage

import (
	"context"
	"encoding/json"
	"errors"

	"github.com/ericbutera/bike/strava-gateway/internal/observability"
	"github.com/ericbutera/bike/strava-gateway/internal/webhook"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/propagation"
)

type Inbox struct {
	DB      *pgxpool.Pool
	Targets []string
	Metrics *observability.Metrics
}

// Store commits the event and target work atomically. The HTTP callback can
// acknowledge only after this transaction commits.
func (inbox Inbox) Store(ctx context.Context, event webhook.Event, raw json.RawMessage) (stored bool, err error) {
	carrier := propagation.MapCarrier{}
	otel.GetTextMapPropagator().Inject(ctx, carrier)
	tx, err := inbox.DB.Begin(ctx)
	if err != nil {
		return false, err
	}
	defer rollbackTransaction(ctx, tx, &err)
	var id int64
	err = tx.QueryRow(ctx, `INSERT INTO strava_webhook_events
		(event_key, subscription_id, owner_id, object_id, object_type, aspect_type, event_time, payload, traceparent, tracestate)
		VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT (event_key) DO NOTHING RETURNING id`,
		event.Key(), event.SubscriptionID, event.OwnerID, event.ObjectID, event.ObjectType,
		event.AspectType, event.EventTime, raw, nullableTrace(carrier.Get("traceparent")), nullableTrace(carrier.Get("tracestate"))).Scan(&id)
	if errors.Is(err, pgx.ErrNoRows) {
		return false, tx.Commit(ctx)
	}
	if err != nil {
		return false, err
	}
	rows, err := tx.Query(ctx, `INSERT INTO strava_delivery_outbox (event_id, target, status)
		SELECT $1, target, 'waiting_for_fetch' FROM gateway_site_links
		WHERE athlete_id=$2 AND enabled=true AND target=ANY($3) RETURNING target`, id, event.OwnerID, inbox.Targets)
	if err != nil {
		return false, err
	}
	enqueued := make(map[string]int64)
	for rows.Next() {
		var target string
		if err := rows.Scan(&target); err != nil {
			rows.Close()
			return false, err
		}
		enqueued[target]++
	}
	if err := rows.Err(); err != nil {
		rows.Close()
		return false, err
	}
	rows.Close()
	if err := tx.Commit(ctx); err != nil {
		return false, err
	}
	if inbox.Metrics != nil {
		for site, count := range enqueued {
			inbox.Metrics.RecordDeliveryEnqueued(site, count)
		}
	}
	return true, nil
}

func nullableTrace(value string) *string {
	if value == "" {
		return nil
	}
	return &value
}
