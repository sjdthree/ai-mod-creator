#!/usr/bin/env bash
#
# Build and enter the MCE dev container (JDK 21 + Rust + Gradle + headless-client deps).
#
# Everything the container needs lives ONLY in the image and in named Docker volumes
# (mce-cargo / mce-gradle / mce-target). The host's Java, Gradle, Rust, and other repos are never
# touched. The repo is bind-mounted read-write at /workspace.
#
# Usage:
#   ./scripts/dev-container.sh build     # build the image
#   ./scripts/dev-container.sh shell     # enter an interactive shell (builds image if missing)
#   ./scripts/dev-container.sh gate      # run the Rust CI gate inside the container
#   ./scripts/dev-container.sh clean     # remove the image and cache volumes
set -euo pipefail
cd "$(dirname "$0")/.."

IMAGE=mce-dev
VOLUMES=(-v mce-cargo:/home/dev/.cargo/registry -v mce-gradle:/home/dev/.gradle -v mce-target:/home/dev/target)

build() { docker build --load -t "$IMAGE" -f .devcontainer/Dockerfile .devcontainer; }
ensure() { docker image inspect "$IMAGE" >/dev/null 2>&1 || build; }
run() { ensure; docker run --rm -it -v "$PWD:/workspace" "${VOLUMES[@]}" -w /workspace "$IMAGE" "$@"; }

case "${1:-shell}" in
  build) build ;;
  shell | "") run bash ;;
  gate) run bash -lc './scripts/ci.sh' ;;
  clean)
    docker image rm -f "$IMAGE" 2>/dev/null || true
    docker volume rm -f mce-cargo mce-gradle mce-target 2>/dev/null || true
    echo "removed image + cache volumes (host untouched)" ;;
  *) echo "usage: $0 [build|shell|gate|clean]"; exit 2 ;;
esac
