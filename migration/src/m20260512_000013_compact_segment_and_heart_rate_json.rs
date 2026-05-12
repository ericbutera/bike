use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};

const STORAGE_COORDINATE_SCALE: f64 = 10_000_000.0;
const STORAGE_DISTANCE_SCALE: f64 = 10.0;
const STORAGE_ELEVATION_SCALE: f64 = 10.0;
const STORAGE_SPEED_SCALE: f64 = 100.0;
const HEART_RATE_ZONE_SHARE_PERCENT_SCALE: f64 = 1000.0;
const HEART_RATE_ZONE_LABELS: [&str; 5] = ["Z1", "Z2", "Z3", "Z4", "Z5"];

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum Segments {
    Table,
    Id,
    RouteDataJson,
    RouteDataJsonCompact,
    RouteDataJsonLegacy,
}

#[derive(Iden)]
enum Activities {
    Table,
    Id,
    HeartRateZonesJson,
    HeartRateZonesJsonCompact,
    HeartRateZonesJsonLegacy,
}

#[derive(Iden)]
enum UserPreferences {
    Table,
    Id,
    HeartRateZoneBoundsJson,
    HeartRateZoneBoundsJsonCompact,
    HeartRateZoneBoundsJsonLegacy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct LegacyActivityRoutePoint {
    elapsed_seconds: i32,
    latitude: f64,
    longitude: f64,
    distance_meters: Option<f64>,
    elevation_meters: Option<f64>,
    speed_mps: Option<f64>,
    heart_rate_bpm: Option<i32>,
    cadence_rpm: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CompactActivityRoutePoint(
    i32,
    i32,
    i32,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct LegacyActivityHeartRateZoneSummary {
    zone: i32,
    label: String,
    min_bpm: Option<i32>,
    max_bpm: Option<i32>,
    duration_seconds: i32,
    share_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CompactActivityHeartRateZoneSummary(
    i32,
    Option<i32>,
    Option<i32>,
    i32,
    i32,
);

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Segments::Table)
                    .add_column(ColumnDef::new(Segments::RouteDataJsonCompact).json().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .add_column(
                        ColumnDef::new(Activities::HeartRateZonesJsonCompact)
                            .json()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserPreferences::Table)
                    .add_column(
                        ColumnDef::new(UserPreferences::HeartRateZoneBoundsJsonCompact)
                            .json()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        rewrite_segment_route_rows_to_compact(manager).await?;
        rewrite_activity_heart_rate_zone_rows_to_compact(manager).await?;
        rewrite_heart_rate_bound_rows_to_compact(manager).await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Segments::Table)
                    .drop_column(Segments::RouteDataJson)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .drop_column(Activities::HeartRateZonesJson)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserPreferences::Table)
                    .drop_column(UserPreferences::HeartRateZoneBoundsJson)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Segments::Table)
                    .rename_column(Segments::RouteDataJsonCompact, Segments::RouteDataJson)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .rename_column(
                        Activities::HeartRateZonesJsonCompact,
                        Activities::HeartRateZonesJson,
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserPreferences::Table)
                    .rename_column(
                        UserPreferences::HeartRateZoneBoundsJsonCompact,
                        UserPreferences::HeartRateZoneBoundsJson,
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Segments::Table)
                    .add_column(ColumnDef::new(Segments::RouteDataJsonLegacy).text().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .add_column(
                        ColumnDef::new(Activities::HeartRateZonesJsonLegacy)
                            .text()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserPreferences::Table)
                    .add_column(
                        ColumnDef::new(UserPreferences::HeartRateZoneBoundsJsonLegacy)
                            .text()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        rewrite_segment_route_rows_to_legacy(manager).await?;
        rewrite_activity_heart_rate_zone_rows_to_legacy(manager).await?;
        rewrite_heart_rate_bound_rows_to_legacy(manager).await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Segments::Table)
                    .drop_column(Segments::RouteDataJson)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .drop_column(Activities::HeartRateZonesJson)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserPreferences::Table)
                    .drop_column(UserPreferences::HeartRateZoneBoundsJson)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Segments::Table)
                    .rename_column(Segments::RouteDataJsonLegacy, Segments::RouteDataJson)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .rename_column(
                        Activities::HeartRateZonesJsonLegacy,
                        Activities::HeartRateZonesJson,
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(UserPreferences::Table)
                    .rename_column(
                        UserPreferences::HeartRateZoneBoundsJsonLegacy,
                        UserPreferences::HeartRateZoneBoundsJson,
                    )
                    .to_owned(),
            )
            .await
    }
}

