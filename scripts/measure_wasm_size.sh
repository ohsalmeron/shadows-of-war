#!/usr/bin/env bash
# Report initial WASM download size for CrazyGames (Basic Launch ≤ 50MB).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

WASM_DIR="$ROOT/target/wasm32-unknown-unknown/release"
if [[ ! -d "$WASM_DIR" ]]; then
  echo "Build release WASM first: cargo build -p sow-client --target wasm32-unknown-unknown --release"
  exit 1
fi

total=0
for f in "$WASM_DIR"/*.wasm "$ROOT/web"/*.js 2>/dev/null; do
  [[ -f "$f" ]] || continue
  size=$(stat -c%s "$f" 2>/dev/null || stat -f%z "$f")
  echo "$(basename "$f"): $(( size / 1024 )) KB"
  total=$((total + size))
done

echo "Total tracked: $(( total / 1024 / 1024 )) MB (CrazyGames Basic limit: 50 MB)"
