ARG GO_IMAGE
ARG RUNTIME_IMAGE

FROM ${GO_IMAGE:-scratch} AS build
ENV GOTOOLCHAIN=local
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 go build -o /out/gateway ./cmd/gateway && \
    CGO_ENABLED=0 go build -o /out/migrate ./cmd/migrate && \
    CGO_ENABLED=0 go build -o /out/worker ./cmd/worker && \
    CGO_ENABLED=0 go build -o /out/admin ./cmd/admin && \
    CGO_ENABLED=0 go build -o /out/import-rust ./cmd/import_rust && \
    CGO_ENABLED=0 go build -o /out/export-rust ./cmd/export_rust

FROM ${RUNTIME_IMAGE:-scratch}
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /out/gateway /app/gateway
COPY --from=build /out/migrate /app/migrate
COPY --from=build /out/worker /app/worker
COPY --from=build /out/admin /app/admin
COPY --from=build /out/import-rust /app/import-rust
COPY --from=build /out/export-rust /app/export-rust
EXPOSE 8080 50051
CMD ["/app/gateway"]
