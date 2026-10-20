// Versions come from mise's exported environment in local tasks and CI.
variable "RUST_IMAGE" { default = null }
variable "RUNTIME_IMAGE" { default = null }
variable "MISE_IMAGE" { default = null }
variable "PROTOC_VERSION" { default = null }
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
  targets = ["backend", "ui", "maps"]
}
target "checks" {
  inherits = ["_owned"]
  dockerfile = "Dockerfile.checks"
  args = { MISE_IMAGE = MISE_IMAGE, RUST_IMAGE = RUST_IMAGE, DOCKER_CLI_IMAGE = DOCKER_CLI_IMAGE }
  tags = [image("checks")]
}

target "_rust" {
  inherits = ["_owned"]
  context = "bike-rs"
  dockerfile = "Dockerfile"
  args = {
    RUST_IMAGE = RUST_IMAGE
    RUNTIME_IMAGE = RUNTIME_IMAGE
    MISE_IMAGE = MISE_IMAGE
    PROTOC_VERSION = PROTOC_VERSION
  }
}
target "backend" {
  inherits = ["_rust"]
  target = "backend"
  tags = [image("backend")]
}
target "_node" {
  inherits = ["_owned"]
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
target "e2e-tools" {
  inherits = ["_node"]
  dockerfile = "bike-ui/Dockerfile.e2e"
  args = { NODE_IMAGE = NODE_IMAGE, PLAYWRIGHT_IMAGE = PLAYWRIGHT_IMAGE, DOCKER_CLI_IMAGE = DOCKER_CLI_IMAGE }
  tags = [image("e2e-tools")]
}

target "_owned" {
  labels = { "dev.bike.storage" = "disposable-image" }
}
