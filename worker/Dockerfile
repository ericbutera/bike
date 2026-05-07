FROM lukemathwalker/cargo-chef:latest-rust-1-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN sed -i '/^\[patch\."https:\/\/github\.com\/ericbutera\/kaleido"\]/,/^auth = { path/d' Cargo.toml
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin worker
COPY . .
RUN sed -i '/^\[patch\."https:\/\/github\.com\/ericbutera\/kaleido"\]/,/^auth = { path/d' Cargo.toml
RUN cargo build --release --bin worker

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/worker /app/worker
USER nobody

CMD ["/app/worker"]
