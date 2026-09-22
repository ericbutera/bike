use crate::app_error::AppError;
use crate::entities::provider_rate_limit_buckets;
use crate::metrics;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseConnection, Set, TransactionTrait};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ProviderQuotaBucketSpec {
    pub provider: &'static str,
    pub bucket: &'static str,
    pub limit_count: i32,
    pub window: Duration,
    pub units: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRateLimitPause {
    pub provider: String,
    pub bucket: String,
    pub retry_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderQuotaReservation {
    Reserved,
    RateLimited(ProviderRateLimitPause),
}

pub async fn reserve_provider_quota(
    db: &DatabaseConnection,
    specs: &[ProviderQuotaBucketSpec],
) -> Result<ProviderQuotaReservation, AppError> {
    if specs.is_empty() {
        return Ok(ProviderQuotaReservation::Reserved);
    }

    let provider = validate_quota_specs(specs)?;

    let txn = db.begin().await?;
    let now = Utc::now();

    ensure_quota_bucket_rows(&txn, specs, now).await?;
    let mut rows = load_quota_bucket_rows(&txn, provider, specs).await?;

    if let Some(pause) = quota_reservation_pause(provider, specs, &rows, now)? {
        for row in &rows {
            record_provider_rate_limit_bucket_metrics(row);
        }
        txn.commit().await?;
        return Ok(ProviderQuotaReservation::RateLimited(pause));
    }

    reserve_quota_bucket_rows(&txn, provider, specs, rows.drain(..), now).await?;

    txn.commit().await?;
    Ok(ProviderQuotaReservation::Reserved)
}

fn validate_quota_specs(specs: &[ProviderQuotaBucketSpec]) -> Result<&'static str, AppError> {
    let provider = specs[0].provider;
    if specs.iter().any(|spec| spec.provider != provider) {
        return Err(AppError::internal(
            "Provider quota reservation cannot span multiple providers",
        ));
    }

    if specs
        .iter()
        .any(|spec| spec.limit_count <= 0 || spec.units <= 0)
    {
        return Err(AppError::internal(
            "Provider quota reservation must use positive limits and units",
        ));
    }

    Ok(provider)
}

async fn ensure_quota_bucket_rows<C>(
    db: &C,
    specs: &[ProviderQuotaBucketSpec],
    now: DateTime<Utc>,
) -> Result<(), AppError>
where
    C: ConnectionTrait,
{
    for spec in specs {
        ensure_bucket_row(db, spec, now).await?;
    }

    Ok(())
}

async fn load_quota_bucket_rows<C>(
    db: &C,
    provider: &str,
    specs: &[ProviderQuotaBucketSpec],
) -> Result<Vec<provider_rate_limit_buckets::Model>, AppError>
where
    C: ConnectionTrait,
{
    let bucket_names = specs.iter().map(|spec| spec.bucket).collect::<Vec<_>>();
    Ok(
        provider_rate_limit_buckets::Model::list_for_reservation(db, provider, &bucket_names)
            .await?,
    )
}

fn quota_reservation_pause(
    provider: &str,
    specs: &[ProviderQuotaBucketSpec],
    rows: &[provider_rate_limit_buckets::Model],
    now: DateTime<Utc>,
) -> Result<Option<ProviderRateLimitPause>, AppError> {
    let row_by_bucket = rows
        .iter()
        .map(|row| (row.bucket.as_str(), row))
        .collect::<HashMap<_, _>>();
    let mut retry_at: Option<DateTime<Utc>> = None;
    let mut retry_bucket: Option<String> = None;

    for spec in specs {
        let row = quota_bucket_row(provider, spec, &row_by_bucket)?;
        let used_count = effective_used_count(row, now);

        if used_count + spec.units > spec.limit_count
            && retry_at.is_none_or(|current| row.reset_at > current)
        {
            retry_at = Some(row.reset_at);
            retry_bucket = Some(spec.bucket.to_string());
        }
    }

    Ok(retry_at
        .zip(retry_bucket)
        .map(|(retry_at, bucket)| ProviderRateLimitPause {
            provider: provider.to_string(),
            bucket,
            retry_at,
        }))
}

fn quota_bucket_row<'a>(
    provider: &str,
    spec: &ProviderQuotaBucketSpec,
    row_by_bucket: &HashMap<&str, &'a provider_rate_limit_buckets::Model>,
) -> Result<&'a provider_rate_limit_buckets::Model, AppError> {
    row_by_bucket.get(spec.bucket).copied().ok_or_else(|| {
        AppError::internal(format!(
            "Provider quota bucket {}:{} was not initialized",
            provider, spec.bucket
        ))
    })
}

fn effective_used_count(row: &provider_rate_limit_buckets::Model, now: DateTime<Utc>) -> i32 {
    if row.reset_at <= now {
        0
    } else {
        row.used_count
    }
}

async fn reserve_quota_bucket_rows<C>(
    db: &C,
    provider: &str,
    specs: &[ProviderQuotaBucketSpec],
    rows: impl IntoIterator<Item = provider_rate_limit_buckets::Model>,
    now: DateTime<Utc>,
) -> Result<(), AppError>
where
    C: ConnectionTrait,
{
    for row in rows {
        let spec = quota_spec_for_row(provider, specs, &row)?;
        let updated = reserve_quota_bucket_row(db, row, spec, now).await?;
        record_provider_rate_limit_bucket_metrics(&updated);
    }

    Ok(())
}

fn quota_spec_for_row<'a>(
    provider: &str,
    specs: &'a [ProviderQuotaBucketSpec],
    row: &provider_rate_limit_buckets::Model,
) -> Result<&'a ProviderQuotaBucketSpec, AppError> {
    specs
        .iter()
        .find(|spec| spec.bucket == row.bucket)
        .ok_or_else(|| {
            AppError::internal(format!(
                "Provider quota bucket {}:{} did not match reservation specs",
                provider, row.bucket
            ))
        })
}

