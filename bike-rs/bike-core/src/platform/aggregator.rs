use chrono::{Duration, Utc};
use sea_orm::{prelude::*, sea_query::Alias, ColumnTrait, ExprTrait, FromQueryResult, QuerySelect};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A named, displayable metric with a machine-readable key and human-readable label.
/// Used in sectioned admin metrics responses so the UI can map keys to icons/links.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NamedStat {
    /// Machine-readable identifier — used by the UI to look up icons and links.
    pub key: String,
    /// Human-readable display label.
    pub label: String,
    /// Short time-range description, e.g. "last 30 days".
    pub desc: String,
    pub value: i64,
    pub error: Option<String>,
}

impl NamedStat {
    pub fn new(
        key: &'static str,
        label: &'static str,
        desc: &'static str,
        result: Result<u64, DbErr>,
    ) -> Self {
        match result {
            Ok(v) => NamedStat {
                key: key.into(),
                label: label.into(),
                desc: desc.into(),
                value: v as i64,
                error: None,
            },
            Err(e) => NamedStat {
                key: key.into(),
                label: label.into(),
                desc: desc.into(),
                value: 0,
                error: Some(e.to_string()),
            },
        }
    }
}

/// A single metric value with optional error context.
#[derive(Serialize, ToSchema, Debug)]
pub struct StatResult {
    /// Format: int64
    pub value: i64,
    pub error: Option<String>,
}

impl StatResult {
    pub fn from_result(res: Result<u64, DbErr>) -> Self {
        match res {
            Ok(value) => StatResult {
                value: value as i64,
                error: None,
            },
            Err(e) => StatResult {
                value: 0,
                error: Some(e.to_string()),
            },
        }
    }

    pub fn empty() -> Self {
        StatResult {
            value: 0,
            error: None,
        }
    }
}

/// Generic database aggregation helpers for admin metrics.
pub struct Aggregator;

impl Aggregator {
    /// COUNT(*) over an entire table.
    pub async fn total<E>(db: &DatabaseConnection, count_col: E::Column) -> Result<u64, DbErr>
    where
        E: EntityTrait,
        E::Column: ColumnTrait,
    {
        #[derive(FromQueryResult)]
        struct CountResult {
            count: Option<i64>,
        }

        let res: Option<CountResult> = E::find()
            .select_only()
            .column_as(count_col.count(), "count")
            .into_model::<CountResult>()
            .one(db)
            .await?;

        Ok(res.and_then(|r| r.count).unwrap_or(0) as u64)
    }

    /// COUNT(*) for rows newer than `days` days, filtered by `date_col`.
    pub async fn recent_count<E>(
        db: &DatabaseConnection,
        date_col: E::Column,
        days: i64,
    ) -> Result<u64, DbErr>
    where
        E: EntityTrait,
        E::Column: ColumnTrait,
    {
        let threshold = Utc::now() - Duration::days(days);

        #[derive(FromQueryResult)]
        struct CountResult {
            count: Option<i64>,
        }

        let res: Option<CountResult> = E::find()
            .select_only()
            .column_as(date_col.count(), "count")
            .filter(date_col.gte(threshold))
            .into_model::<CountResult>()
            .one(db)
            .await?;

        Ok(res.and_then(|r| r.count).unwrap_or(0) as u64)
    }

    /// COUNT(*) for rows newer than `days` days where `not_null_col` IS NOT NULL.
    pub async fn recent_count_not_null<E>(
        db: &DatabaseConnection,
        date_col: E::Column,
        not_null_col: E::Column,
        days: i64,
    ) -> Result<u64, DbErr>
    where
        E: EntityTrait,
        E::Column: ColumnTrait,
    {
        let threshold = Utc::now() - Duration::days(days);

        #[derive(FromQueryResult)]
        struct CountResult {
            count: Option<i64>,
        }

        let res: Option<CountResult> = E::find()
            .select_only()
            .column_as(date_col.count(), "count")
            .filter(date_col.gte(threshold))
            .filter(not_null_col.is_not_null())
            .into_model::<CountResult>()
            .one(db)
            .await?;

        Ok(res.and_then(|r| r.count).unwrap_or(0) as u64)
    }

    /// SUM of `sum_col` for rows newer than `days` days.
    ///
    /// Casts the SUM to FLOAT8 so the result decodes correctly regardless of
    /// whether the underlying column is INT4 (SUM → INT8) or INT8 (SUM → NUMERIC).
    pub async fn recent_sum<E>(
        db: &DatabaseConnection,
        date_col: E::Column,
        sum_col: E::Column,
        days: i64,
    ) -> Result<u64, DbErr>
    where
        E: EntityTrait,
        E::Column: ColumnTrait,
    {
        let threshold = Utc::now() - Duration::days(days);

        #[derive(FromQueryResult)]
        struct SumResult {
            total: Option<f64>,
        }

        let res: Option<SumResult> = E::find()
            .select_only()
            .column_as(sum_col.sum().cast_as(Alias::new("float8")), "total")
            .filter(date_col.gte(threshold))
            .into_model::<SumResult>()
            .one(db)
            .await?;

        Ok(res.and_then(|r| r.total).unwrap_or(0.0) as u64)
    }
}
