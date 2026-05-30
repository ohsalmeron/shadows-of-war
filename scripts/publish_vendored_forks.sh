#!/usr/bin/env bash
# Publish vendored path dependencies for AGPL corresponding-source compliance.
# Run from repo root after forking each tree to ohsalmeron/* on GitHub.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

publish_fork() {
  local dir="$1"
  local repo="$2"
  local sha
  sha="$(git -C "$dir" rev-parse HEAD)"
  echo "$dir @ $sha -> github.com/ohsalmeron/$repo"
  if command -v gh >/dev/null 2>&1; then
    if ! gh repo view "ohsalmeron/$repo" >/dev/null 2>&1; then
      gh repo create "ohsalmeron/$repo" --private --source="$dir" --remote=origin --push
    else
      git -C "$dir" push origin HEAD:main 2>/dev/null || git -C "$dir" push origin HEAD
    fi
  else
    echo "  gh not available — push $dir manually and pin $sha in NOTICE"
  fi
}

for pair in "egui:egui-sow" "winit:winit-sow" "blade:blade-sow"; do
  dir="${pair%%:*}"
  repo="${pair##*:}"
  [[ -d "$dir/.git" ]] || { echo "Skip $dir (no .git)"; continue; }
  publish_fork "$dir" "$repo"
done

echo "Update NOTICE and README with pinned SHAs from:"
for d in egui winit blade; do
  [[ -d "$d/.git" ]] && echo "  $d: $(git -C "$d" rev-parse HEAD)"
done
