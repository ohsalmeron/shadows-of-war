#!/usr/bin/env bash
# Native debug: sow-server + 2× sow-client (Rust binaries). No web dist/ folder.
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sow.sh" native "$@"
