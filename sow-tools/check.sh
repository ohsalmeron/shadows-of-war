#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
python3 sow-tools/check_file_sizes.py
cargo check --workspace
# Poka-yoke: emoji must flow through the SOW emoji pipeline, never raw egui font.
cargo test -p sow-ui-kit --test emoji_pipeline_guard
