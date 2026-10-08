ARG NODE_IMAGE
ARG NPM_VERSION
ARG PLAYWRIGHT_IMAGE

FROM ${NODE_IMAGE:-scratch} AS node-toolchain
ARG NPM_VERSION
ENV NODE_PACKAGE_CACHE=/var/cache/bike-node
RUN --mount=type=cache,id=bike-node-packages,target=/var/cache/bike-node \
    npm install --cache "${NODE_PACKAGE_CACHE}/npm" --prefer-offline -g "npm@${NPM_VERSION:?NPM_VERSION must be supplied by mise}"
FROM ${PLAYWRIGHT_IMAGE:-scratch}
# Keep the browser image's dependencies and use Bike's selected Node/npm.
COPY --from=node-toolchain /usr/local/ /usr/local/
WORKDIR /app
ENV NODE_PACKAGE_CACHE=/var/cache/bike-node
COPY map-renderer/package.json map-renderer/package-lock.json ./
RUN --mount=type=cache,id=bike-node-packages,target=/var/cache/bike-node \
    npm ci --cache "${NODE_PACKAGE_CACHE}/npm" --prefer-offline --omit=dev
COPY map-renderer/server.mjs map-renderer/cache.mjs map-renderer/request.mjs map-renderer/instrumentation.mjs map-renderer/logging.mjs map-renderer/error-log-record.mjs map-renderer/index.html map-renderer/smoke.mjs ./
COPY map-renderer/styles ./styles
COPY proto ./proto
ENV MAP_IMAGE_CACHE_DIR=/cache
ENV MAP_SERVICE_PROTO_PATH=/app/proto/bike/maps/v1/maps.proto
EXPOSE 3100 50051
HEALTHCHECK --interval=5s --timeout=3s --retries=12 CMD node -e "fetch('http://127.0.0.1:3100/healthz').then(response => process.exit(response.ok ? 0 : 1)).catch(() => process.exit(1))"
CMD ["node", "--import", "./instrumentation.mjs", "server.mjs"]
