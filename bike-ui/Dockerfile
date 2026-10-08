ARG NODE_ALPINE_IMAGE
ARG NPM_VERSION
ARG PNPM_VERSION

FROM ${NODE_ALPINE_IMAGE:-scratch} AS base
ARG NPM_VERSION
ARG PNPM_VERSION
WORKDIR /app
ENV NEXT_TELEMETRY_DISABLED=1
RUN npm install -g "npm@${NPM_VERSION:?NPM_VERSION must be supplied by mise}"
RUN npm install -g --allow-scripts=pnpm "pnpm@${PNPM_VERSION:?PNPM_VERSION must be supplied by mise}"

FROM base AS deps
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

FROM base AS builder
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN pnpm run build

FROM ${NODE_ALPINE_IMAGE:-scratch} AS runner
WORKDIR /app
ENV NODE_ENV=production
ENV PORT=3000

RUN addgroup -S nextjs && adduser -S nextjs -G nextjs

COPY --from=builder --chown=nextjs:nextjs /app/.next/standalone ./
COPY --from=builder --chown=nextjs:nextjs /app/.next/static ./.next/static
COPY --from=builder /app/public ./public

USER nextjs
EXPOSE 3000
CMD ["node", "server.js"]
