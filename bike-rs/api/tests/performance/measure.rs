use super::{checks, dataset};
use reqwest::Client;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::Serialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const REQUESTS: usize = 200;

#[derive(Clone, Copy, Default, Serialize)]
struct SqlTotals {
    calls: i64,
    execution_ms: f64,
    rows: i64,
}

#[derive(Serialize)]
struct Run {
    scenario: String,
    run: usize,
    requests: usize,
    errors: usize,
    p50_ms: f64,
    p95_ms: f64,
    elapsed_ms: f64,
    requests_per_second: f64,
    response_bytes_min: usize,
    response_bytes_max: usize,
    sql: SqlTotals,
}

struct Sample {
    elapsed_ms: f64,
    bytes: usize,
    valid: bool,
}

pub async fn measure(db: &DatabaseConnection, base_url: &str) -> Value {
    let client = Client::builder().timeout(Duration::from_secs(5)).build().unwrap();
    let availability = isolation_checks(&client, base_url).await;
    let mut report = measure_scenarios(db, &client, base_url, &checks::SCENARIOS).await;
    report["availability"] = availability;
    if std::env::var("BIKE_PERF_RANK_GROWTH").as_deref() == Ok("1") {
        grow_unrelated_segment_history(db).await;
        report["grown_history"] = measure_scenarios(db, &client, base_url, &checks::SCENARIOS[1..4]).await;
        report["grown_history"]["segment_efforts"] = 1926.into();
    }
    report
}

async fn measure_scenarios(
    db: &DatabaseConnection,
    client: &Client,
    base_url: &str,
    scenarios: &[(&str, &str)],
) -> Value {
    let mut runs = Vec::new();
    let mut first = Vec::new();
    for &(scenario, path) in scenarios {
        let url = format!("{base_url}{path}");
        let sample = request(client, &url, scenario).await;
        assert!(sample.valid, "first request failed correctness: {scenario}");
        first.push(json!({"scenario": scenario, "elapsed_ms": sample.elapsed_ms, "bytes": sample.bytes}));
        for _ in 0..10 {
            assert!(request(client, &url, scenario).await.valid);
        }
        for run in 1..=3 {
            runs.push(measure_run(db, client, &url, scenario, run).await);
        }
    }
    json!({"first_request": first, "runs": runs, "requests_per_run": REQUESTS,
        "warmups": 10, "concurrency": 1, "request_timeout_seconds": 5,
        "unmeasured": ["decode/build/serialization stage times", "peak memory", "process CPU"],
        "cache_state": "first request separate; warmups before each scenario; database and OS caches not reset"})
}

async fn measure_run(db: &DatabaseConnection, client: &Client, url: &str, scenario: &str, run: usize) -> Run {
    let before = sql_totals(db).await;
    let started = Instant::now();
    let mut samples = Vec::with_capacity(REQUESTS);
    for _ in 0..REQUESTS {
        samples.push(request(client, url, scenario).await);
        if samples.iter().filter(|sample| !sample.valid).count() >= 3 {
            break;
        }
        if started.elapsed() > Duration::from_secs(120) {
            break;
        }
    }
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let after = sql_totals(db).await;
    let mut durations: Vec<_> = samples.iter().map(|sample| sample.elapsed_ms).collect();
    durations.sort_by(f64::total_cmp);
    Run {
        scenario: scenario.into(),
        run,
        requests: samples.len(),
        errors: samples.iter().filter(|sample| !sample.valid).count(),
        p50_ms: percentile(&durations, 50),
        p95_ms: percentile(&durations, 95),
        elapsed_ms,
        requests_per_second: samples.len() as f64 * 1000.0 / elapsed_ms,
        response_bytes_min: samples.iter().map(|sample| sample.bytes).min().unwrap(),
        response_bytes_max: samples.iter().map(|sample| sample.bytes).max().unwrap(),
        sql: SqlTotals {
            calls: after.calls - before.calls,
            execution_ms: after.execution_ms - before.execution_ms,
            rows: after.rows - before.rows,
        },
    }
}

pub fn percentile(sorted: &[f64], percent: usize) -> f64 {
    assert!(!sorted.is_empty());
    sorted[(sorted.len() * percent).div_ceil(100).saturating_sub(1)]
}

async fn request(client: &Client, url: &str, scenario: &str) -> Sample {
    let start = Instant::now();
    let result = read_response(client, url).await;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    match result {
        Ok((status, bytes)) => Sample {
            elapsed_ms,
            bytes: bytes.len(),
            valid: status == 200 && serde_json::from_slice(&bytes).is_ok_and(|body| checks::valid(scenario, &body)),
        },
        Err(error) => {
            eprintln!("{scenario}: {error}");
            Sample {
                elapsed_ms,
                bytes: 0,
                valid: false,
            }
        }
    }
}

