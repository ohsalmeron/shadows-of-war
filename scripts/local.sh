#!/usr/bin/env bash
# Browser WASM at http://127.0.0.1:8080 — builds dist/play/ (shell only) + symlink assets/static → repo.
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sow.sh" local "$@"
