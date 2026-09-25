use api::openapi::ApiDoc;
use std::fs;
use std::path::PathBuf;
use utoipa::OpenApi;

fn main() {
    let document = ApiDoc::openapi();
    let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs/openapi");

    fs::create_dir_all(&output_dir).expect("create OpenAPI output directory");
    fs::write(
        output_dir.join("openapi.json"),
        serde_json::to_string_pretty(&document).expect("serialize OpenAPI JSON"),
    )
    .expect("write OpenAPI JSON");
    fs::write(
        output_dir.join("openapi.yaml"),
        document.to_yaml().expect("serialize OpenAPI YAML"),
    )
    .expect("write OpenAPI YAML");

    println!(
        "Generated {} and {}",
        output_dir.join("openapi.json").display(),
        output_dir.join("openapi.yaml").display()
    );
}
