#!/usr/bin/env bash
# Shadows of War - Cloud Deployment & portal packaging
#   ./scripts/cloud.sh              → deploy to shadowsofwar.io VPS
#   ./scripts/cloud.sh package      → dist/ + shadows-of-war-crazygames.zip (CrazyGames)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
# shellcheck source=deploy-env.sh
source "${ROOT}/scripts/deploy-env.sh"
# shellcheck source=web-assets.sh
source "${ROOT}/scripts/web-assets.sh"

MODE="${1:-deploy}"
PORTAL="${2:-crazygames}"
PACKAGE=0
if [[ "${MODE}" == "package" ]]; then
  PACKAGE=1
fi

VPS_IP="35.239.160.167"
VPS_USER="bizkit"
WEB_DEST_DIR="/var/www/shadowsofwar.io/html"
BACKEND_DEST_DIR="/home/bizkit/shadowsofwar"
NGINX_SITE="/etc/nginx/sites-available/shadowsofwar.io"

export CARGO_TARGET_DIR="${ROOT}/target"
WASM_IN="${CARGO_TARGET_DIR}/wasm32-unknown-unknown/wasm-release/sow_client.wasm"

print_agpl_release_steps() {
  local tag="v${CLEAN_VERSION}"
  echo ""
  echo "AGPL corresponding source (required before monetizing shadowsofwar.io):"
  echo "  1. Make GitHub repo PUBLIC: github.com/ohsalmeron/shadows-of-war → Settings → Change visibility"
  echo "  2. Commit, push, then tag this deploy:"
  echo "       git tag -a ${tag} -m \"Release ${tag}\" && git push origin ${tag}"
  echo "  3. Source URL for users: https://github.com/ohsalmeron/shadows-of-war/tree/${tag}"
  if git rev-parse "${tag}" >/dev/null 2>&1; then
    echo "  (local tag ${tag} already exists)"
  elif git diff --quiet && git diff --cached --quiet 2>/dev/null; then
    git tag -a "${tag}" -m "Release ${tag} (Shadows of War)"
    echo "  Created local tag ${tag} — run: git push origin ${tag}"
  else
    echo "  (uncommitted changes — create tag after you commit and push)"
  fi
}

if [[ "${PACKAGE}" -eq 1 ]]; then
  echo "========================================================="
  echo "Packaging for portal: ${PORTAL} (no VPS deploy)"
  echo "========================================================="
else
  echo "========================================================="
  echo "Starting Production Deployment (Shadows of War -> VPS)"
  echo "========================================================="
fi

echo "==> Preflight: local build tools..."
check_local_build_tools

if [[ "${PACKAGE}" -eq 0 ]]; then
  echo "==> Preflight: VPS..."
  check_vps_ready "${VPS_USER}" "${VPS_IP}" "${NGINX_SITE}"
fi

VERSION_FILE="${ROOT}/.version"
if [[ ! -f "${VERSION_FILE}" ]]; then
  echo "0.1.0" > "${VERSION_FILE}"
fi
CURRENT_VERSION=$(cat "${VERSION_FILE}")

if [[ "${CURRENT_VERSION}" == *.* ]]; then
  PATCH=$(echo "${CURRENT_VERSION}" | rev | cut -d. -f1 | rev)
else
  PATCH="${CURRENT_VERSION}"
fi

NEW_PATCH=$((PATCH + 1))
CLEAN_VERSION="0.1.${NEW_PATCH}"
echo "${CLEAN_VERSION}" > "${VERSION_FILE}"
echo "Version bumped to ${CLEAN_VERSION}"

echo "==> Compiling WASM client..."
RUSTFLAGS="-C target-feature=-bulk-memory" cargo build --profile wasm-release -p sow-client --target wasm32-unknown-unknown

SERVER_BIN=""
RELAY_BIN=""
if [[ "${PACKAGE}" -eq 0 ]]; then
  echo "==> Compiling backend (sow-server, sow-relay)..."
  if cargo build --release -p sow-server --target x86_64-unknown-linux-musl \
    && cargo build --release -p sow-relay --target x86_64-unknown-linux-musl; then
    SERVER_BIN="target/x86_64-unknown-linux-musl/release/sow-server"
    RELAY_BIN="target/x86_64-unknown-linux-musl/release/sow-relay"
  else
    echo "Musl build failed, falling back to gnu target..."
    cargo build --release -p sow-server --target x86_64-unknown-linux-gnu
    cargo build --release -p sow-relay --target x86_64-unknown-linux-gnu
    SERVER_BIN="target/x86_64-unknown-linux-gnu/release/sow-server"
    RELAY_BIN="target/x86_64-unknown-linux-gnu/release/sow-relay"
  fi
fi

echo "Rust compilation successful."

