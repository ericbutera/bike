ARG NODE_IMAGE
ARG NPM_VERSION
ARG GO_IMAGE
ARG RUNTIME_IMAGE

FROM ${NODE_IMAGE:-scratch} AS assets
ARG NPM_VERSION
WORKDIR /assets
ENV NODE_PACKAGE_CACHE=/var/cache/bike-node
RUN --mount=type=cache,id=bike-node-packages,target=/var/cache/bike-node \
    npm install --cache "${NODE_PACKAGE_CACHE}/npm" --prefer-offline -g "npm@${NPM_VERSION:?NPM_VERSION must be supplied by mise}"
COPY map-renderer/package.json map-renderer/package-lock.json ./
RUN --mount=type=cache,id=bike-node-packages,target=/var/cache/bike-node \
    npm ci --cache "${NODE_PACKAGE_CACHE}/npm" --prefer-offline --omit=dev
FROM ${GO_IMAGE:-scratch} AS build
ARG TARGETPLATFORM
ENV GOTOOLCHAIN=local
WORKDIR /src
COPY map-renderer/go.mod map-renderer/go.sum ./
RUN --mount=type=cache,id=bike-go-modules,target=/go/pkg/mod go mod download
COPY map-renderer/cmd ./cmd
COPY map-renderer/internal ./internal
COPY map-renderer/gen ./gen
RUN --mount=type=cache,id=bike-go-modules,target=/go/pkg/mod \
    --mount=type=cache,id=bike-go-build-${TARGETPLATFORM},target=/root/.cache/go-build \
    CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o /out/renderer ./cmd/renderer

FROM assets AS browser
RUN --mount=type=cache,id=bike-node-packages,target=/var/cache/bike-node \
    npm ci --cache "${NODE_PACKAGE_CACHE}/npm" --prefer-offline
RUN --mount=type=cache,id=bike-chromium-headless-shell,target=/var/cache/bike-chromium \
    PLAYWRIGHT_BROWSERS_PATH=/var/cache/bike-chromium \
    node node_modules/playwright/cli.js install chromium --only-shell \
    && mkdir -p /out/chromium \
    && cp -a /var/cache/bike-chromium/chromium_headless_shell-*/chrome-headless-shell-linux*/. /out/chromium/

# The build downloads only Chromium's headless shell; Node stays in build stages.
FROM ${RUNTIME_IMAGE:-scratch}
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates fontconfig fonts-freefont-ttf fonts-ipafont-gothic fonts-liberation \
    fonts-noto-color-emoji fonts-tlwg-loma-otf fonts-unifont fonts-wqy-zenhei \
    libasound2t64 libatk-bridge2.0-0t64 libatk1.0-0t64 \
    libcairo2 libcups2t64 libdbus-1-3 libgbm1 libglib2.0-0t64 libgtk-3-0t64 \
    libnspr4 libnss3 libpango-1.0-0 libudev1 libvulkan1 libx11-6 libxcb1 \
    libxcomposite1 libxdamage1 libxext6 libxfixes3 libxkbcommon0 libxrandr2 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=browser /out/chromium /opt/chromium/
WORKDIR /app
COPY --from=build /out/renderer /app/renderer
COPY --from=assets /assets/node_modules/maplibre-gl/dist/maplibre-gl.css /assets/node_modules/maplibre-gl/dist/maplibre-gl.mjs /assets/node_modules/maplibre-gl/dist/maplibre-gl-shared.mjs /assets/node_modules/maplibre-gl/dist/maplibre-gl-worker.mjs /app/vendor/
COPY map-renderer/index.html /app/index.html
COPY map-renderer/styles /app/styles
ENV CHROME_EXECUTABLE=/opt/chromium/chrome-headless-shell
EXPOSE 50051 9090
HEALTHCHECK --interval=5s --timeout=3s --retries=12 CMD ["/app/renderer", "healthcheck"]
CMD ["/app/renderer"]
