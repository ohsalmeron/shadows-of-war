#!/usr/bin/env bash
# Poki portal: build dist/poki/ (.br WASM/JS). Upload that folder; art/maps on CDN.
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sow.sh" poki "$@"
