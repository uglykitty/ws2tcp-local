#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

podman build \
  -t ws2tcp-local \
  -f "$repo_dir/Dockerfile" \
  "$repo_dir/.."
