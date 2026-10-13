#!/usr/bin/env bash
#
# Stage everything `cargo tauri build` needs to produce a SELF-CONTAINED, no-Python .app:
#   binaries/amc-<triple>            — the engine (sidecar)
#   binaries/llama-server-<triple>   — the model runtime (sidecar; static llama.cpp build)
#   resources/models/<name>/         — the model pinned in model.lock: amc-model.json + GGUF (+ optional MLX adapter/)
#   resources/amc-ops.gbnf           — the JSON grammar (engine-owned; the manifest pins its hash)
#   resources/licenses/              — third-party notices (incl. the Gemma notice for the weights)
#
# The model comes from Hugging Face (scripts/fetch_model.sh → models/<name>); set
# AMC_MODEL_DIR to stage a different build, AMC_ADAPTER_DIR to include the MLX fallback adapter.
#
# After staging:  cd apps/desktop/src-tauri && cargo tauri build --bundles app
#
# A static, dependency-free llama-server (only macOS system frameworks) is built from a llama.cpp
# checkout with:
#   cmake -B build -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DGGML_METAL=ON \
#         -DLLAMA_CURL=OFF -DLLAMA_OPENSSL=OFF -DGGML_OPENMP=OFF
#   cmake --build build --target llama-server -j
# then point AMC_LLAMA_SERVER at build/bin/llama-server.
set -euo pipefail
cd "$(dirname "$0")/../src-tauri"
REPO="$(git rev-parse --show-toplevel)"
TRIPLE="$(rustc -vV | sed -n 's/host: //p')"

echo "==> engine (amc)"
cargo build --release -p amc-cli --manifest-path "$REPO/Cargo.toml"
AMC="$REPO/target/release/amc"
rm -rf resources/models resources/adapters resources/licenses
mkdir -p binaries resources/models resources/licenses
cp "$AMC" "binaries/amc-$TRIPLE"

echo "==> model runtime (no Python): llama-server"
: "${AMC_LLAMA_SERVER:?set AMC_LLAMA_SERVER to a static llama-server binary (see header for the build)}"
otool -L "$AMC_LLAMA_SERVER" | grep -viE "System/Library|/usr/lib/lib(System|c\+\+|objc)|:$" \
  && { echo "ERROR: llama-server has non-system dylib deps; rebuild static (see header)"; exit 1; } || true
cp "$AMC_LLAMA_SERVER" "binaries/llama-server-$TRIPLE"; chmod +x "binaries/llama-server-$TRIPLE"

echo "==> AMC Model (verified against this engine, weights re-hashed)"
NAME="$(sed -n 's/^name=//p' "$REPO/model.lock" | tr -d '[:space:]')"
MODEL_DIR="${AMC_MODEL_DIR:-$REPO/models/$NAME}"
[ -n "${AMC_MODEL_DIR:-}" ] || "$REPO/scripts/fetch_model.sh"
"$AMC" model verify "$MODEL_DIR"
STAGED="resources/models/$NAME"
mkdir -p "$STAGED"
cp "$MODEL_DIR/amc-model.json" "$STAGED/"
cp "$MODEL_DIR/$(sed -n 's/.*"gguf": *"\([^"]*\)".*/\1/p' "$MODEL_DIR/amc-model.json")" "$STAGED/"
if [ -n "${AMC_ADAPTER_DIR:-}" ] && [ -d "$AMC_ADAPTER_DIR" ]; then
  cp -R "$AMC_ADAPTER_DIR" "$STAGED/adapter"
fi
cp "$REPO/crates/amc-intent/contract/amc-ops.gbnf" resources/

echo "==> license notices"
cp "$REPO/LICENSE" "$REPO/NOTICE" resources/licenses/
cp "$REPO"/apps/desktop/licenses/* resources/licenses/

echo "staged for triple $TRIPLE. Next: cd apps/desktop/src-tauri && cargo tauri build --bundles app"
