use crate::{geometry, workloads};
use anyhow::{ensure, Result};
use postgres::{
    types::{FromSql, ToSql, Type},
    Client, Row,
};
use serde_json::{json, Value};
use std::time::Instant;

pub struct Raw<'a>(pub &'a [u8]);
impl<'a> FromSql<'a> for Raw<'a> {
    fn from_sql(
        _: &Type,
        raw: &'a [u8],
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Self(raw))
    }
    fn accepts(_: &Type) -> bool {
        true
    }
}

#[derive(Default)]
pub struct Access {
    pub sql_ms: f64,
    pub calls: usize,
    pub payload_bytes: usize,
    pub rows: usize,
    pub plans: Vec<Value>,
    pub collect_plans: bool,
}

impl Access {
    pub fn query(
        &mut self,
        db: &mut Client,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<Row>> {
        let started = Instant::now();
        let rows = db.query(sql, params)?;
        self.sql_ms += started.elapsed().as_secs_f64() * 1000.0;
        self.calls += 1;
        self.rows += rows.len();
        for row in &rows {
            for i in 0..row.len() {
                if let Some(raw) = row.get::<_, Option<Raw<'_>>>(i) {
                    self.payload_bytes += raw.0.len();
                }
            }
        }
        if self.collect_plans && !self.plans.iter().any(|plan| plan["sql"] == sql) {
            self.plan(db, sql, params)?;
        }
        Ok(rows)
    }

