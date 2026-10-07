ARG MISE_IMAGE=ghcr.io/jdx/mise:2026.10.3-debian@sha256:58c4c847f5518a9a87a9a582886a485443dbca5eb024426577f58d586a0990c6
ARG RUST_IMAGE=rust:1.99.0-trixie@sha256:6ff07edce8775d0f64be7aba9197229407301bddf2054d62c27b541a6238a181
ARG RUNTIME_IMAGE=debian:trixie-slim@sha256:a29215f6a35e51e22adffa17f89e9d2ef06214e64a2bad10d765c46aea49f11f

FROM ${MISE_IMAGE} AS protobuf
ARG PROTOC_VERSION=36.2
RUN mise use --global protoc@"${PROTOC_VERSION}" \
    && cp -a "$(mise where protoc@"${PROTOC_VERSION}")" /opt/protoc

FROM ${RUST_IMAGE} AS chef
ARG CARGO_CHEF_REVISION=449576bbc2645200936adb9dece80810c9a335f8
WORKDIR /app
RUN cargo install --git https://github.com/LukeMathWalker/cargo-chef --rev "${CARGO_CHEF_REVISION}" --locked cargo-chef

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
COPY --from=planner /app/vendor vendor
RUN cargo chef cook --locked --release --recipe-path recipe.json
COPY --from=protobuf /opt/protoc/bin/protoc /usr/local/bin/protoc
COPY --from=protobuf /opt/protoc/include /usr/local/include
COPY . .
RUN cargo build --locked --release --bin api --bin worker --bin migration --bin recover-generated-imports

FROM ${RUNTIME_IMAGE}
WORKDIR /app
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates libssl3t64 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/worker /app/worker
USER nobody

CMD ["/app/worker"]
