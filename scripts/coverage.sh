#!/usr/bin/env bash
# Business-logic coverage gate for TokenUsage (domain + provider parsers).
# Excludes GUI bootstrap, OS window bindings, thin Tauri commands, updater network.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/src-tauri"
# shellcheck disable=SC1090
source "${HOME}/.cargo/env" 2>/dev/null || true

export TOKENUSAGE_SKIP_DIRECT_QUOTA=1

# Gate focuses on pure domain + store + quota JSON mapping.
# GUI shell and network fetch modules are excluded.
EXCLUDE=(
  --exclude-files 'src/main.rs'
  --exclude-files 'src/lib.rs'
  --exclude-files 'src/commands.rs'
  --exclude-files 'src/state.rs'
  --exclude-files 'src/application/*'
  --exclude-files 'src/infrastructure/window_ctl.rs'
  --exclude-files 'src/infrastructure/updater.rs'
  --exclude-files 'src/infrastructure/providers/paths.rs'
  --exclude-files 'src/infrastructure/providers/mod.rs'
  --exclude-files 'src/infrastructure/providers/quota/codex_fetch.rs'
  --exclude-files 'src/infrastructure/providers/quota/claude_fetch.rs'
  --exclude-files 'src/infrastructure/providers/quota/grok_fetch.rs'
)

echo "== cargo test (lib + risk_scenarios) =="
cargo test --lib
cargo test --test risk_scenarios

echo "== tarpaulin (fail-under 70 on business logic) =="
if ! command -v cargo-tarpaulin >/dev/null 2>&1; then
  echo "Installing cargo-tarpaulin..."
  cargo install cargo-tarpaulin --locked || cargo install cargo-tarpaulin
fi

cargo tarpaulin --lib \
  --tests \
  --out Stdout \
  --out Html \
  --output-dir target/coverage \
  --timeout 180 \
  --fail-under 75 \
  "${EXCLUDE[@]}"

echo "Coverage HTML: $ROOT/src-tauri/target/coverage/tarpaulin-report.html"