async fn rewrite_segment_route_rows_to_compact(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let select = Query::select()
        .column(Segments::Id)
        .column(Segments::RouteDataJson)
        .from(Segments::Table)
        .and_where(Expr::col(Segments::RouteDataJson).is_not_null())
        .to_owned();
    let rows = connection.query_all(&select).await?;

    for row in rows {
        let id: i32 = row.try_get("", "id")?;
        let raw: String = row.try_get("", "route_data_json")?;
        if raw.trim().is_empty() {
            continue;
        }

        let compact_json = serde_json::to_value(
            serde_json::from_str::<Vec<LegacyActivityRoutePoint>>(&raw)
                .map_err(|error| json_error("parse legacy segment route data", error))?
                .into_iter()
                .map(CompactActivityRoutePoint::from)
                .collect::<Vec<_>>(),
        )
        .map_err(|error| json_error("serialize compact segment route data", error))?;

        let update = Query::update()
            .table(Segments::Table)
            .value(Segments::RouteDataJsonCompact, compact_json)
            .and_where(Expr::col(Segments::Id).eq(id))
            .to_owned();

        connection.execute(&update).await?;
    }

    Ok(())
}

async fn rewrite_segment_route_rows_to_legacy(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let select = Query::select()
        .column(Segments::Id)
        .column(Segments::RouteDataJson)
        .from(Segments::Table)
        .and_where(Expr::col(Segments::RouteDataJson).is_not_null())
        .to_owned();
    let rows = connection.query_all(&select).await?;

    for row in rows {
        let id: i32 = row.try_get("", "id")?;
        let raw: serde_json::Value = row.try_get("", "route_data_json")?;
        let legacy_json = serde_json::to_string(
            &serde_json::from_value::<Vec<CompactActivityRoutePoint>>(raw)
                .map_err(|error| json_error("parse compact segment route data", error))?
                .into_iter()
                .map(LegacyActivityRoutePoint::from)
                .collect::<Vec<_>>(),
        )
        .map_err(|error| json_error("serialize legacy segment route data", error))?;

        let update = Query::update()
            .table(Segments::Table)
            .value(Segments::RouteDataJsonLegacy, legacy_json)
            .and_where(Expr::col(Segments::Id).eq(id))
            .to_owned();

        connection.execute(&update).await?;
    }

    Ok(())
}

async fn rewrite_activity_heart_rate_zone_rows_to_compact(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let select = Query::select()
        .column(Activities::Id)
        .column(Activities::HeartRateZonesJson)
        .from(Activities::Table)
        .and_where(Expr::col(Activities::HeartRateZonesJson).is_not_null())
        .to_owned();
    let rows = connection.query_all(&select).await?;

    for row in rows {
        let id: i32 = row.try_get("", "id")?;
        let raw: String = row.try_get("", "heart_rate_zones_json")?;
        if raw.trim().is_empty() {
            continue;
        }

        let compact_json = serde_json::to_value(
            serde_json::from_str::<Vec<LegacyActivityHeartRateZoneSummary>>(&raw)
                .map_err(|error| json_error("parse legacy activity heart rate zones", error))?
                .into_iter()
                .map(CompactActivityHeartRateZoneSummary::from)
                .collect::<Vec<_>>(),
        )
        .map_err(|error| json_error("serialize compact activity heart rate zones", error))?;

        let update = Query::update()
            .table(Activities::Table)
            .value(Activities::HeartRateZonesJsonCompact, compact_json)
            .and_where(Expr::col(Activities::Id).eq(id))
            .to_owned();

        connection.execute(&update).await?;
    }

    Ok(())
}

async fn rewrite_activity_heart_rate_zone_rows_to_legacy(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let select = Query::select()
        .column(Activities::Id)
        .column(Activities::HeartRateZonesJson)
        .from(Activities::Table)
        .and_where(Expr::col(Activities::HeartRateZonesJson).is_not_null())
        .to_owned();
    let rows = connection.query_all(&select).await?;

    for row in rows {
        let id: i32 = row.try_get("", "id")?;
        let raw: serde_json::Value = row.try_get("", "heart_rate_zones_json")?;
        let legacy_json = serde_json::to_string(
            &serde_json::from_value::<Vec<CompactActivityHeartRateZoneSummary>>(raw)
                .map_err(|error| json_error("parse compact activity heart rate zones", error))?
                .into_iter()
                .map(LegacyActivityHeartRateZoneSummary::from)
                .collect::<Vec<_>>(),
        )
        .map_err(|error| json_error("serialize legacy activity heart rate zones", error))?;

        let update = Query::update()
            .table(Activities::Table)
            .value(Activities::HeartRateZonesJsonLegacy, legacy_json)
            .and_where(Expr::col(Activities::Id).eq(id))
            .to_owned();

        connection.execute(&update).await?;
    }

    Ok(())
}

async fn rewrite_heart_rate_bound_rows_to_compact(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let select = Query::select()
        .column(UserPreferences::Id)
        .column(UserPreferences::HeartRateZoneBoundsJson)
        .from(UserPreferences::Table)
        .and_where(Expr::col(UserPreferences::HeartRateZoneBoundsJson).is_not_null())
        .to_owned();
    let rows = connection.query_all(&select).await?;

    for row in rows {
        let id: i32 = row.try_get("", "id")?;
        let raw: String = row.try_get("", "heart_rate_zone_bounds_json")?;
        if raw.trim().is_empty() {
            continue;
        }

        let compact_json = serde_json::to_value(
            serde_json::from_str::<Vec<i32>>(&raw)
                .map_err(|error| json_error("parse legacy heart rate bounds", error))?,
        )
        .map_err(|error| json_error("serialize compact heart rate bounds", error))?;

        let update = Query::update()
            .table(UserPreferences::Table)
            .value(UserPreferences::HeartRateZoneBoundsJsonCompact, compact_json)
            .and_where(Expr::col(UserPreferences::Id).eq(id))
            .to_owned();

        connection.execute(&update).await?;
    }

    Ok(())
}

