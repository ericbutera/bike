ARG NODE_IMAGE=node:24.21.0-trixie@sha256:1278a37eb510ec1606fba0e80f554bcc941ae0b44f35ae373c4822ae7717c64d
ARG NPM_VERSION=12.2.0
ARG PLAYWRIGHT_IMAGE=mcr.microsoft.com/playwright:v1.63.0-noble@sha256:eff16c30e6f3f4af0a03fa4b706120d5e9b0891c344a27d64559aff5900a4a27

FROM ${NODE_IMAGE} AS node-toolchain
ARG NPM_VERSION
RUN npm install -g "npm@${NPM_VERSION}"
FROM ${PLAYWRIGHT_IMAGE}
# Keep the browser image's dependencies and use Bike's selected Node/npm.
COPY --from=node-toolchain /usr/local/ /usr/local/
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
