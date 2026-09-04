#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/sow-ui/LEGACY_UI.sha256"

[[ -f "$MANIFEST" ]] || {
    echo "ERROR: missing legacy UI freeze manifest: $MANIFEST" >&2
    exit 1
}

CURRENT="$(mktemp "${TMPDIR:-/tmp}/sow-ui-freeze.XXXXXX")"
trap 'rm -f "$CURRENT"' EXIT

(
    cd "$ROOT"
    find sow-ui/src -type f -name '*.rs' -print | sort |
        while IFS= read -r file; do
            sha256sum "$file"
        done
) >"$CURRENT"

if ! diff -u "$MANIFEST" "$CURRENT" >/dev/null; then
    echo "ERROR: sow-ui is frozen; a legacy UI source file was added, removed, or changed." >&2
    echo "Update LEGACY_UI.sha256 only with explicit owner approval." >&2
    exit 1
fi

echo "PASS: legacy egui UI freeze verified"
