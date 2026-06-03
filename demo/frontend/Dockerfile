# Build stage: install deps and build Astro static output.
# Expects build context = repo root (..) so demo/frontend and graphics/ are available.
# Symlinks public/logo.svg and public/m.svg -> ../../../graphics/* resolve to /graphics/*.
FROM node:alpine@sha256:144769ec3f32e8ee36b3cfde91e82bee25d9367b20f31a151f3f7eea3a2a8541 AS builder

RUN corepack enable 2>/dev/null || npm install -g pnpm

WORKDIR /app

# When building for Docker demo, frontend uses /api/martin for tiles/metrics (proxied to tiles service)
ARG PUBLIC_MARTIN_BASE_URL=/api/martin
ENV PUBLIC_MARTIN_BASE_URL=${PUBLIC_MARTIN_BASE_URL}

COPY demo/frontend/package.json demo/frontend/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile

COPY demo/frontend .
# Satisfy symlinks public/logo.svg, public/m.svg -> ../../../graphics/* (resolves to /graphics/*)
COPY graphics /graphics
RUN pnpm build

# Production stage: serve dist/ with nginx
FROM nginx:alpine@sha256:8b1e78743a03dbb2c95171cc58639fef29abc8816598e27fb910ed2e621e589a

COPY --from=builder /app/dist /usr/share/nginx/html
COPY demo/frontend/nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
