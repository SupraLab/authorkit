#!/usr/bin/env bash
# Local coverage report for open-source gates (mirrors .github/workflows/ci.yml).
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
  echo "Installing cargo-llvm-cov…"
  cargo install cargo-llvm-cov --locked
fi
rustup component add llvm-tools-preview >/dev/null

UI_IGNORE='(app|brand|theme|widgets|main|i18n|md_layout|providers|panels/)\.rs|panels/'
LLM_IGNORE='openai_compat\.rs'

echo "=== Domain gates ==="
cargo llvm-cov -p ak-core --fail-under-lines 85 --summary-only
cargo llvm-cov -p ak-export --fail-under-lines 95 --summary-only
cargo llvm-cov -p ak-coherence --fail-under-lines 80 --summary-only
cargo llvm-cov -p ak-llm \
  --ignore-filename-regex "$LLM_IGNORE" \
  --fail-under-lines 85 --summary-only
cargo llvm-cov -p authorkit-desktop --lib \
  --ignore-filename-regex "$UI_IGNORE" \
  --fail-under-lines 74 --summary-only

echo
echo "=== Full summary (excl. UI paint + HTTP client) ==="
cargo llvm-cov --workspace --summary-only \
  --ignore-filename-regex "$LLM_IGNORE|(apps/desktop/src/(app|brand|theme|widgets|main|i18n|md_layout|providers)\.rs|apps/desktop/src/panels/)"

echo
echo "HTML: cargo llvm-cov --workspace --html --open --ignore-filename-regex '$LLM_IGNORE|(apps/desktop/src/(app|brand|theme|widgets|main|i18n|md_layout|providers)\\.rs|apps/desktop/src/panels/)'"
