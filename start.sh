#!/usr/bin/env bash
# Start CompiraMCP (release) on Linux / macOS (x86_64 or arm64).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export COMPIRA_DATA_DIR="${COMPIRA_DATA_DIR:-$ROOT/data}"
export COMPIRA_STATIC_DIR="${COMPIRA_STATIC_DIR:-$ROOT/frontend/dist}"
export COMPIRA_PORT="${COMPIRA_PORT:-8080}"
mkdir -p "$COMPIRA_DATA_DIR"

if [[ ! -d "$COMPIRA_STATIC_DIR" ]]; then
  echo "Building frontend..."
  (cd "$ROOT/frontend" && npm install && npm run build)
fi

echo "Starting CompiraMCP on http://localhost:${COMPIRA_PORT}"
echo "  data:   $COMPIRA_DATA_DIR"
echo "  static: $COMPIRA_STATIC_DIR"
cd "$ROOT/backend"
exec cargo run --release
