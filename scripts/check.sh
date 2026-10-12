#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
# Opt in explicitly: the starter's Rust-only CI does not install the pinned Pi CLI.
# This runs actual RPC children with a fake provider and disposable profiles only.
node --test scripts/pi-runtime/tests/*.test.mjs
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked
if [[ "${PI_GUI_CHECK_COMPAT:-0}" == "1" ]]; then
  node --test scripts/pi-compat/tests/*.test.mjs
  node --test scripts/pi-runtime/tests/integration/*.test.mjs
fi
