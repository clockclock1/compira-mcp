# syntax=docker/dockerfile:1.7
# Multi-arch: linux/amd64 + linux/arm64 (docker buildx)
# Frontend is arch-independent — build on BUILDPLATFORM.

ARG RUST_VERSION=1.88

# ── Frontend ─────────────────────────────────────────────────────────────────
FROM --platform=$BUILDPLATFORM node:22-bookworm-slim AS frontend
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json* ./
RUN npm ci --ignore-scripts || npm install
COPY frontend/ ./
RUN npm run build

# ── Backend (built for TARGETPLATFORM) ───────────────────────────────────────
FROM rust:${RUST_VERSION}-bookworm AS backend
WORKDIR /app/backend

RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential \
      cmake \
      pkg-config \
      perl \
      clang \
      libclang-dev \
    && rm -rf /var/lib/apt/lists/*

COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/backend/target \
    cargo build --release \
    && cp /app/backend/target/release/compira-mcp /tmp/compira-mcp

# ── Runtime ──────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates \
      git \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=backend /tmp/compira-mcp /app/compira-mcp
COPY --from=frontend /app/frontend/dist /app/static

ENV COMPIRA_STATIC_DIR=/app/static \
    COMPIRA_DATA_DIR=/app/data \
    COMPIRA_HOST=0.0.0.0 \
    COMPIRA_PORT=8080 \
    RUST_LOG=info,compira_mcp=info

EXPOSE 8080
VOLUME ["/app/data"]
CMD ["/app/compira-mcp"]
