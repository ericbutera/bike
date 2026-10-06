package storage

import (
	"context"
	"fmt"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/jackc/pgx/v5/pgxpool"
)

const (
	quotaOverall15MinuteEnv = "STRAVA_QUOTA_OVERALL_15M_LIMIT"
	quotaOverallDailyEnv    = "STRAVA_QUOTA_OVERALL_DAILY_LIMIT"
	quotaRead15MinuteEnv    = "STRAVA_QUOTA_READ_15M_LIMIT"
	quotaReadDailyEnv       = "STRAVA_QUOTA_READ_DAILY_LIMIT"
)

// Quota is app-wide. Strava's non-upload requests count against both the
// overall and non-upload buckets, each with a 15-minute and UTC-day window.
type Quota struct {
	DB     *pgxpool.Pool
	Limits [4]int // overall 15m/day, non-upload 15m/day
	Now    func() time.Time
}

type bucket struct {
	name  string
	limit int
	reset time.Time
}

type QuotaSnapshot struct {
	Bucket  string
	Limit   int
	Used    int
	ResetAt time.Time
}

// ParseQuotaLimits reads optional positive overrides in the order used by Quota.Limits.
// Missing or empty values retain buckets()' existing defaults.
func ParseQuotaLimits(lookup func(string) (string, bool)) ([4]int, error) {
	keys := [4]string{
		quotaOverall15MinuteEnv, quotaOverallDailyEnv,
		quotaRead15MinuteEnv, quotaReadDailyEnv,
	}
	var limits [4]int
	for index, key := range keys {
		raw, exists := lookup(key)
		if !exists || strings.TrimSpace(raw) == "" {
			continue
		}

		limit, err := strconv.Atoi(strings.TrimSpace(raw))
		if err != nil || limit <= 0 {
			return [4]int{}, fmt.Errorf("%s must be a positive integer", key)
		}
		limits[index] = limit
	}

	return limits, nil
}

