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
variable "GO_IMAGE" { default = null }
variable "IMAGE_REGISTRY" { default = "" }
variable "IMAGE_TAG" { default = "review" }

function "image" {
  params = [name]
  result = "${IMAGE_REGISTRY == "" ? "" : "${IMAGE_REGISTRY}/"}bike-${name == "maps" && IMAGE_REGISTRY == "" ? "map-renderer" : name}:${IMAGE_TAG}"
}

group "default" {
  targets = ["api", "worker", "ui", "maps", "e2e"]
}
group "rust" { targets = ["api", "worker"] }
target "checks" {
  dockerfile = "Dockerfile.checks"
  args = { MISE_IMAGE = MISE_IMAGE, RUST_IMAGE = RUST_IMAGE, DOCKER_CLI_IMAGE = DOCKER_CLI_IMAGE }
  tags = [image("checks")]
}
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
}
target "worker" {
  inherits = ["_rust"]
  target = "worker"
  tags = [image("worker")]
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
}
target "maps" {
  inherits = ["_node"]
  dockerfile = "map-renderer/Dockerfile"
  args = { NODE_IMAGE = NODE_IMAGE, GO_IMAGE = GO_IMAGE, RUNTIME_IMAGE = RUNTIME_IMAGE }
  tags = [image("maps")]
}
target "e2e" {
  inherits = ["_node"]
  dockerfile = "bike-ui/Dockerfile.e2e"
  args = { NODE_IMAGE = NODE_IMAGE, PLAYWRIGHT_IMAGE = PLAYWRIGHT_IMAGE, DOCKER_CLI_IMAGE = DOCKER_CLI_IMAGE }
  tags = [image("e2e")]
}
