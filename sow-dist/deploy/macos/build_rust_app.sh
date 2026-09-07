#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target}"
APP_PATH="${TARGET_BUILD_DIR:?}/ShadowsOfWar.app"
ICONSET_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/ShadowsOfWar.icon.XXXXXX")"
trap 'rm -rf "$ICONSET_ROOT"' EXIT

export PATH="$PATH:$HOME/.cargo/bin:/opt/homebrew/bin"

cargo build --locked --release --bin client --manifest-path "$REPO_ROOT/Cargo.toml"

mkdir -p "$APP_PATH/Contents/MacOS" "$APP_PATH/Contents/Resources"
ditto "$TARGET_DIR/release/client" "$APP_PATH/Contents/MacOS/ShadowsOfWar"

ICONSET="$ICONSET_ROOT/ShadowsOfWar.iconset"
mkdir -p "$ICONSET"
for spec in \
    "16 16 icon_16x16.png" \
    "32 32 icon_16x16@2x.png" \
    "32 32 icon_32x32.png" \
    "64 64 icon_32x32@2x.png" \
    "128 128 icon_128x128.png" \
    "256 256 icon_128x128@2x.png" \
    "256 256 icon_256x256.png" \
    "512 512 icon_256x256@2x.png" \
    "512 512 icon_512x512.png" \
    "1024 1024 icon_512x512@2x.png"; do
    read -r width height filename <<<"$spec"
    sips -z "$height" "$width" "$REPO_ROOT/assets/shell/brand/app-icon.png" \
        --out "$ICONSET/$filename" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP_PATH/Contents/Resources/ShadowsOfWar.icns"