async fn rewrite_heart_rate_bound_rows_to_legacy(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let select = Query::select()
        .column(UserPreferences::Id)
        .column(UserPreferences::HeartRateZoneBoundsJson)
        .from(UserPreferences::Table)
        .and_where(Expr::col(UserPreferences::HeartRateZoneBoundsJson).is_not_null())
        .to_owned();
    let rows = connection.query_all(&select).await?;

    for row in rows {
        let id: i32 = row.try_get("", "id")?;
        let raw: serde_json::Value = row.try_get("", "heart_rate_zone_bounds_json")?;
        let legacy_json = serde_json::to_string(
            &serde_json::from_value::<Vec<i32>>(raw)
                .map_err(|error| json_error("parse compact heart rate bounds", error))?,
        )
        .map_err(|error| json_error("serialize legacy heart rate bounds", error))?;

        let update = Query::update()
            .table(UserPreferences::Table)
            .value(UserPreferences::HeartRateZoneBoundsJsonLegacy, legacy_json)
            .and_where(Expr::col(UserPreferences::Id).eq(id))
            .to_owned();

        connection.execute(&update).await?;
    }

    Ok(())
}

fn json_error(context: &str, error: impl std::fmt::Display) -> DbErr {
    DbErr::Custom(format!("{context}: {error}"))
}

impl From<LegacyActivityRoutePoint> for CompactActivityRoutePoint {
    fn from(value: LegacyActivityRoutePoint) -> Self {
        Self(
            value.elapsed_seconds,
            encode_coordinate(value.latitude),
            encode_coordinate(value.longitude),
            encode_scaled_metric(value.distance_meters, STORAGE_DISTANCE_SCALE),
            encode_scaled_metric(value.elevation_meters, STORAGE_ELEVATION_SCALE),
            encode_scaled_metric(value.speed_mps, STORAGE_SPEED_SCALE),
            value.heart_rate_bpm,
            value.cadence_rpm,
        )
    }
}

impl From<CompactActivityRoutePoint> for LegacyActivityRoutePoint {
    fn from(value: CompactActivityRoutePoint) -> Self {
        Self {
            elapsed_seconds: value.0,
            latitude: decode_coordinate(value.1),
            longitude: decode_coordinate(value.2),
            distance_meters: decode_scaled_metric(value.3, STORAGE_DISTANCE_SCALE),
            elevation_meters: decode_scaled_metric(value.4, STORAGE_ELEVATION_SCALE),
            speed_mps: decode_scaled_metric(value.5, STORAGE_SPEED_SCALE),
            heart_rate_bpm: value.6,
            cadence_rpm: value.7,
        }
    }
}

impl From<LegacyActivityHeartRateZoneSummary> for CompactActivityHeartRateZoneSummary {
    fn from(value: LegacyActivityHeartRateZoneSummary) -> Self {
        Self(
            value.zone,
            value.min_bpm,
            value.max_bpm,
            value.duration_seconds,
            encode_share_percent(value.share_percent),
        )
    }
}

impl From<CompactActivityHeartRateZoneSummary> for LegacyActivityHeartRateZoneSummary {
    fn from(value: CompactActivityHeartRateZoneSummary) -> Self {
        Self {
            zone: value.0,
            label: heart_rate_zone_label(value.0),
            min_bpm: value.1,
            max_bpm: value.2,
            duration_seconds: value.3,
            share_percent: decode_share_percent(value.4),
        }
    }
}

fn heart_rate_zone_label(zone: i32) -> String {
    HEART_RATE_ZONE_LABELS
        .get(zone.saturating_sub(1) as usize)
        .map(|value| (*value).to_string())
        .unwrap_or_else(|| format!("Z{zone}"))
}

fn encode_share_percent(value: f64) -> i32 {
    (value * HEART_RATE_ZONE_SHARE_PERCENT_SCALE).round() as i32
}

fn decode_share_percent(value: i32) -> f64 {
    f64::from(value) / HEART_RATE_ZONE_SHARE_PERCENT_SCALE
}

fn encode_coordinate(value: f64) -> i32 {
    (value * STORAGE_COORDINATE_SCALE).round() as i32
}

fn decode_coordinate(value: i32) -> f64 {
    f64::from(value) / STORAGE_COORDINATE_SCALE
}

fn encode_scaled_metric(value: Option<f64>, scale: f64) -> Option<i32> {
    value.and_then(|metric| {
        if !metric.is_finite() {
            return None;
        }

        let scaled = (metric * scale).round();
        if scaled < f64::from(i32::MIN) || scaled > f64::from(i32::MAX) {
            None
        } else {
            Some(scaled as i32)
        }
    })
}

fn decode_scaled_metric(value: Option<i32>, scale: f64) -> Option<f64> {
    value.map(|metric| f64::from(metric) / scale)
}