async fn reserve_quota_bucket_row<C>(
    db: &C,
    row: provider_rate_limit_buckets::Model,
    spec: &ProviderQuotaBucketSpec,
    now: DateTime<Utc>,
) -> Result<provider_rate_limit_buckets::Model, AppError>
where
    C: ConnectionTrait,
{
    let mut active_model: provider_rate_limit_buckets::ActiveModel = row.into();
    if active_model.reset_at.as_ref() <= &now {
        active_model.used_count = Set(spec.units);
        active_model.reset_at = Set(now + spec.window);
    } else {
        let used_count = *active_model.used_count.as_ref() + spec.units;
        active_model.used_count = Set(used_count);
    }
    active_model.limit_count = Set(spec.limit_count);
    Ok(active_model.update(db).await?)
}

pub async fn reconcile_provider_quota_usage(
    db: &DatabaseConnection,
    provider: &'static str,
    bucket: &'static str,
    limit_count: i32,
    used_count: i32,
    reset_at: DateTime<Utc>,
) -> Result<(), AppError> {
    if limit_count <= 0 || used_count < 0 {
        return Ok(());
    }

    let now = Utc::now();
    let Some(row) = provider_rate_limit_buckets::Model::find_bucket(db, provider, bucket).await?
    else {
        let row = provider_rate_limit_buckets::ActiveModel {
            provider: Set(provider.to_string()),
            bucket: Set(bucket.to_string()),
            limit_count: Set(limit_count),
            used_count: Set(used_count),
            reset_at: Set(reset_at),
            ..Default::default()
        };
        let row = row.insert(db).await?;
        record_provider_rate_limit_bucket_metrics(&row);
        return Ok(());
    };

    let should_reconcile =
        row.reset_at <= now || used_count > row.used_count || limit_count != row.limit_count;
    if !should_reconcile {
        record_provider_rate_limit_bucket_metrics(&row);
        return Ok(());
    }

    let mut active_model: provider_rate_limit_buckets::ActiveModel = row.into();
    active_model.limit_count = Set(limit_count);
    active_model.used_count = Set(used_count);
    active_model.reset_at = Set(reset_at);
    let updated = active_model.update(db).await?;
    record_provider_rate_limit_bucket_metrics(&updated);

    Ok(())
}

fn record_provider_rate_limit_bucket_metrics(row: &provider_rate_limit_buckets::Model) {
    metrics::set_provider_rate_limit_bucket(
        &row.provider,
        &row.bucket,
        row.limit_count,
        row.used_count,
        row.reset_at.timestamp(),
    );
}

async fn ensure_bucket_row<C>(
    db: &C,
    spec: &ProviderQuotaBucketSpec,
    now: DateTime<Utc>,
) -> Result<(), AppError>
where
    C: ConnectionTrait,
{
    let existing =
        provider_rate_limit_buckets::Model::find_bucket(db, spec.provider, spec.bucket).await?;

    if existing.is_some() {
        return Ok(());
    }

    let row = provider_rate_limit_buckets::ActiveModel {
        provider: Set(spec.provider.to_string()),
        bucket: Set(spec.bucket.to_string()),
        limit_count: Set(spec.limit_count),
        used_count: Set(0),
        reset_at: Set(now + spec.window),
        ..Default::default()
    };

    match row.insert(db).await {
        Ok(row) => {
            record_provider_rate_limit_bucket_metrics(&row);
            Ok(())
        }
        Err(error) => {
            tracing::debug!(
                error = ?error,
                provider = spec.provider,
                bucket = spec.bucket,
                "provider quota bucket insert lost an initialization race"
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, EntityTrait, Schema};

    async fn test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");
        let schema = Schema::new(db.get_database_backend());
        db.execute(&schema.create_table_from_entity(provider_rate_limit_buckets::Entity))
            .await
            .expect("create provider rate limit buckets table");
        db
    }

    #[tokio::test]
    async fn reserve_provider_quota_blocks_when_any_bucket_is_exhausted() {
        let db = test_db().await;
        let short = ProviderQuotaBucketSpec {
            provider: "example",
            bucket: "overall_15_minute",
            limit_count: 1,
            window: Duration::minutes(15),
            units: 1,
        };
        let daily = ProviderQuotaBucketSpec {
            provider: "example",
            bucket: "overall_daily",
            limit_count: 10,
            window: Duration::hours(24),
            units: 1,
        };

        assert_eq!(
            reserve_provider_quota(&db, &[short.clone(), daily.clone()])
                .await
                .expect("reserve quota"),
            ProviderQuotaReservation::Reserved
        );

        let blocked = reserve_provider_quota(&db, &[short, daily])
            .await
            .expect("reserve quota");

        assert!(matches!(
            blocked,
            ProviderQuotaReservation::RateLimited(ProviderRateLimitPause { bucket, .. })
                if bucket == "overall_15_minute"
        ));
    }

    #[tokio::test]
    async fn reconcile_provider_quota_usage_moves_local_count_forward() {
        let db = test_db().await;
        let reset_at = Utc::now() + Duration::minutes(15);

        reconcile_provider_quota_usage(&db, "example", "read_15_minute", 200, 25, reset_at)
            .await
            .expect("reconcile quota");

        let row = provider_rate_limit_buckets::Entity::find()
            .one(&db)
            .await
            .expect("load row")
            .expect("row exists");
        assert_eq!(row.limit_count, 200);
        assert_eq!(row.used_count, 25);
        assert_eq!(row.reset_at, reset_at);
    }
}
