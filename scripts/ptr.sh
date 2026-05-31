#!/usr/bin/env bash
# Shadows of War - PTR Deployment (ptr.shadowsofwar.io on GCP VPS)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
# shellcheck source=deploy-env.sh
source "${ROOT}/scripts/deploy-env.sh"
# shellcheck source=web-assets.sh
source "${ROOT}/scripts/web-assets.sh"

VPS_IP="shadowsofwar.io"
VPS_USER="bizkit"
WEB_DEST_DIR="/var/www/ptr.shadowsofwar.io/html"
BACKEND_DEST_DIR="/home/bizkit/shadowsofwar-ptr"
NGINX_SITE="/etc/nginx/sites-available/ptr.shadowsofwar.io"
SYSTEMD_SERVICE="sow-server-ptr"

export CARGO_TARGET_DIR="${ROOT}/target"
WASM_IN="${CARGO_TARGET_DIR}/wasm32-unknown-unknown/wasm-release/sow_client.wasm"
echo "========================================================="
echo "🚀 Starting PTR Deployment (Shadows of War -> ptr.shadowsofwar.io)"
echo "========================================================="

echo "==> Preflight: local build tools..."
check_local_build_tools

echo "==> Preflight: DNS..."
ensure_ptr_dns || true

echo "==> Preflight: VPS..."
check_vps_ready "${VPS_USER}" "${VPS_IP}" "${NGINX_SITE}" "${SYSTEMD_SERVICE}"

# 1. Bump Version
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
echo "✅ Version bumped to ${CLEAN_VERSION}"

# 2. Build Backend and Frontend
echo "==> Compiling Backend and Frontend..."
RUSTFLAGS="-C target-feature=-bulk-memory" cargo build --profile wasm-release -p sow-client --target wasm32-unknown-unknown
# Try MUSL, fallback to GNU
if cargo build --release -p sow-server --target x86_64-unknown-linux-musl && cargo build --release -p sow-relay --target x86_64-unknown-linux-musl; then
    SERVER_BIN="target/x86_64-unknown-linux-musl/release/sow-server"
    RELAY_BIN="target/x86_64-unknown-linux-musl/release/sow-relay"
else
    echo "⚠️ Musl build failed, falling back to gnu target..."
    cargo build --release -p sow-server --target x86_64-unknown-linux-gnu
    cargo build --release -p sow-relay --target x86_64-unknown-linux-gnu
    SERVER_BIN="target/x86_64-unknown-linux-gnu/release/sow-server"
    RELAY_BIN="target/x86_64-unknown-linux-gnu/release/sow-relay"
fi

echo "✅ Rust compilation successful."

# 3. Package Frontend (wasm-bindgen + brotli)
echo "==> Packaging Frontend (wasm-bindgen)..."
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

LOADER_TEMPLATE="${ROOT}/web/index.html.template"
SW_TEMPLATE="${ROOT}/web/sw.js.template"
if [[ ! -f "${LOADER_TEMPLATE}" ]]; then
  echo "❌ Missing loader template: ${LOADER_TEMPLATE}"
  exit 1
fi
if [[ ! -f "${SW_TEMPLATE}" ]]; then
  echo "❌ Missing service worker template: ${SW_TEMPLATE}"
  exit 1
fi

build_index_html "${LOADER_TEMPLATE}" dist/index.html "${CLEAN_VERSION}" "${JS_FILE}" "${WASM_FILE}" "${BUILD_TS}"

minify_js_shim "dist/${JS_FILE}"

optimize_wasm_bundle "dist/${WASM_FILE}"

if command -v brotli >/dev/null 2>&1; then
  brotli -f -Z dist/${WASM_FILE} &
  BROTLI_WASM_PID=$!
  brotli -f -Z dist/${JS_FILE} &
  BROTLI_JS_PID=$!
  wait $BROTLI_WASM_PID
  wait $BROTLI_JS_PID
  echo "✅ Brotli compression finished."
fi

sed -e "s/__VERSION__/${CLEAN_VERSION}/g" \
    -e "s/__JS_FILE__/${JS_FILE}/g" \
    -e "s/__WASM_FILE__/${WASM_FILE}/g" \
    -e "s/__BUILD_TS__/${BUILD_TS}/g" \
    "${SW_TEMPLATE}" > dist/sw.js

