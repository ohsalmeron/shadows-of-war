#!/usr/bin/env bash
# Production play host: build dist/play/ (WASM shell only) → play.shadowsofwar.io + CDN assets + backend.
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sow.sh" cloud-game "$@"
