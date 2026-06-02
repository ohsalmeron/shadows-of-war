#!/usr/bin/env bash
# CrazyGames: always rebuilds dist/crazygames/ (.br WASM/JS + SDK + assets/static symlink).
# Optional: --sync-cdn (push streamed leaders to prod CDN before build).
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sow.sh" crazygames "$@"
