#!/usr/bin/env bash
#
# Fetch the AMC model pinned in model.lock from Hugging Face into models/<name>/ (gitignored) and check
# it against this engine.
#
# A model directory holds `amc-model.json` (the manifest: contract/grammar/weights sha256 pins) plus the
# GGUF it names. Integrity chain: model.lock pins the manifest's sha256 → the manifest pins the GGUF's
# sha256 and the contract/grammar this engine embeds → `amc model verify` checks all of it.
#
# Usage:  ./scripts/fetch_model.sh [dest-dir]       (default models/<name>)
# Auth:   only needed while the Hugging Face repo is private — uses $HF_TOKEN, else the token saved by
#         `hf auth login` (~/.cache/huggingface/token). Public repos need nothing.
set -euo pipefail
cd "$(dirname "$0")/.."

lock() { sed -n "s/^$1=//p" model.lock | tr -d '[:space:]'; }
NAME="$(lock name)"; REPO="$(lock repo)"; REV="$(lock revision)"; PIN="$(lock manifest_sha256)"; REMOTE_MANIFEST="$(lock manifest_file)"
DEST="${1:-models/$NAME}"
BASE="https://huggingface.co/$REPO/resolve/$REV"

TOKEN="${HF_TOKEN:-}"
[ -z "$TOKEN" ] && [ -f "$HOME/.cache/huggingface/token" ] && TOKEN="$(cat "$HOME/.cache/huggingface/token")"
get() {  # get <file> <out>
  if [ -n "$TOKEN" ]; then
    curl -fL --progress-bar --retry 3 -H "Authorization: Bearer $TOKEN" -o "$2" "$BASE/$1"
  else
    curl -fL --progress-bar --retry 3 -o "$2" "$BASE/$1"
  fi
}
sha() { shasum -a 256 "$1" | cut -d' ' -f1; }

mkdir -p "$DEST"
echo "==> $NAME from huggingface.co/$REPO@$REV"
get "$REMOTE_MANIFEST" "$DEST/amc-model.json"
got="$(sha "$DEST/amc-model.json")"
if [ "$got" != "$PIN" ]; then
  echo "ERROR: amc-model.json sha256 $got does not match model.lock ($PIN)"; exit 1
fi
GGUF="$(sed -n 's/.*"gguf": *"\([^"]*\)".*/\1/p' "$DEST/amc-model.json")"
WANT="$(sed -n 's/.*"gguf_sha256": *"\([^"]*\)".*/\1/p' "$DEST/amc-model.json")"
if [ -f "$DEST/$GGUF" ] && [ "$(sha "$DEST/$GGUF")" = "$WANT" ]; then
  echo "    $GGUF already present"
else
  get "$GGUF" "$DEST/$GGUF"
fi

cargo build -q -p amc-cli
echo "==> verifying against this engine"
target/debug/amc model verify "$DEST"
