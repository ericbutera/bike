//! Ownership, time, sport, zoom and spatial predicates run before materializing coordinates.
use super::{geometry::zoom_band, raster::Tile, types::HeatmapQuery, PROJECTION_VERSION};
use sea_orm::{ConnectionTrait, DbBackend, DbErr, FromQueryResult, Statement, Value};

#[derive(Debug, FromQueryResult)]
pub struct Progress {
    pub revision: i64,
    pub ready: i64,
    pub pending: i64,
    pub failed: i64,
    pub skipped: i64,
    pub min_x: Option<f64>,
    pub min_y: Option<f64>,
    pub max_x: Option<f64>,
    pub max_y: Option<f64>,
}

#[derive(FromQueryResult)]
pub struct TileChunk {
    pub activity_id: i32,
    pub chunk_index: i32,
    pub points: Vec<u8>,
}

#[derive(Debug, FromQueryResult)]
pub struct ActivityCenter {
    pub x: f64,
    pub y: f64,
}

pub struct HeatmapData;

impl HeatmapData {
    pub async fn revision<C: ConnectionTrait>(db: &C, user_id: i32) -> Result<i64, DbErr> {
        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT revision FROM heatmap_user_states WHERE user_id=$1",
                [user_id.into()],
            ))
            .await?;
        row.map(|row| row.try_get("", "revision"))
            .transpose()
            .map(|n| n.unwrap_or(0))
    }

    pub async fn progress<C: ConnectionTrait>(
        db: &C,
        user_id: i32,
        filters: &HeatmapQuery,
    ) -> Result<Progress, DbErr> {
        let scope = ActivityScope::new(user_id, filters);
        Progress::find_by_statement(Statement::from_sql_and_values(DbBackend::Postgres, format!(r#"
            SELECT COALESCE((SELECT revision FROM heatmap_user_states WHERE user_id=$1),0) AS revision,
                count(*) FILTER(WHERE p.status='ready') AS ready,
                count(*) FILTER(WHERE p.status='pending') AS pending,
                count(*) FILTER(WHERE p.status='failed') AS failed,
                count(*) FILTER(WHERE p.status='skipped') AS skipped,
                min(p.min_x) FILTER(WHERE p.status='ready') AS min_x,
                min(p.min_y) FILTER(WHERE p.status='ready') AS min_y,
                max(p.max_x) FILTER(WHERE p.status='ready') AS max_x,
                max(p.max_y) FILTER(WHERE p.status='ready') AS max_y
            FROM heatmap_projections p JOIN activities a ON a.id=p.activity_id WHERE {}
        "#,scope.predicate),scope.values)).one(db).await?.ok_or_else(|| DbErr::Custom("Missing heatmap progress".into()))
    }

    pub async fn activity_centers<C: ConnectionTrait>(
        db: &C,
        user_id: i32,
        filters: &HeatmapQuery,
    ) -> Result<Vec<ActivityCenter>, DbErr> {
        let scope = ActivityScope::new(user_id, filters);
        ActivityCenter::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                r#"
                SELECT
                    CASE WHEN p.max_x - p.min_x > 0.5
                        THEN (p.min_x + p.max_x + 1.0) / 2.0
                            - CASE WHEN p.min_x + p.max_x >= 1.0 THEN 1.0 ELSE 0.0 END
                        ELSE (p.min_x + p.max_x) / 2.0
                    END AS x,
                    (p.min_y + p.max_y) / 2.0 AS y
                FROM heatmap_projections p JOIN activities a ON a.id=p.activity_id
                WHERE {} AND p.status='ready' AND p.projection_version={}
                ORDER BY p.activity_id
            "#,
                scope.predicate, PROJECTION_VERSION
            ),
            scope.values,
        ))
        .all(db)
        .await
    }

    pub async fn tile_page<C: ConnectionTrait>(
        db: &C,
        user_id: i32,
        filters: &HeatmapQuery,
        tile: Tile,
        cursor: (i32, i32),
    ) -> Result<Vec<TileChunk>, DbErr> {
        let mut scope = ActivityScope::new(user_id, filters);
        let band = scope.bind(zoom_band(tile.z).into());
        let activity = scope.bind(cursor.0.into());
        let chunk = scope.bind(cursor.1.into());
        let mut boxes = Vec::new();
        for bounds in tile.query_bounds() {
            let b: Vec<_> = bounds.into_iter().map(|v| scope.bind(v.into())).collect();
            boxes.push(format!("box(point(c.min_x,c.min_y),point(c.max_x,c.max_y)) && box(point({},{}),point({},{}))",b[0],b[1],b[2],b[3]));
        }
        TileChunk::find_by_statement(Statement::from_sql_and_values(DbBackend::Postgres,format!(
            "SELECT c.activity_id,c.chunk_index,c.points FROM heatmap_chunks c JOIN heatmap_projections p ON p.activity_id=c.activity_id JOIN activities a ON a.id=p.activity_id WHERE {} AND p.status='ready' AND p.projection_version={} AND c.band={} AND (c.activity_id,c.chunk_index) > ({},{}) AND ({}) ORDER BY c.activity_id,c.chunk_index LIMIT 64",
            scope.predicate,PROJECTION_VERSION,band,activity,chunk,boxes.join(" OR ")),scope.values)).all(db).await
    }
}

struct ActivityScope {
    predicate: String,
    values: Vec<Value>,
}

impl ActivityScope {
    fn new(user_id: i32, filters: &HeatmapQuery) -> Self {
        let mut scope = Self {
            predicate: "a.user_id=$1 AND p.user_id=$1".into(),
            values: vec![user_id.into()],
        };
        if let Some(sport) = filters.sport {
            let parameters: Vec<_> = sport
                .stored_values()
                .iter()
                .map(|value| scope.bind((*value).into()))
                .collect();
            scope
                .predicate
                .push_str(&format!(" AND a.sport IN ({})", parameters.join(",")));
        }
        if let Some(from) = filters.from {
            let from = scope.bind(from.into());
            scope
                .predicate
                .push_str(&format!(" AND a.started_at >= {from}"));
        }
        if let Some(to) = filters.to {
            let to = scope.bind(to.into());
            scope
                .predicate
                .push_str(&format!(" AND a.started_at < {to}"));
        }
        scope
    }
    fn bind(&mut self, value: Value) -> String {
        self.values.push(value);
        format!("${}", self.values.len())
    }
}
