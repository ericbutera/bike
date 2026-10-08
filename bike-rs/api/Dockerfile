ARG MISE_IMAGE
ARG RUST_IMAGE
ARG RUNTIME_IMAGE

FROM ${MISE_IMAGE:-scratch} AS protobuf
ARG PROTOC_VERSION
RUN mise use --global protoc@"${PROTOC_VERSION:?PROTOC_VERSION must be supplied by mise}" \
    && cp -a "$(mise where protoc@"${PROTOC_VERSION}")" /opt/protoc

FROM ${RUST_IMAGE:-scratch} AS chef
ARG CARGO_CHEF_REVISION
WORKDIR /app
RUN cargo install --git https://github.com/LukeMathWalker/cargo-chef --rev "${CARGO_CHEF_REVISION:?CARGO_CHEF_REVISION must be supplied by mise}" --locked cargo-chef

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

FROM ${RUNTIME_IMAGE:-scratch}
WORKDIR /app
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates libssl3t64 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/api /app/api
COPY --from=builder /app/target/release/migration /app/migration
COPY --from=builder /app/target/release/recover-generated-imports /app/recover-generated-imports
USER nobody

EXPOSE 3000
CMD ["/app/api"]
