// Versions come from mise's exported environment in local tasks and CI.
variable "RUST_IMAGE" { default = null }
variable "RUNTIME_IMAGE" { default = null }
variable "MISE_IMAGE" { default = null }
variable "PROTOC_VERSION" { default = null }
variable "CARGO_CHEF_REVISION" { default = null }
variable "NODE_IMAGE" { default = null }
variable "NODE_ALPINE_IMAGE" { default = null }
variable "NPM_VERSION" { default = null }
variable "PNPM_VERSION" { default = null }
variable "PLAYWRIGHT_IMAGE" { default = null }
variable "DOCKER_CLI_IMAGE" { default = null }
variable "DOCKER_DIND_IMAGE" { default = null }
variable "GO_IMAGE" { default = null }
variable "K6_IMAGE" { default = null }
variable "IMAGE_REGISTRY" { default = "" }
variable "IMAGE_TAG" { default = "review" }
variable "CACHE_REGISTRY" { default = "" }
variable "CACHE_SCOPE" { default = "main" }
// The persistent builder normally uses its own cache. Enable registry import
// explicitly when recovering a builder whose exported caches already exist.
variable "CACHE_IMPORT" { default = false }

function "image" {
  params = [name]
  result = "${IMAGE_REGISTRY == "" ? "" : "${IMAGE_REGISTRY}/"}bike-${name == "maps" && IMAGE_REGISTRY == "" ? "map-renderer" : name}:${IMAGE_TAG}"
}

function "cache_from" {
  params = [name]
  result = CACHE_REGISTRY == "" || !CACHE_IMPORT ? [] : distinct([
    "type=registry,ref=${CACHE_REGISTRY}/bike-build-cache:${name}-main",
    "type=registry,ref=${CACHE_REGISTRY}/bike-build-cache:${name}-${CACHE_SCOPE}",
  ])
}

function "cache_to" {
  params = [name]
  result = CACHE_REGISTRY == "" ? [] : ["type=registry,ref=${CACHE_REGISTRY}/bike-build-cache:${name}-${CACHE_SCOPE},mode=max"]
}

group "default" {
  targets = ["api", "worker", "ui", "maps", "strava-gateway", "e2e", "e2e-engine"]
}
group "rust" { targets = ["api", "worker"] }
group "e2e-runtime" { targets = ["api", "ui", "maps", "e2e"] }

target "_rust" {
  context = "bike-rs"
  dockerfile = "Dockerfile"
  args = {
    RUST_IMAGE = RUST_IMAGE
    RUNTIME_IMAGE = RUNTIME_IMAGE
    MISE_IMAGE = MISE_IMAGE
    PROTOC_VERSION = PROTOC_VERSION
    CARGO_CHEF_REVISION = CARGO_CHEF_REVISION
  }
}
target "api" {
  inherits = ["_rust"]
  target = "api"
  tags = [image("api")]
  cache-from = cache_from("api")
  cache-to = cache_to("api")
}
target "worker" {
  inherits = ["_rust"]
  target = "worker"
  tags = [image("worker")]
  cache-from = cache_from("worker")
  cache-to = cache_to("worker")
}
target "_node" {
  args = { NPM_VERSION = NPM_VERSION, PNPM_VERSION = PNPM_VERSION }
}
target "ui" {
  inherits = ["_node"]
  context = "bike-ui"
  dockerfile = "Dockerfile"
  args = { NODE_ALPINE_IMAGE = NODE_ALPINE_IMAGE }
  tags = [image("ui")]
  cache-from = cache_from("ui")
  cache-to = cache_to("ui")
}
target "maps" {
  inherits = ["_node"]
  dockerfile = "map-renderer/Dockerfile"
  args = { NODE_IMAGE = NODE_IMAGE, GO_IMAGE = GO_IMAGE, RUNTIME_IMAGE = RUNTIME_IMAGE }
  tags = [image("maps")]
  cache-from = cache_from("maps")
  cache-to = cache_to("maps")
}
target "strava-gateway" {
  context = "strava-gateway"
  args = { GO_IMAGE = GO_IMAGE, RUNTIME_IMAGE = RUNTIME_IMAGE }
  tags = [image("strava-gateway")]
  cache-from = cache_from("strava-gateway")
  cache-to = cache_to("strava-gateway")
}
target "e2e" {
  inherits = ["_node"]
  dockerfile = "bike-ui/Dockerfile.e2e"
  args = { NODE_IMAGE = NODE_IMAGE, PLAYWRIGHT_IMAGE = PLAYWRIGHT_IMAGE, DOCKER_CLI_IMAGE = DOCKER_CLI_IMAGE }
  tags = [image("e2e")]
  cache-from = cache_from("e2e")
  cache-to = cache_to("e2e")
}
target "e2e-engine" {
  dockerfile = "bike-ui/Dockerfile.e2e-engine"
  args = { DOCKER_DIND_IMAGE = DOCKER_DIND_IMAGE }
  tags = [image("e2e-engine")]
  cache-from = cache_from("e2e-engine")
  cache-to = cache_to("e2e-engine")
}
target "synthetics" {
  context = "integration-tests"
  args = { K6_IMAGE = K6_IMAGE }
  tags = [image("synthetics")]
}