    pub fn plan(
        &mut self,
        db: &mut Client,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<()> {
        let explain =
            format!("EXPLAIN (ANALYZE, BUFFERS, SETTINGS, SERIALIZE TEXT, FORMAT JSON) {sql}");
        let row = db.query_one(&explain, params)?;
        let plan: Value = row.get(0);
        self.plans.push(json!({"sql":sql,"plan":plan}));
        Ok(())
    }
}

pub fn counters(db: &mut Client) -> Result<Value> {
    let row = db.query_one("SELECT pg_read_file('/sys/fs/cgroup/cpu.stat'), pg_read_file('/sys/fs/cgroup/memory.current'), pg_read_file('/sys/fs/cgroup/memory.peak'), pg_read_file('/sys/fs/cgroup/memory.stat')", &[])?;
    let cpu: String = row.get(0);
    let memory: String = row.get(1);
    let peak: String = row.get(2);
    let stat: String = row.get(3);
    fn number(text: &str, key: &str) -> u64 {
        text.lines()
            .find_map(|line| {
                let mut values = line.split_whitespace();
                if values.next()? == key {
                    values.next()?.parse().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0)
    }
    Ok(
        json!({"cpu_usage_usec":number(&cpu,"usage_usec"),"cpu_user_usec":number(&cpu,"user_usec"),"cpu_system_usec":number(&cpu,"system_usec"),"memory_current_bytes":memory.trim().parse::<u64>()?,"memory_lifetime_peak_bytes":peak.trim().parse::<u64>()?,"memory_anon_bytes":number(&stat,"anon"),"memory_file_bytes":number(&stat,"file")}),
    )
}

pub fn prepare(db: &mut Client, arm: &str) -> Result<Value> {
    let version: String = db.query_one("SELECT version()", &[])?.get(0);
    let empty_bytes: i64 = db
        .query_one("SELECT pg_database_size(current_database())", &[])?
        .get(0);
    let before = counters(db)?;
    let started = Instant::now();
    if arm.starts_with('P') {
        db.batch_execute("CREATE EXTENSION postgis;")?;
    }
    let extension_version = if arm.starts_with('P') {
        Some(
            db.query_one("SELECT postgis_full_version()", &[])?
                .get::<_, String>(0),
        )
    } else {
        None
    };
    if arm.ends_with('1') {
        prepare_projection(db, arm, true)?;
        prepare_projection(db, arm, false)?;
        db.batch_execute("CREATE INDEX projection_owner_sport_date ON postgis_eval.activity_projection(user_id,sport,started_at); CREATE INDEX projection_minx ON postgis_eval.activity_projection(minx); ANALYZE postgis_eval.activity_projection;")?;
    }
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let after = counters(db)?;
    let counts = db.query_one("SELECT (SELECT count(*) FROM postgis_eval.activities), (SELECT count(*) FROM postgis_eval.segments), (SELECT count(*) FROM postgis_eval.segment_efforts), (SELECT sum(coalesce(json_array_length(derived_data_json->'route_points'),0)) FROM postgis_eval.activities)", &[])?;
    let relations: Value = db.query_one("SELECT jsonb_agg(jsonb_build_object('table',c.relname,'heap_bytes',pg_relation_size(c.oid),'table_including_toast_bytes',pg_table_size(c.oid),'indexes_bytes',pg_indexes_size(c.oid),'total_bytes',pg_total_relation_size(c.oid)) ORDER BY c.relname) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='postgis_eval' AND c.relkind='r'", &[])?.get(0);
    let bytes: i64 = db
        .query_one("SELECT pg_database_size(current_database())", &[])?
        .get(0);
    Ok(
        json!({"arm":arm,"postgres":version,"postgis":extension_version,"raw_database_bytes":empty_bytes,"database_bytes":bytes,"prepare_elapsed_ms":elapsed_ms,"prepare_cpu_ms":(after["cpu_usage_usec"].as_u64().unwrap()-before["cpu_usage_usec"].as_u64().unwrap()) as f64/1000.0,"activities":counts.get::<_,i64>(0),"segments":counts.get::<_,i64>(1),"efforts":counts.get::<_,i64>(2),"route_points":counts.get::<_,i64>(3),"relations":relations}),
    )
}

fn prepare_projection(db: &mut Client, arm: &str, activities: bool) -> Result<()> {
    let (table, projection, selected) = if activities {
        (
            "activities",
            "activity_projection",
            "id,user_id,sport,started_at::text,derived_data_json",
        )
    } else {
        ("segments", "segment_projection", "id,route_data_json")
    };
    let extra = if activities {
        "user_id integer,sport text,started_at timestamptz,"
    } else {
        ""
    };
    let data = if arm == "V1" {
        "coords bytea"
    } else {
        "geom geometry(LineString,4326)"
    };
    db.batch_execute(&format!("CREATE TABLE postgis_eval.{projection}(id integer PRIMARY KEY,{extra} minx double precision,maxx double precision,miny double precision,maxy double precision,startx double precision,starty double precision,endx double precision,endy double precision,{data});"))?;
    let extra_columns = if activities {
        ",user_id,sport,started_at"
    } else {
        ""
    };
    let extra_values = if activities {
        ",$11,$12,$13::text::timestamptz"
    } else {
        ""
    };
    let route_column = if arm == "V1" { "coords" } else { "geom" };
    let route_value = if arm == "V1" {
        "$10"
    } else {
        "ST_GeomFromWKB($10,4326)"
    };
    let insert = db.prepare(&format!("INSERT INTO postgis_eval.{projection}(id,minx,maxx,miny,maxy,startx,starty,endx,endy,{route_column}{extra_columns}) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,{route_value}{extra_values})"))?;
    let mut after = 0;
    loop {
        let rows = db.query(
            &format!(
                "SELECT {selected} FROM postgis_eval.{table} WHERE id>$1 ORDER BY id LIMIT 16"
            ),
            &[&after],
        )?;
        if rows.is_empty() {
            break;
        }
        after = rows.last().unwrap().get("id");
        for row in rows {
            // Import projections use exactly the current Rust JSON decoder.
            // SQL's decimal-to-float conversion can differ by one ULP; storing
            // these decoded coordinates once in binary preserves the oracle.
            let points = if activities {
                workloads::activity(&row)?
            } else {
                workloads::segment(&row)?
            };
            let coords = workloads::coords(&points);
            if coords.len() < 2 {
                continue;
            }
            ensure!(
                coords.iter().all(|p| p[0].is_finite()
                    && p[1].is_finite()
                    && p[0].abs() <= 180.0
                    && p[1].abs() <= 90.0),
                "Projection contains invalid coordinates"
            );
            let minx = coords.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
            let maxx = coords
                .iter()
                .map(|p| p[0])
                .fold(f64::NEG_INFINITY, f64::max);
            let miny = coords.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
            let maxy = coords
                .iter()
                .map(|p| p[1])
                .fold(f64::NEG_INFINITY, f64::max);
            let start = coords.first().unwrap();
            let end = coords.last().unwrap();
            let bytes = geometry::wkb(&coords);
            let id: i32 = row.get("id");
            let mut values: Vec<&(dyn ToSql + Sync)> = vec![
                &id, &minx, &maxx, &miny, &maxy, &start[0], &start[1], &end[0], &end[1], &bytes,
            ];
            let owner: i32;
            let sport: String;
            let date: String;
            if activities {
                owner = row.get("user_id");
                sport = row.get("sport");
                date = row.get("started_at");
                values.extend([&owner as &(dyn ToSql + Sync), &sport, &date]);
            }
            db.execute(&insert, &values)?;
        }
    }
    if arm == "P1" {
        db.batch_execute(&format!("CREATE INDEX {projection}_geometry ON postgis_eval.{projection} USING gist (geom); CREATE INDEX {projection}_geography ON postgis_eval.{projection} USING gist ((geom::geography));"))?;
        if !activities {
            db.batch_execute(&format!("CREATE INDEX segment_start_geography ON postgis_eval.{projection} USING gist ((ST_StartPoint(geom)::geography)); CREATE INDEX segment_end_geography ON postgis_eval.{projection} USING gist ((ST_EndPoint(geom)::geography));"))?;
        }
    }
    db.batch_execute(&format!("ANALYZE postgis_eval.{projection};"))?;
    Ok(())
}
