FROM mcr.microsoft.com/playwright:v1.63.0-noble
WORKDIR /app
COPY map-renderer/package.json map-renderer/package-lock.json ./
RUN npm ci --omit=dev
COPY map-renderer/server.mjs map-renderer/cache.mjs map-renderer/index.html map-renderer/smoke.mjs ./
COPY map-renderer/styles ./styles
ENV MAP_IMAGE_CACHE_DIR=/cache
EXPOSE 3100
HEALTHCHECK --interval=5s --timeout=3s --retries=12 CMD node -e "fetch('http://127.0.0.1:3100/').then(response => process.exit(response.ok ? 0 : 1)).catch(() => process.exit(1))"
CMD ["node", "server.mjs"]
