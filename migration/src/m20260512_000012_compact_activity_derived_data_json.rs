use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};

const STORAGE_FORMAT_VERSION: u8 = 1;
const STORAGE_COORDINATE_SCALE: f64 = 10_000_000.0;
const STORAGE_DISTANCE_SCALE: f64 = 10.0;
const STORAGE_ELEVATION_SCALE: f64 = 10.0;
const STORAGE_SPEED_SCALE: f64 = 100.0;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum Activities {
    Table,
    Id,
    DerivedDataJson,
    DerivedDataJsonCompact,
    DerivedDataJsonLegacy,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct LegacyActivityDerivedData {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    laps: Vec<LegacyActivityLap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    chart_points: Vec<LegacyActivityChartPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    route_points: Vec<LegacyActivityRoutePoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct LegacyActivityLap {
    lap_index: i32,
    title: String,
    start_offset_seconds: Option<i32>,
    duration_seconds: Option<i32>,
    distance_meters: Option<f64>,
    elevation_gain_meters: Option<f64>,
    elevation_loss_meters: Option<f64>,
    average_speed_mps: Option<f64>,
    max_speed_mps: Option<f64>,
    average_heart_rate_bpm: Option<i32>,
    max_heart_rate_bpm: Option<i32>,
    average_cadence_rpm: Option<i32>,
    max_cadence_rpm: Option<i32>,
    calories: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct LegacyActivityChartPoint {
    elapsed_seconds: i32,
    distance_meters: Option<f64>,
    elevation_meters: Option<f64>,
    speed_mps: Option<f64>,
    heart_rate_bpm: Option<i32>,
    cadence_rpm: Option<i32>,
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
struct CompactActivityDerivedData {
    v: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "l")]
    laps: Vec<CompactActivityLap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "c")]
    chart_points: Vec<CompactActivityChartPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "r")]
    route_points: Vec<CompactActivityRoutePoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CompactActivityLap(
    i32,
    String,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CompactActivityChartPoint(
    i32,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

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

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .add_column(ColumnDef::new(Activities::DerivedDataJsonCompact).json().null())
                    .to_owned(),
            )
            .await?;

        rewrite_legacy_rows_to_compact(manager).await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .drop_column(Activities::DerivedDataJson)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .rename_column(
                        Activities::DerivedDataJsonCompact,
                        Activities::DerivedDataJson,
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .add_column(ColumnDef::new(Activities::DerivedDataJsonLegacy).text().null())
                    .to_owned(),
            )
            .await?;

        rewrite_compact_rows_to_legacy(manager).await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .drop_column(Activities::DerivedDataJson)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Activities::Table)
                    .rename_column(
                        Activities::DerivedDataJsonLegacy,
                        Activities::DerivedDataJson,
                    )
                    .to_owned(),
            )
            .await
    }
}

async fn rewrite_legacy_rows_to_compact(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let select = Query::select()
        .column(Activities::Id)
        .column(Activities::DerivedDataJson)
        .from(Activities::Table)
        .and_where(Expr::col(Activities::DerivedDataJson).is_not_null())
        .to_owned();
    let rows = connection.query_all(&select).await?;

    for row in rows {
        let id: i32 = row.try_get("", "id")?;
        let raw: String = row.try_get("", "derived_data_json")?;
        if raw.trim().is_empty() {
            continue;
        }

        let compact_json = serde_json::to_value(CompactActivityDerivedData::from(
            serde_json::from_str::<LegacyActivityDerivedData>(&raw)
                .map_err(|error| json_error("parse legacy activity derived data", error))?,
        ))
        .map_err(|error| json_error("serialize compact activity derived data", error))?;

        let update = Query::update()
            .table(Activities::Table)
            .value(Activities::DerivedDataJsonCompact, compact_json)
            .and_where(Expr::col(Activities::Id).eq(id))
            .to_owned();

        connection.execute(&update).await?;
    }

    Ok(())
}

async fn rewrite_compact_rows_to_legacy(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let select = Query::select()
        .column(Activities::Id)
        .column(Activities::DerivedDataJson)
        .from(Activities::Table)
        .and_where(Expr::col(Activities::DerivedDataJson).is_not_null())
        .to_owned();
    let rows = connection.query_all(&select).await?;

    for row in rows {
        let id: i32 = row.try_get("", "id")?;
        let raw: serde_json::Value = row.try_get("", "derived_data_json")?;
        let compact = serde_json::from_value::<CompactActivityDerivedData>(raw)
            .map_err(|error| json_error("parse compact activity derived data", error))?;
        let legacy_json = serde_json::to_string(&LegacyActivityDerivedData::from(compact))
            .map_err(|error| json_error("serialize legacy activity derived data", error))?;

        let update = Query::update()
            .table(Activities::Table)
            .value(Activities::DerivedDataJsonLegacy, legacy_json)
            .and_where(Expr::col(Activities::Id).eq(id))
            .to_owned();

        connection.execute(&update).await?;
    }

    Ok(())
}

