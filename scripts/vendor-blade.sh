#!/usr/bin/env bash
# Vendor the pinned blade graphics fork (ohsalmeron/blade).
# Idempotent: restores blade/ at the exact rev our Cargo.tomls pin when it is
# missing (fresh clone — blade/ is gitignored) or drifted. Safe to run always.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

# Single source of truth: the rev pinned in sow-render/Cargo.toml
# (sow-client and sow-map pin the same rev).
REV="$(sed -nE 's|.*ohsalmeron/blade", rev = "([0-9a-f]{40})".*|\1|p' sow-render/Cargo.toml | head -n 1)"
if [ -z "${REV}" ]; then
  echo "vendor-blade: cannot parse pinned blade rev from sow-render/Cargo.toml" >&2
  exit 1
fi
URL="https://github.com/ohsalmeron/blade"

if [ ! -f blade/blade-egui/Cargo.toml ]; then
  echo "vendor-blade: cloning ${URL} @ ${REV}"
  rm -rf blade
  git clone --no-checkout "${URL}" blade
  git -C blade checkout --quiet "${REV}"
fi
CURRENT="$(git -C blade rev-parse HEAD 2>/dev/null || echo MISSING)"
if [ "${CURRENT}" != "${REV}" ]; then
  echo "vendor-blade: checkout ${REV} (have ${CURRENT})"
  git -C blade fetch --quiet origin "${REV}" || git -C blade fetch --quiet
  git -C blade checkout --quiet "${REV}"
fi

EGUI_VER="$(sed -nE 's/^version = "([^"]+)".*/\1/p' blade/blade-egui/Cargo.toml | head -n 1)"
GRAPHICS_VER="$(sed -nE 's/^version = "([^"]+)".*/\1/p' blade/blade-graphics/Cargo.toml | head -n 1)"
if [ "${EGUI_VER}" != "0.8.1" ] || [ "${GRAPHICS_VER}" != "0.8.4" ]; then
  echo "vendor-blade: version mismatch at ${REV} (egui=${EGUI_VER} graphics=${GRAPHICS_VER})" >&2
  exit 1
fi
echo "vendor-blade: OK ${REV} (egui ${EGUI_VER}, graphics ${GRAPHICS_VER})"
