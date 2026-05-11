#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

if ! command -v cargo >/dev/null 2>&1; then
  cat >&2 <<'EOF'
Missing dependency: cargo

Install Rust first, for example:
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
EOF
  exit 1
fi

cargo build --release
exec "$SCRIPT_DIR/target/release/lumactl" install "$@"