fn json_error(context: &str, error: impl std::fmt::Display) -> DbErr {
    DbErr::Custom(format!("{context}: {error}"))
}

impl From<LegacyActivityDerivedData> for CompactActivityDerivedData {
    fn from(value: LegacyActivityDerivedData) -> Self {
        Self {
            v: STORAGE_FORMAT_VERSION,
            laps: value.laps.into_iter().map(CompactActivityLap::from).collect(),
            chart_points: value
                .chart_points
                .into_iter()
                .map(CompactActivityChartPoint::from)
                .collect(),
            route_points: value
                .route_points
                .into_iter()
                .map(CompactActivityRoutePoint::from)
                .collect(),
        }
    }
}

impl From<CompactActivityDerivedData> for LegacyActivityDerivedData {
    fn from(value: CompactActivityDerivedData) -> Self {
        if value.v != STORAGE_FORMAT_VERSION {
            return Self::default();
        }

        Self {
            laps: value.laps.into_iter().map(LegacyActivityLap::from).collect(),
            chart_points: value
                .chart_points
                .into_iter()
                .map(LegacyActivityChartPoint::from)
                .collect(),
            route_points: value
                .route_points
                .into_iter()
                .map(LegacyActivityRoutePoint::from)
                .collect(),
        }
    }
}

impl From<LegacyActivityLap> for CompactActivityLap {
    fn from(value: LegacyActivityLap) -> Self {
        Self(
            value.lap_index,
            value.title,
            value.start_offset_seconds,
            value.duration_seconds,
            encode_scaled_metric(value.distance_meters, STORAGE_DISTANCE_SCALE),
            encode_scaled_metric(value.elevation_gain_meters, STORAGE_ELEVATION_SCALE),
            encode_scaled_metric(value.elevation_loss_meters, STORAGE_ELEVATION_SCALE),
            encode_scaled_metric(value.average_speed_mps, STORAGE_SPEED_SCALE),
            encode_scaled_metric(value.max_speed_mps, STORAGE_SPEED_SCALE),
            value.average_heart_rate_bpm,
            value.max_heart_rate_bpm,
            value.average_cadence_rpm,
            value.max_cadence_rpm,
            value.calories,
        )
    }
}

impl From<CompactActivityLap> for LegacyActivityLap {
    fn from(value: CompactActivityLap) -> Self {
        Self {
            lap_index: value.0,
            title: value.1,
            start_offset_seconds: value.2,
            duration_seconds: value.3,
            distance_meters: decode_scaled_metric(value.4, STORAGE_DISTANCE_SCALE),
            elevation_gain_meters: decode_scaled_metric(value.5, STORAGE_ELEVATION_SCALE),
            elevation_loss_meters: decode_scaled_metric(value.6, STORAGE_ELEVATION_SCALE),
            average_speed_mps: decode_scaled_metric(value.7, STORAGE_SPEED_SCALE),
            max_speed_mps: decode_scaled_metric(value.8, STORAGE_SPEED_SCALE),
            average_heart_rate_bpm: value.9,
            max_heart_rate_bpm: value.10,
            average_cadence_rpm: value.11,
            max_cadence_rpm: value.12,
            calories: value.13,
        }
    }
}

impl From<LegacyActivityChartPoint> for CompactActivityChartPoint {
    fn from(value: LegacyActivityChartPoint) -> Self {
        Self(
            value.elapsed_seconds,
            encode_scaled_metric(value.distance_meters, STORAGE_DISTANCE_SCALE),
            encode_scaled_metric(value.elevation_meters, STORAGE_ELEVATION_SCALE),
            encode_scaled_metric(value.speed_mps, STORAGE_SPEED_SCALE),
            value.heart_rate_bpm,
            value.cadence_rpm,
        )
    }
}

impl From<CompactActivityChartPoint> for LegacyActivityChartPoint {
    fn from(value: CompactActivityChartPoint) -> Self {
        Self {
            elapsed_seconds: value.0,
            distance_meters: decode_scaled_metric(value.1, STORAGE_DISTANCE_SCALE),
            elevation_meters: decode_scaled_metric(value.2, STORAGE_ELEVATION_SCALE),
            speed_mps: decode_scaled_metric(value.3, STORAGE_SPEED_SCALE),
            heart_rate_bpm: value.4,
            cadence_rpm: value.5,
        }
    }
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