async fn read_response(client: &Client, url: &str) -> Result<(u16, Vec<u8>), reqwest::Error> {
    let response = client
        .get(url)
        .header("cookie", format!("refresh_token={}", dataset::SESSION))
        .send()
        .await?;
    let status = response.status().as_u16();
    Ok((status, response.bytes().await?.to_vec()))
}

async fn sql_totals(db: &DatabaseConnection) -> SqlTotals {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            r#"
        SELECT COALESCE(sum(calls), 0)::bigint AS calls,
               COALESCE(sum(total_exec_time), 0)::double precision AS execution_ms,
               COALESCE(sum(rows), 0)::bigint AS rows
        FROM pg_stat_statements
        WHERE dbid = (SELECT oid FROM pg_database WHERE datname = current_database())
          AND query NOT LIKE '%pg_stat_statements%'
    "#,
        ))
        .await
        .unwrap()
        .unwrap();
    SqlTotals {
        calls: row.try_get("", "calls").unwrap(),
        execution_ms: row.try_get("", "execution_ms").unwrap(),
        rows: row.try_get("", "rows").unwrap(),
    }
}

async fn isolation_checks(client: &Client, base: &str) -> Value {
    for path in ["/api/activities/2100", "/api/segments/2", "/api/segments/2/comparison"] {
        let (status, _) = read_response(client, &format!("{base}{path}")).await.unwrap();
        assert_eq!(status, 404, "cross-owner read: {path}");
    }
    let anonymous = client.get(format!("{base}/api/activities")).send().await.unwrap();
    assert_eq!(anonymous.status(), 401);
    result_availability_checks(client, base).await
}

async fn result_availability_checks(client: &Client, base: &str) -> Value {
    let mut results = Vec::new();
    for id in [1001, 1002, 1003, 1100] {
        let (status, bytes) = read_response(client, &format!("{base}/api/activities/{id}"))
            .await
            .unwrap();
        assert_eq!(status, 200);
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["id"], id);
        let count = body["segment_efforts"].as_array().map_or(0, Vec::len);
        assert_eq!(count, if id == 1001 { 0 } else { 2 });
        // Baseline revisions predate the state field. Candidates must report the
        // fixture's state and preserve null ranks rather than reconstructing them.
        if let Some(state) = body.get("segment_ranks_state") {
            let expected = match id {
                1001 | 1002 => "missing",
                1003 => "updating",
                _ => "ready",
            };
            assert_eq!(state, expected);
            if id == 1002 {
                assert!(body["segment_efforts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|effort| effort["overall_rank"].is_null() && effort["personal_rank"].is_null()));
            }
        }
        results.push(json!({"id": id, "state": body["segment_ranks_state"],
            "efforts": body["segment_efforts"]}));
    }
    json!(results)
}

// A small history-growth probe for PERF01b, not the PERF09 capacity profile.
// Add slower repeated attempts only to other activities; selected efforts,
// their ranks, route payloads, and the owner's activity count remain unchanged.
async fn grow_unrelated_segment_history(db: &DatabaseConnection) {
    let result = db
        .execute_raw(Statement::from_string(
            DbBackend::Postgres,
            r#"
        INSERT INTO segment_efforts (
            id, user_id, segment_id, activity_id, effort_index,
            start_route_point_index, end_route_point_index,
            start_elapsed_seconds, end_elapsed_seconds, duration_seconds,
            distance_meters, overall_rank, user_rank, created_at, updated_at
        )
        SELECT (SELECT max(id) FROM segment_efforts) + row_number() OVER (ORDER BY e.id, n),
               e.user_id, e.segment_id, e.activity_id, e.effort_index + n * 2,
               e.start_route_point_index, e.end_route_point_index,
               e.start_elapsed_seconds, e.start_elapsed_seconds + 1000 + n, 1000 + n,
               e.distance_meters, 198 + row_number() OVER (ORDER BY e.id, n),
               198 + row_number() OVER (ORDER BY e.id, n), e.created_at, e.updated_at
        FROM segment_efforts e CROSS JOIN generate_series(1, 9) n
        WHERE e.segment_id = 1 AND e.activity_id NOT IN (1002, 1003, 1100)
    "#,
        ))
        .await
        .unwrap();
    assert_eq!(result.rows_affected(), 1728);
    db.execute_raw(Statement::from_string(DbBackend::Postgres, "ANALYZE segment_efforts"))
        .await
        .unwrap();
}
