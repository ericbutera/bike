ARG RUST_VERSION=1.97.1
ARG RUST_BUILDER_IMAGE=lukemathwalker/cargo-chef:latest-rust-1.97.1-bookworm
ARG RUNTIME_IMAGE=debian:bookworm-slim

FROM ${RUST_BUILDER_IMAGE} AS chef
ARG RUST_VERSION
WORKDIR /app
ENV RUSTUP_TOOLCHAIN=${RUST_VERSION}

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
COPY --from=planner /app/vendor vendor
RUN cargo chef cook --locked --release --recipe-path recipe.json
COPY . .
RUN cargo build --locked --release --bin api --bin worker

FROM ${RUNTIME_IMAGE}
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/worker /app/worker
USER nobody

CMD ["/app/worker"]
