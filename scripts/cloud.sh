#!/usr/bin/env bash
# Full production: cloud-game (dist/play → VPS) + cloud-site (marketing HTML).
# Incremental: skips rebuild/rsync when inputs unchanged; use --force to redeploy.
exec "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sow.sh" cloud "$@"
