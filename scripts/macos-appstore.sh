#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="ShadowsOfWar"
APP="${ROOT}/dist/macos/${APP_NAME}.app"
TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT}/target}"
INFO_PLIST="${ROOT}/sow-dist/deploy/macos/Info.plist"
ENTITLEMENTS="${ROOT}/sow-dist/deploy/macos/${APP_NAME}.entitlements"

find_signing_identity() {
  security find-identity -v -p codesigning 2>/dev/null \
    | awk '/"Apple Development:/{print $2; exit}' \
    | head -n 1
}

build_app() {
  local signing_identity="${SOW_MACOS_SIGNING_IDENTITY:-$(find_signing_identity)}"
  if [[ -z "${signing_identity}" ]]; then
    echo "No Apple Development signing identity is available." >&2
    exit 1
  fi

  cargo build --locked --release --bin client

  rm -rf "${APP}"
  mkdir -p "${APP}/Contents/MacOS" "${APP}/Contents/Resources"
  cp "${TARGET_DIR}/release/client" "${APP}/Contents/MacOS/${APP_NAME}"
  cp "${INFO_PLIST}" "${APP}/Contents/Info.plist"

  local iconset_root iconset
  iconset_root="$(mktemp -d "${TMPDIR:-/tmp}/ShadowsOfWar.icon.XXXXXX")"
  iconset="${iconset_root}/ShadowsOfWar.iconset"
  mkdir -p "${iconset}"
  trap 'rm -rf "${iconset_root}"' RETURN
  sips -z 16 16 "${ROOT}/assets/shell/brand/app-icon.png" --out "${iconset}/icon_16x16.png" >/dev/null
  sips -z 32 32 "${ROOT}/assets/shell/brand/app-icon.png" --out "${iconset}/icon_16x16@2x.png" >/dev/null
  sips -z 32 32 "${ROOT}/assets/shell/brand/app-icon.png" --out "${iconset}/icon_32x32.png" >/dev/null
  sips -z 64 64 "${ROOT}/assets/shell/brand/app-icon.png" --out "${iconset}/icon_32x32@2x.png" >/dev/null
  sips -z 128 128 "${ROOT}/assets/shell/brand/app-icon.png" --out "${iconset}/icon_128x128.png" >/dev/null
  sips -z 256 256 "${ROOT}/assets/shell/brand/app-icon.png" --out "${iconset}/icon_128x128@2x.png" >/dev/null
  sips -z 256 256 "${ROOT}/assets/shell/brand/app-icon.png" --out "${iconset}/icon_256x256.png" >/dev/null
  sips -z 512 512 "${ROOT}/assets/shell/brand/app-icon.png" --out "${iconset}/icon_256x256@2x.png" >/dev/null
  sips -z 512 512 "${ROOT}/assets/shell/brand/app-icon.png" --out "${iconset}/icon_512x512.png" >/dev/null
  sips -z 1024 1024 "${ROOT}/assets/shell/brand/app-icon.png" --out "${iconset}/icon_512x512@2x.png" >/dev/null
  iconutil -c icns "${iconset}" -o "${APP}/Contents/Resources/ShadowsOfWar.icns"

  codesign --force --sign "${signing_identity}" --entitlements "${ENTITLEMENTS}" \
    --timestamp=none "${APP}/Contents/MacOS/${APP_NAME}"
  codesign --force --sign "${signing_identity}" --entitlements "${ENTITLEMENTS}" \
    --timestamp=none "${APP}"
  codesign --verify --deep --strict --verbose=2 "${APP}"

  echo "Built and signed: ${APP}"
  echo "Signing identity: ${signing_identity}"
}

case "${1:-build}" in
  build)
    build_app
    ;;
  run)
    build_app
    open -n "${APP}"
    ;;
  verify)
    codesign --verify --deep --strict --verbose=2 "${APP}"
    codesign -d --entitlements :- "${APP}" 2>/dev/null
    ;;
  *)
    echo "Usage: $0 {build|run|verify}" >&2
    exit 2
    ;;
esac