WASM_KB=$(( $(stat -c%s "dist/${WASM_FILE}") / 1024 ))
JS_KB=$(( $(stat -c%s "dist/${JS_FILE}") / 1024 ))
echo "Bundle sizes: ${WASM_FILE}=${WASM_KB} KB, ${JS_FILE}=${JS_KB} KB"

# 4. Deployment
echo "==> [PARALLEL] Pushing to VPS..."
rsync -avz --delete --exclude='*.bin' dist/ ${VPS_USER}@${VPS_IP}:${WEB_DEST_DIR}/ &
RSYNC_WEB_PID=$!

ssh ${VPS_USER}@${VPS_IP} "mkdir -p ${BACKEND_DEST_DIR}"
rsync -avz ${SERVER_BIN} ${VPS_USER}@${VPS_IP}:${BACKEND_DEST_DIR}/sow-server &
RSYNC_SERVER_PID=$!

rsync -avz ${RELAY_BIN} ${VPS_USER}@${VPS_IP}:${BACKEND_DEST_DIR}/sow-relay &
RSYNC_RELAY_PID=$!

ssh ${VPS_USER}@${VPS_IP} "mkdir -p ${BACKEND_DEST_DIR}/assets/maps"
rsync -avz --exclude='map.bin' --exclude='mini_map.bin' --exclude='manifest.json' --exclude='maps.json' assets/maps/ ${VPS_USER}@${VPS_IP}:${BACKEND_DEST_DIR}/assets/maps/ &
RSYNC_ASSETS_PID=$!

wait $RSYNC_WEB_PID || { echo "❌ Error subiendo Frontend"; exit 1; }
wait $RSYNC_SERVER_PID || { echo "❌ Error subiendo Backend (Orchestrator)"; exit 1; }
wait $RSYNC_RELAY_PID || { echo "❌ Error subiendo Backend (Relay)"; exit 1; }
wait $RSYNC_ASSETS_PID || { echo "❌ Error subiendo Assets del servidor"; exit 1; }
echo "✅ VPS sync complete."

# 4.5 Nginx — brotli_static + Cache-Control (idempotent)
sync_vps_nginx "${VPS_USER}" "${VPS_IP}" "${NGINX_SITE}" "${ROOT}/nginx_config_ptr.conf"

# 5. Restart PTR backend (production sow-server untouched)
echo "==> Ensuring sow-server-ptr systemd unit and restarting..."
ssh ${VPS_USER}@${VPS_IP} "cat << 'SYSTEMD' | sudo tee /etc/systemd/system/sow-server-ptr.service > /dev/null
[Unit]
Description=Shadows of War Server (PTR)
After=network.target

[Service]
KillMode=process
Type=simple
User=bizkit
WorkingDirectory=${BACKEND_DEST_DIR}
ExecStart=${BACKEND_DEST_DIR}/sow-server
Restart=always
RestartSec=3
Environment=\"RUST_LOG=info\"
Environment=\"SOW_WS_LISTEN=0.0.0.0:25575\"
Environment=\"SOW_MAPS_HTTP_LISTEN=0.0.0.0:25576\"

[Install]
WantedBy=multi-user.target
SYSTEMD"

ssh -t ${VPS_USER}@${VPS_IP} "which redis-server >/dev/null 2>&1 || sudo DEBIAN_FRONTEND=noninteractive apt-get install -yq redis-server; sudo systemctl enable --now sow-redis 2>/dev/null || sudo systemctl enable --now redis-server; sudo systemctl daemon-reload; sudo systemctl enable --now ${SYSTEMD_SERVICE}; sudo systemctl restart ${SYSTEMD_SERVICE}" || { echo "❌ Error reiniciando el servicio PTR"; exit 1; }

if ptr_dns_resolves; then
  echo "==> Ensuring TLS for ptr.shadowsofwar.io..."
  ssh ${VPS_USER}@${VPS_IP} "sudo test -f /etc/letsencrypt/live/ptr.shadowsofwar.io/fullchain.pem \
    || sudo certbot --nginx -d ptr.shadowsofwar.io --non-interactive --agree-tos --register-unsafely-without-email --redirect"
fi

if ptr_dns_resolves; then
  verify_prod_headers "https://ptr.shadowsofwar.io" \
    || echo "⚠️  Header verification failed"
else
  echo "⚠️  Skipping live header check (DNS not resolving)"
fi

echo "========================================================="
echo "🎉 PTR Deployment Completed Successfully (v${CLEAN_VERSION})!"
echo "🕹️  Play live: https://ptr.shadowsofwar.io"
echo "========================================================="
