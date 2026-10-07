fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(false)
        .compile_protos(
            &["../proto/bike/strava/v1/gateway.proto"],
            &["../proto/bike/strava/v1"],
        )?;
    println!("cargo:rerun-if-changed=../proto/bike/strava/v1/gateway.proto");
    Ok(())
}
