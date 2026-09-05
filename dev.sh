#!/usr/bin/env bash
# Dev mode: backend (debug) + Vite HMR on Linux / macOS.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export COMPIRA_DATA_DIR="${COMPIRA_DATA_DIR:-$ROOT/data}"
export COMPIRA_HOST="${COMPIRA_HOST:-127.0.0.1}"
export COMPIRA_PORT="${COMPIRA_PORT:-8080}"
export RUST_LOG="${RUST_LOG:-info,compira_mcp=debug}"
mkdir -p "$COMPIRA_DATA_DIR"

cleanup() {
  jobs -p | xargs -r kill 2>/dev/null || true
}
trap cleanup EXIT INT TERM

echo "Starting CompiraMCP (dev mode)..."
echo "  Backend  : http://${COMPIRA_HOST}:${COMPIRA_PORT}"
echo "  Frontend : http://127.0.0.1:5173"
echo "  MCP      : http://${COMPIRA_HOST}:${COMPIRA_PORT}/mcp"
echo

(cd "$ROOT/backend" && cargo run) &
BACKEND_PID=$!

(cd "$ROOT/frontend" && npm run dev) &
FRONTEND_PID=$!

wait "$BACKEND_PID" "$FRONTEND_PID"
