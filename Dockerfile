FROM mcr.microsoft.com/playwright:v1.63.0-noble
WORKDIR /app
COPY map-renderer/package.json map-renderer/package-lock.json ./
RUN npm ci --omit=dev
COPY map-renderer/server.mjs map-renderer/cache.mjs map-renderer/request.mjs map-renderer/instrumentation.mjs map-renderer/logging.mjs map-renderer/error-log-record.mjs map-renderer/index.html map-renderer/smoke.mjs ./
COPY map-renderer/styles ./styles
COPY proto ./proto
ENV MAP_IMAGE_CACHE_DIR=/cache
ENV MAP_SERVICE_PROTO_PATH=/app/proto/bike/maps/v1/maps.proto
EXPOSE 3100 50051
HEALTHCHECK --interval=5s --timeout=3s --retries=12 CMD node -e "fetch('http://127.0.0.1:3100/healthz').then(response => process.exit(response.ok ? 0 : 1)).catch(() => process.exit(1))"
CMD ["node", "--import", "./instrumentation.mjs", "server.mjs"]