func (quota Quota) Snapshot(ctx context.Context) ([]QuotaSnapshot, error) {
	rows, err := quota.DB.Query(ctx, `SELECT bucket,limit_count,used_count,reset_at FROM gateway_rate_limits ORDER BY bucket`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var snapshots []QuotaSnapshot
	for rows.Next() {
		var snapshot QuotaSnapshot
		if err := rows.Scan(&snapshot.Bucket, &snapshot.Limit, &snapshot.Used, &snapshot.ResetAt); err != nil {
			return nil, err
		}
		snapshots = append(snapshots, snapshot)
	}
	return snapshots, rows.Err()
}

func (quota Quota) BlockedBuckets(ctx context.Context) ([]string, error) {
	rows, err := quota.DB.Query(ctx, `SELECT bucket FROM gateway_rate_limits
		WHERE used_count>=limit_count AND reset_at>now() ORDER BY bucket`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var buckets []string
	for rows.Next() {
		var bucket string
		if err := rows.Scan(&bucket); err != nil {
			return nil, err
		}
		buckets = append(buckets, bucket)
	}
	return buckets, rows.Err()
}

func (quota Quota) buckets() [4]bucket {
	now := time.Now().UTC()
	if quota.Now != nil {
		now = quota.Now().UTC()
	}
	limits := quota.Limits
	defaults := [4]int{200, 2000, 100, 1000}
	for index := range limits {
		if limits[index] <= 0 {
			limits[index] = defaults[index]
		}
	}
	quarter := now.Truncate(15 * time.Minute).Add(15 * time.Minute)
	day := now.Truncate(24 * time.Hour).Add(24 * time.Hour)
	return [4]bucket{
		{"nonupload_15m", limits[2], quarter},
		{"nonupload_day", limits[3], day},
		{"overall_15m", limits[0], quarter},
		{"overall_day", limits[1], day},
	}
}

// Acquire returns the earliest time at which all exhausted buckets reset. A
// zero time means that one request unit was reserved from all four buckets.
func (quota Quota) Acquire(ctx context.Context) (next time.Time, err error) {
	tx, err := quota.DB.Begin(ctx)
	if err != nil {
		return time.Time{}, err
	}
	defer rollbackTransaction(ctx, tx, &err)
	buckets := quota.buckets()
	now := time.Now().UTC()
	if quota.Now != nil {
		now = quota.Now().UTC()
	}
	var retryAt time.Time
	for _, item := range buckets {
		if _, err := tx.Exec(ctx, `INSERT INTO gateway_rate_limits (bucket, limit_count, reset_at)
			VALUES ($1,$2,$3) ON CONFLICT (bucket) DO NOTHING`, item.name, item.limit, item.reset); err != nil {
			return time.Time{}, err
		}
		var limit, used int
		var reset time.Time
		if err := tx.QueryRow(ctx, `SELECT limit_count, used_count, reset_at FROM gateway_rate_limits
			WHERE bucket=$1 FOR UPDATE`, item.name).Scan(&limit, &used, &reset); err != nil {
			return time.Time{}, err
		}
		if !reset.After(now) {
			used, reset = 0, item.reset
		}
		if used >= limit && reset.After(retryAt) {
			retryAt = reset
		}
		if _, err := tx.Exec(ctx, `UPDATE gateway_rate_limits SET
			used_count=$2, reset_at=$3, updated_at=now() WHERE bucket=$1`,
			item.name, used, reset); err != nil {
			return time.Time{}, err
		}
	}
	if !retryAt.IsZero() {
		if err := tx.Commit(ctx); err != nil {
			return time.Time{}, err
		}
		return retryAt, nil
	}
	for _, item := range buckets {
		if _, err := tx.Exec(ctx, `UPDATE gateway_rate_limits SET used_count=used_count+1,
			updated_at=now() WHERE bucket=$1`, item.name); err != nil {
			return time.Time{}, err
		}
	}
	return time.Time{}, tx.Commit(ctx)
}

func (quota Quota) Reconcile(ctx context.Context, headers http.Header) error {
	buckets := quota.buckets()
	overallLimits, overallUsage := parsePair(headers.Get("X-RateLimit-Limit")), parsePair(headers.Get("X-RateLimit-Usage"))
	readLimits, readUsage := parsePair(headers.Get("X-ReadRateLimit-Limit")), parsePair(headers.Get("X-ReadRateLimit-Usage"))
	limits := [4][2]int{readLimits, readLimits, overallLimits, overallLimits}
	usage := [4][2]int{readUsage, readUsage, overallUsage, overallUsage}
	for index, item := range buckets {
		pairIndex := 0
		if strings.HasSuffix(item.name, "_day") {
			pairIndex = 1
		}
		limit, used := limits[index][pairIndex], usage[index][pairIndex]
		if limit <= 0 || used < 0 {
			continue
		}
		if _, err := quota.DB.Exec(ctx, `UPDATE gateway_rate_limits SET
			limit_count=$2, used_count=greatest(used_count,$3), updated_at=now()
			WHERE bucket=$1`, item.name, limit, used); err != nil {
			return err
		}
	}
	return nil
}

func parsePair(value string) [2]int {
	parts := strings.Split(value, ",")
	if len(parts) != 2 {
		return [2]int{-1, -1}
	}
	var result [2]int
	for index, part := range parts {
		number, err := strconv.Atoi(strings.TrimSpace(part))
		if err != nil {
			return [2]int{-1, -1}
		}
		result[index] = number
	}
	return result
}

func (quota Quota) Status(ctx context.Context) (string, error) {
	rows, err := quota.DB.Query(ctx, `SELECT bucket, used_count, limit_count FROM gateway_rate_limits ORDER BY bucket`)
	if err != nil {
		return "", err
	}
	defer rows.Close()
	var status strings.Builder
	for rows.Next() {
		var name string
		var used, limit int
		if err := rows.Scan(&name, &used, &limit); err != nil {
			return "", err
		}
		fmt.Fprintf(&status, "%s=%d/%d ", name, used, limit)
	}
	return strings.TrimSpace(status.String()), rows.Err()
}
