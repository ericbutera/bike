fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure().compile_protos(&["../proto/bike/maps/v1/maps.proto"], &["../proto"])?;
    println!("cargo:rerun-if-changed=../proto/bike/maps/v1/maps.proto");
    Ok(())
}
