#!/usr/bin/env bash
# Quick standalone relay test against live VPS
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
cargo run --bin test-relay -- --url "${1:-wss://shadowsofwar.io/ws/}"
