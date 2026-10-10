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
RUN --mount=type=cache,id=bike-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=bike-cargo-git,target=/usr/local/cargo/git \
    cargo install --git https://github.com/LukeMathWalker/cargo-chef --rev "${CARGO_CHEF_REVISION:?CARGO_CHEF_REVISION must be supplied by mise}" --locked cargo-chef

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG TARGETPLATFORM
COPY --from=protobuf /opt/protoc/bin/protoc /usr/local/bin/protoc
COPY --from=protobuf /opt/protoc/include /usr/local/include
COPY --from=planner /app/recipe.json recipe.json
COPY --from=planner /app/vendor vendor
RUN --mount=type=cache,id=bike-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=bike-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=bike-rust-target-${TARGETPLATFORM},target=/app/target,sharing=locked \
    cargo chef cook --locked --release --recipe-path recipe.json
COPY . .
# Bake's API and worker targets share this single compilation. Copy binaries out
# of the mutable target cache before exporting either runtime image.
RUN --mount=type=cache,id=bike-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=bike-cargo-git,target=/usr/local/cargo/git \
    --mount=type=cache,id=bike-rust-target-${TARGETPLATFORM},target=/app/target,sharing=locked \
    cargo build --locked --release --bin api --bin worker --bin migration --bin recover-generated-imports --bin e2e-fixtures \
    && mkdir /out \
    && cp target/release/api target/release/worker target/release/migration target/release/recover-generated-imports target/release/e2e-fixtures /out/

FROM ${RUNTIME_IMAGE:-scratch} AS runtime
WORKDIR /app
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates libssl3t64 && rm -rf /var/lib/apt/lists/* \
    && install -d -o nobody -g nogroup /cache
USER nobody

FROM runtime AS worker
COPY --from=builder /out/worker /app/worker
CMD ["/app/worker"]

FROM runtime AS api
COPY --from=builder /out/api /app/api
COPY --from=builder /out/migration /app/migration
COPY --from=builder /out/recover-generated-imports /app/recover-generated-imports
COPY --from=builder /out/e2e-fixtures /app/e2e-fixtures
EXPOSE 3000
CMD ["/app/api"]
