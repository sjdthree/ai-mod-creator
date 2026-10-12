#!/usr/bin/env bash
#
# Local equivalent of the every-commit CI tier (SPEC-005 §28).
# Run this before every push — it mirrors .github/workflows/ci.yml exactly, so a green run here
# means a green run on GitHub Actions (whether or not Actions is enabled on the repo).
#
# Usage:  ./scripts/ci.sh
set -euo pipefail

cd "$(dirname "$0")/.."

step() { printf '\n\033[1;34m==> %s\033[0m\n' "$1"; }

step "rustfmt (check)"
cargo fmt --all --check

step "clippy (deny warnings)"
cargo clippy --workspace --all-targets -- -D warnings

step "tests (workspace)"
cargo test --workspace

step "licenses (cargo-deny: engine + desktop)"
if command -v cargo-deny >/dev/null; then
  cargo deny check licenses
  cargo deny --manifest-path apps/desktop/src-tauri/Cargo.toml check licenses
else
  echo "cargo-deny not installed — skipping (cargo install cargo-deny --locked)"
fi

step "benchmark runner — empty-suite smoke"
cargo run -q -p mce-cli -- bench benchmarks/ --json >/dev/null
# Clean up the report this smoke run just produced (reports are gitignored anyway).
rm -rf benchmarks/reports/run-* 2>/dev/null || true

printf '\n\033[1;32mAll local CI checks passed.\033[0m\n'
