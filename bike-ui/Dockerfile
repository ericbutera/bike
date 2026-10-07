ARG NODE_ALPINE_IMAGE=node:24.21.0-alpine3.24@sha256:ebfe2f90462722a7a4de65e91990e97fe0d401c70e0e762c5b53302f905ec1c1
ARG NPM_VERSION=12.2.0
ARG PNPM_VERSION=12.9.1

FROM ${NODE_ALPINE_IMAGE} AS base
ARG NPM_VERSION
ARG PNPM_VERSION
WORKDIR /app
ENV NEXT_TELEMETRY_DISABLED=1
RUN npm install -g "npm@${NPM_VERSION}"
RUN npm install -g "pnpm@${PNPM_VERSION}"

FROM base AS deps
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

FROM base AS builder
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN pnpm run build

FROM ${NODE_ALPINE_IMAGE} AS runner
WORKDIR /app
ENV NODE_ENV=production
ENV PORT=3000

RUN addgroup -S nextjs && adduser -S nextjs -G nextjs

COPY --from=builder /app/.next/standalone ./
COPY --from=builder /app/.next/static ./.next/static
COPY --from=builder /app/public ./public

USER nextjs
EXPOSE 3000
CMD ["node", "server.js"]
