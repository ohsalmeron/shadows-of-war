#!/usr/bin/env bash
# Shadows of War - Cloud Deployment
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

VPS_IP="74.208.246.177"
VPS_USER="bizkit"
WEB_DEST_DIR="/var/www/darkrift.ai/html"
BACKEND_DEST_DIR="/home/bizkit/darkrift"

export CARGO_TARGET_DIR="${ROOT}/target"
WASM_IN="${CARGO_TARGET_DIR}/wasm32-unknown-unknown/release/sow_client.wasm"
SIM_WASM_IN="${CARGO_TARGET_DIR}/wasm32-unknown-unknown/release/sow_sim.wasm"

echo "========================================================="
echo "🚀 Starting Production Deployment (Shadows of War -> VPS)"
echo "========================================================="

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
cargo build --release -p sow-client --target wasm32-unknown-unknown
cargo build --release -p sow-sim --target wasm32-unknown-unknown

# Try MUSL, fallback to GNU
if cargo build --release -p sow-server --target x86_64-unknown-linux-musl; then
    SERVER_BIN="target/x86_64-unknown-linux-musl/release/sow-server"
else
    echo "⚠️ Musl build failed, falling back to gnu target..."
    cargo build --release -p sow-server --target x86_64-unknown-linux-gnu
    SERVER_BIN="target/x86_64-unknown-linux-gnu/release/sow-server"
fi

echo "✅ Rust compilation successful."

# 3. WASM Caching Logic
echo "==> Packaging Frontend (wasm-bindgen)..."
mkdir -p dist

WASM_HASH=$(md5sum "${WASM_IN}" | awk '{print $1}')
SIM_WASM_HASH=$(md5sum "${SIM_WASM_IN}" | awk '{print $1}')
LAST_HASH_FILE="${ROOT}/.wasm_hash"

if [[ -f "${LAST_HASH_FILE}" ]] && [[ "$(cat "${LAST_HASH_FILE}")" == "${WASM_HASH}_${SIM_WASM_HASH}" ]]; then
    echo "⚡ WASM hasn't changed. Skipping wasm-bindgen and brotli compression!"
    # Update assets just in case they changed
    rsync -a assets/ dist/assets/
    cp web/sow.svg dist/sow.svg
else
    echo "🔄 WASM changed. Running wasm-bindgen and brotli..."
    rm -rf dist/*
    
    BUILD_TS=$(date +%s)
    JS_FILE="sow_client_${BUILD_TS}.js"
    WASM_FILE="sow_client_${BUILD_TS}_bg.wasm"
    
    ~/.cargo/bin/wasm-bindgen --out-dir dist --target web --out-name "sow_client_${BUILD_TS}" --no-typescript "${WASM_IN}"
    
    mkdir -p dist/assets
    ~/.cargo/bin/wasm-bindgen --out-dir dist/assets --target no-modules --out-name "sow_sim_worker" --no-typescript "${SIM_WASM_IN}"
    
    echo "importScripts('/assets/sow_sim_worker.js?v=${BUILD_TS}'); wasm_bindgen({ module_or_path: '/assets/sow_sim_worker_bg.wasm?v=${BUILD_TS}' });" > dist/assets/sow_sim_worker_boot.js

    rsync -a assets/ dist/assets/ || true
    cp -a web/favicon_io/* dist/ 2>/dev/null || true
    cp web/sow.svg dist/sow.svg
    cp web/sw.js dist/sw.js 2>/dev/null || true
    
    LOADER_TEMPLATE="${ROOT}/web/index.html.template"
    if [[ ! -f "${LOADER_TEMPLATE}" ]]; then
      echo "❌ Missing loader template: ${LOADER_TEMPLATE}"
      exit 1
    fi
    
    sed -e "s/__VERSION__/${CLEAN_VERSION}/g" \
        -e "s/__JS_FILE__/${JS_FILE}/g" \
        -e "s/__WASM_FILE__/${WASM_FILE}/g" \
        -e "s/__BUILD_TS__/${BUILD_TS}/g" \
        "${LOADER_TEMPLATE}" > dist/index.html
        
    if command -v brotli >/dev/null 2>&1; then
      brotli -f -Z dist/${WASM_FILE} &
      BROTLI_WASM_PID=$!
      brotli -f -Z dist/${JS_FILE} &
      BROTLI_JS_PID=$!
      wait $BROTLI_WASM_PID
      wait $BROTLI_JS_PID
      echo "✅ Brotli compression finished."
    fi
    
    echo "${WASM_HASH}_${SIM_WASM_HASH}" > "${LAST_HASH_FILE}"
fi

# 3.5 Update Map MD5 Hashes
echo "==> Updating Map MD5 Hashes..."
python3 - <<'PY'
import json
import hashlib
from pathlib import Path

maps_src = Path("assets/maps")
for manifest_path in maps_src.rglob("manifest.json"):
    map_br = manifest_path.parent / "map.bin.br"
    if map_br.exists():
        with open(map_br, "rb") as f:
            md5_hash = hashlib.md5(f.read()).hexdigest()
        with open(manifest_path, "r") as f:
            manifest = json.load(f)
        if manifest.get("map_md5") != md5_hash:
            manifest["map_md5"] = md5_hash
            with open(manifest_path, "w") as f:
                json.dump(manifest, f, indent=2)
            print(f"Updated MD5 for {manifest_path.parent.name}: {md5_hash}")
PY

# 4. Deployment
echo "==> [PARALLEL] Pushing to VPS..."
rsync -avz --delete --exclude='*.bin' dist/ ${VPS_USER}@${VPS_IP}:${WEB_DEST_DIR}/ &
RSYNC_WEB_PID=$!

ssh ${VPS_USER}@${VPS_IP} "mkdir -p ${BACKEND_DEST_DIR}"
rsync -avz ${SERVER_BIN} ${VPS_USER}@${VPS_IP}:${BACKEND_DEST_DIR}/dark-rift-server &
RSYNC_SERVER_PID=$!

ssh ${VPS_USER}@${VPS_IP} "mkdir -p /home/bizkit/dark-rift-prod/assets/maps"
rsync -avz --exclude='*.bin' assets/maps/ ${VPS_USER}@${VPS_IP}:/home/bizkit/dark-rift-prod/assets/maps/ &
RSYNC_ASSETS_PID=$!

wait $RSYNC_WEB_PID || { echo "❌ Error subiendo Frontend"; exit 1; }
wait $RSYNC_SERVER_PID || { echo "❌ Error subiendo Backend"; exit 1; }
wait $RSYNC_ASSETS_PID || { echo "❌ Error subiendo Assets del servidor"; exit 1; }
echo "✅ VPS sync complete."

# 5. Restart Systemd
echo "==> Restarting Systemd Service on VPS..."
ssh -t ${VPS_USER}@${VPS_IP} "sudo systemctl restart darkrift-server" || { echo "❌ Error reiniciando el servicio"; exit 1; }

echo "========================================================="
echo "🎉 Deployment Completed Successfully (v${CLEAN_VERSION})!"
echo "🕹️  Play live: https://darkrift.ai"
echo "========================================================="
