#!/usr/bin/env bash
# Remove OpenFront community map binaries from entire git history.
# Run ONCE from repo root before the first public push, after committing HEAD with northamerica only.
#
# Usage: ./scripts/filter_map_history.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FILTER_REPO="$ROOT/scripts/git-filter-repo"
if [[ ! -x "$FILTER_REPO" ]]; then
  if command -v git-filter-repo >/dev/null 2>&1; then
    FILTER_REPO="git-filter-repo"
  else
    echo "Downloading git-filter-repo to scripts/git-filter-repo ..."
    curl -fsSL "https://raw.githubusercontent.com/newren/git-filter-repo/v2.47.0/git-filter-repo" -o "$FILTER_REPO"
    chmod +x "$FILTER_REPO"
  fi
fi

BACKUP="$(mktemp -d)"
trap 'rm -rf "$BACKUP"' EXIT

echo "Backing up shipped maps to $BACKUP"
cp -a assets/maps/northamerica "$BACKUP/"
cp assets/maps/catalog.bin "$BACKUP/"
cp assets/maps/SOURCES.toml "$BACKUP/"

echo "Rewriting history (removing all assets/maps/ blobs)..."
"$FILTER_REPO" --force --invert-paths --path assets/maps/

echo "Restoring northamerica + catalog + SOURCES.toml"
mkdir -p assets/maps
cp -a "$BACKUP/northamerica" assets/maps/
cp "$BACKUP/catalog.bin" assets/maps/
cp "$BACKUP/SOURCES.toml" assets/maps/

git add assets/maps/
git commit -m "$(cat <<'EOF'
Restore shipped map assets after OpenFront map history scrub.

Only northamerica remains under assets/maps/ for public releases.
EOF
)"

echo "Done. Verify with: git log --oneline -- assets/maps/europe/"
echo "Re-add origin remote if filter-repo removed it: git remote add origin <url>"
