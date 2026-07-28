FROM lukemathwalker/cargo-chef:latest-rust-1.93.0-bookworm AS chef
WORKDIR /app
ENV RUSTUP_TOOLCHAIN=1.93.0

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --locked --release --recipe-path recipe.json
COPY . .
RUN cargo build --locked --release --bin api --bin worker

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/api /app/api
USER nobody

EXPOSE 3000
CMD ["/app/api"]