echo "==> Packaging frontend (wasm-bindgen)..."
mkdir -p dist
rm -rf dist/*

BUILD_TS=$(date +%s)
JS_FILE="sow_client_${BUILD_TS}.js"
WASM_FILE="sow_client_${BUILD_TS}_bg.wasm"

~/.cargo/bin/wasm-bindgen --out-dir dist --target web --out-name "sow_client_${BUILD_TS}" --no-typescript "${WASM_IN}"

rsync -a assets/ dist/assets/ || true
copy_web_loader_assets
cp -a web/favicon_io/* dist/ 2>/dev/null || true
cp web/sow.svg dist/sow.svg
mkdir -p dist/sdk
cp -a web/sdk/. dist/sdk/
cp web/privacy.html dist/privacy.html

LOADER_TEMPLATE="${ROOT}/web/index.html.template"
SW_TEMPLATE="${ROOT}/web/sw.js.template"
if [[ ! -f "${LOADER_TEMPLATE}" ]]; then
  echo "Missing loader template: ${LOADER_TEMPLATE}"
  exit 1
fi
if [[ ! -f "${SW_TEMPLATE}" ]]; then
  echo "Missing service worker template: ${SW_TEMPLATE}"
  exit 1
fi

build_index_html "${LOADER_TEMPLATE}" dist/index.html "${CLEAN_VERSION}" "${JS_FILE}" "${WASM_FILE}" "${BUILD_TS}"
if [[ "${PACKAGE}" -eq 1 && "${PORTAL}" == "crazygames" ]]; then
  inject_crazygames_portal dist/index.html
fi
minify_js_shim "dist/${JS_FILE}"

optimize_wasm_bundle "dist/${WASM_FILE}"

if command -v brotli >/dev/null 2>&1; then
  brotli -f -Z "dist/${WASM_FILE}" &
  BROTLI_WASM_PID=$!
  brotli -f -Z "dist/${JS_FILE}" &
  BROTLI_JS_PID=$!
  wait $BROTLI_WASM_PID
  wait $BROTLI_JS_PID
  echo "Brotli compression finished."
fi

sed -e "s/__VERSION__/${CLEAN_VERSION}/g" \
  -e "s/__JS_FILE__/${JS_FILE}/g" \
  -e "s/__WASM_FILE__/${WASM_FILE}/g" \
  -e "s/__BUILD_TS__/${BUILD_TS}/g" \
  "${SW_TEMPLATE}" > dist/sw.js

WASM_KB=$(( $(stat -c%s "dist/${WASM_FILE}") / 1024 ))
JS_KB=$(( $(stat -c%s "dist/${JS_FILE}") / 1024 ))
echo "Bundle sizes: ${WASM_FILE}=${WASM_KB} KB, ${JS_FILE}=${JS_KB} KB (CrazyGames limit ~51200 KB initial)"

if [[ "${PACKAGE}" -eq 1 ]]; then
  ZIP_NAME="shadows-of-war-${PORTAL}.zip"
  rm -f "${ROOT}/${ZIP_NAME}"
  (cd dist && zip -r -q "../${ZIP_NAME}" .)
  echo "========================================================="
  echo "Portal package ready: ${ROOT}/${ZIP_NAME}"
  echo "Upload this zip to the ${PORTAL} Developer Portal."
  echo "Test locally: cd dist && python -m http.server 8080"
  print_agpl_release_steps
  echo "========================================================="
  exit 0
fi

echo "==> [PARALLEL] Pushing to VPS..."
rsync -avz --delete --exclude='*.bin' dist/ "${VPS_USER}@${VPS_IP}:${WEB_DEST_DIR}/" &
RSYNC_WEB_PID=$!

ssh "${VPS_USER}@${VPS_IP}" "mkdir -p ${BACKEND_DEST_DIR}"
rsync -avz "${SERVER_BIN}" "${VPS_USER}@${VPS_IP}:${BACKEND_DEST_DIR}/sow-server" &
RSYNC_SERVER_PID=$!

rsync -avz "${RELAY_BIN}" "${VPS_USER}@${VPS_IP}:${BACKEND_DEST_DIR}/sow-relay" &
RSYNC_RELAY_PID=$!

ssh "${VPS_USER}@${VPS_IP}" "mkdir -p /home/bizkit/shadowsofwar/assets/maps"
rsync -avz --exclude='map.bin' --exclude='mini_map.bin' --exclude='manifest.json' --exclude='maps.json' \
  assets/maps/ "${VPS_USER}@${VPS_IP}:/home/bizkit/shadowsofwar/assets/maps/" &
RSYNC_ASSETS_PID=$!

wait $RSYNC_WEB_PID || { echo "Error uploading frontend"; exit 1; }
wait $RSYNC_SERVER_PID || { echo "Error uploading sow-server"; exit 1; }
wait $RSYNC_RELAY_PID || { echo "Error uploading sow-relay"; exit 1; }
wait $RSYNC_ASSETS_PID || { echo "Error uploading map assets"; exit 1; }
echo "VPS sync complete."

sync_vps_nginx "${VPS_USER}" "${VPS_IP}" "${NGINX_SITE}" "${ROOT}/nginx_config.conf"

echo "==> Ensuring Redis is running and restarting Orchestrator..."
ssh -t "${VPS_USER}@${VPS_IP}" "which redis-server >/dev/null 2>&1 || sudo DEBIAN_FRONTEND=noninteractive apt-get install -yq redis-server; sudo systemctl enable --now sow-redis; sudo systemctl restart sow-server" \
  || { echo "Error restarting sow-server"; exit 1; }

verify_prod_headers "https://shadowsofwar.io" || echo "Header verification failed — site may still be propagating"

echo "========================================================="
echo "Deployment completed (v${CLEAN_VERSION})"
echo "Play live: https://shadowsofwar.io"
echo "Portal zip: ./scripts/cloud.sh package"
print_agpl_release_steps
echo "========================================================="
