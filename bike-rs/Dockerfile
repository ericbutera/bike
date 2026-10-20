ARG MISE_IMAGE
ARG RUST_IMAGE
ARG RUNTIME_IMAGE

FROM ${MISE_IMAGE:-scratch} AS protobuf
ARG PROTOC_VERSION
RUN mise use --global protoc@"${PROTOC_VERSION:?PROTOC_VERSION must be supplied by mise}" \
    && cp -a "$(mise where protoc@"${PROTOC_VERSION}")" /opt/protoc

FROM ${RUST_IMAGE:-scratch} AS builder
WORKDIR /app
ENV RUSTFLAGS="-D warnings"
COPY --from=protobuf /opt/protoc/bin/protoc /usr/local/bin/protoc
COPY --from=protobuf /opt/protoc/include /usr/local/include
COPY . .
RUN cargo build --locked --release \
    --bin api --bin temporal-worker --bin migration \
    --bin recover-generated-imports --bin e2e-fixtures

FROM ${RUNTIME_IMAGE:-scratch} AS runtime
WORKDIR /app
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates libssl3t64 && rm -rf /var/lib/apt/lists/* \
    && install -d -o nobody -g nogroup /cache
USER nobody

FROM runtime AS backend
COPY --from=builder /app/target/release/api /app/api
COPY --from=builder /app/target/release/temporal-worker /app/temporal-worker
COPY --from=builder /app/target/release/migration /app/migration
COPY --from=builder /app/target/release/recover-generated-imports /app/recover-generated-imports
COPY --from=builder /app/target/release/e2e-fixtures /app/e2e-fixtures
EXPOSE 3000
CMD ["/app/api"]
