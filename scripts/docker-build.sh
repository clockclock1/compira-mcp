#!/usr/bin/env bash
# Build multi-arch Docker images (linux/amd64 + linux/arm64).
#
# Usage:
#   ./scripts/docker-build.sh                  # local host arch → --load
#   ./scripts/docker-build.sh --multi          # amd64+arm64 → registry (needs --push / IMAGE)
#   IMAGE=ghcr.io/org/compira-mcp:latest ./scripts/docker-build.sh --multi --push
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

IMAGE="${IMAGE:-compira-mcp:local}"
MULTI=0
PUSH=0
PLATFORMS="linux/amd64,linux/arm64"

for arg in "$@"; do
  case "$arg" in
    --multi) MULTI=1 ;;
    --push) PUSH=1 ;;
    --help|-h)
      sed -n '2,10p' "$0"
      exit 0
      ;;
  esac
done

if ! docker buildx version >/dev/null 2>&1; then
  echo "docker buildx is required" >&2
  exit 1
fi

# Ensure a builder that supports multi-platform
if ! docker buildx inspect compira-builder >/dev/null 2>&1; then
  docker buildx create --name compira-builder --driver docker-container --use
else
  docker buildx use compira-builder
fi
docker buildx inspect --bootstrap >/dev/null

if [[ "$MULTI" -eq 1 ]]; then
  OUTPUT=(--platform "$PLATFORMS")
  if [[ "$PUSH" -eq 1 ]]; then
    OUTPUT+=(--push)
  else
    echo "Multi-arch build without --push stores in buildx cache only."
    echo "Re-run with --push and IMAGE=registry/name:tag to publish."
    OUTPUT+=(--output "type=image,name=${IMAGE},push=false")
  fi
  echo "Building ${IMAGE} for ${PLATFORMS}..."
  docker buildx build -t "$IMAGE" "${OUTPUT[@]}" .
else
  ARCH="$(uname -m)"
  case "$ARCH" in
    x86_64|amd64) PLATFORM=linux/amd64 ;;
    aarch64|arm64) PLATFORM=linux/arm64 ;;
    *)
      echo "Unsupported host arch: $ARCH" >&2
      exit 1
      ;;
  esac
  echo "Building ${IMAGE} for host platform ${PLATFORM}..."
  docker buildx build --platform "$PLATFORM" -t "$IMAGE" --load .
fi

echo "Done: $IMAGE"
