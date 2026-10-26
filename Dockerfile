# syntax=docker/dockerfile:1

# 1. Frontend
FROM docker.io/library/node:24.22.1-alpine3.23 AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

# 2. API: a static musl binary, so the final image needs no libc
FROM docker.io/library/rust:1.99.1-alpine3.23 AS api
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
# Build the dependencies on their own first, so code changes don't rebuild them.
RUN mkdir src && echo 'fn main() {}' > src/main.rs && touch src/lib.rs && cargo build --release --locked && rm -rf src
COPY src ./src
RUN touch src/main.rs src/lib.rs && cargo build --release --locked

# 3. Runtime: distroless, non-root, nothing but the binary and the static files
FROM gcr.io/distroless/static-debian13:nonroot
COPY --from=api /src/target/release/subtitle-dashboard /app/subtitle-dashboard
COPY --from=web /web/dist /app/web
ENV DATA_DIR=/data STATIC_DIR=/app/web PORT=8080
USER nonroot:nonroot
EXPOSE 8080
HEALTHCHECK --interval=60s --timeout=5s --start-period=10s CMD ["/app/subtitle-dashboard", "--healthcheck"]
ENTRYPOINT ["/app/subtitle-dashboard"]
