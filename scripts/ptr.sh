#!/usr/bin/env bash
# Staging play host: build dist/ptr/ (WASM shell only) → ptr.shadowsofwar.io (no static assets in dist).
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sow.sh" ptr "$@"
