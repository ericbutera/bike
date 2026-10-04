#[allow(dead_code)]
mod activity_data {
    include!(concat!(env!("OUT_DIR"), "/activity_data.rs"));
}
#[allow(dead_code)]
mod baseline {
    include!(concat!(env!("OUT_DIR"), "/baseline.rs"));
}
mod database;
mod geometry;
mod workloads;

use anyhow::{ensure, Result};
use postgres::{Client, NoTls};
use serde_json::json;
use std::{env, fs, io::Write, time::Instant};

fn usage() -> (f64, u64) {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    let result = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    assert_eq!(result, 0);
    let r = unsafe { usage.assume_init() };
    (
        (r.ru_utime.tv_sec + r.ru_stime.tv_sec) as f64 * 1000.0
            + (r.ru_utime.tv_usec + r.ru_stime.tv_usec) as f64 / 1000.0,
        r.ru_maxrss as u64 * 1024,
    )
}
fn rss() -> u64 {
    fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
        })
        .unwrap_or(0)
        * 1024
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.get(1).is_some_and(|command| command == "metadata") {
        println!(
            "{}",
            json!({"rustc":env!("PROBE_RUSTC"),"target":env!("PROBE_TARGET")})
        );
        return Ok(());
    }
    ensure!(args.len() >= 4, "prepare ARM OUTPUT or run ARM CASE OUTPUT");
    let arm = &args[2];
    ensure!(
        ["V0", "V1", "P0", "P1"].contains(&arm.as_str()),
        "Unknown arm"
    );
    let mut db = Client::connect(&env::var("DATABASE_URL")?, NoTls)?;
    db.batch_execute("SET search_path TO postgis_eval,public; SET statement_timeout='120s';")?;
    if args[1] == "prepare" {
        let cpu_before = usage().0;
        let mut preparation = database::prepare(&mut db, arm)?;
        preparation["prepare_rust_cpu_ms"] = json!(usage().0 - cpu_before);
        preparation["prepare_rust_peak_rss_bytes"] = json!(usage().1);
        fs::write(&args[3], serde_json::to_vec_pretty(&preparation)?)?;
        return Ok(());
    }
    ensure!(args[1] == "run" && args.len() == 5, "Unknown command");
    let case: workloads::Case = serde_json::from_slice(&fs::read(&args[3])?)?;
    ensure!(
        case.radius >= baseline::minimum_prefilter_radius(),
        "Prefilter radius is smaller than current Rust's endpoint tolerance plus margin"
    );
    for _ in 0..case.warmups {
        workloads::execute(&mut db, arm, &case, false)?;
    }
    let before = database::counters(&mut db)?;
    let rss_before = rss();
    let cpu_before = usage().0;
    let mut observations = Vec::new();
    let mut fingerprint = None;
    for sample in 0..case.samples {
        let started = Instant::now();
        let cpu = usage().0;
        let result = workloads::execute(&mut db, arm, &case, false)?;
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        let rust_cpu_ms = usage().0 - cpu;
        ensure!(
            fingerprint
                .as_ref()
                .is_none_or(|previous| previous == &result.fingerprint),
            "Output changed during one batch"
        );
        fingerprint = Some(result.fingerprint.clone());
        observations.push(json!({"sample":sample,"elapsed_ms":elapsed_ms,"rust_cpu_ms":rust_cpu_ms,"sql_transfer_ms":result.access.sql_ms,"decode_ms":result.decode_ms,"domain_ms":result.domain_ms,"output_ms":result.output_ms,"sql_calls":result.access.calls,"rows":result.access.rows,"candidate_rows":result.candidate_rows,"vertices":result.vertices,"payload_bytes":result.access.payload_bytes,"output_bytes":result.output_bytes,"fingerprint":result.fingerprint}));
    }
    let cpu_after = usage().0;
    let rss_after = rss();
    let peak = usage().1;
    let after = database::counters(&mut db)?;
    let cache = if case.warmups == 0 {
        "post-restore state; no per-case warmup; fresh Rust process; no application cache"
    } else {
        "configured per-case warmup; fresh Rust process; no application cache"
    };
    let result = json!({"arm":arm,"case":case.name,"kind":case.kind,"observations":observations,"pg_before":before,"pg_after":after,"pg_batch_cpu_ms":(after["cpu_usage_usec"].as_u64().unwrap()-before["cpu_usage_usec"].as_u64().unwrap()) as f64/1000.0,"rust_batch_cpu_ms":cpu_after-cpu_before,"rust_rss_before_bytes":rss_before,"rust_rss_after_bytes":rss_after,"rust_process_peak_rss_bytes":peak,"cache":cache});
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&args[4])?;
    file.write_all(&serde_json::to_vec_pretty(&result)?)?;
    let plans = workloads::execute(&mut db, arm, &case, true)?.access.plans;
    fs::write(
        format!("{}.plans.json", &args[4]),
        serde_json::to_vec_pretty(&plans)?,
    )?;
    println!(
        "{} {}: {} observation(s), fingerprint stable",
        arm, case.name, case.samples
    );
    Ok(())
}
