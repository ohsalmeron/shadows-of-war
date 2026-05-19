#!/usr/bin/env bash
# Shadows of War - PTR (Darkrift.ai) Deployment
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

VPS_IP="darkrift.ai"
VPS_USER="bizkit"
WEB_DEST_DIR="/var/www/darkrift.ai/html"
BACKEND_DEST_DIR="/home/bizkit/shadowsofwar"

export CARGO_TARGET_DIR="${ROOT}/target"
WASM_IN="${CARGO_TARGET_DIR}/wasm32-unknown-unknown/release/sow_client.wasm"
echo "========================================================="
echo "🚀 Starting PTR Deployment (Shadows of War -> darkrift.ai)"
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
RUSTFLAGS="-C target-feature=-bulk-memory" cargo build --release -p sow-client --target wasm32-unknown-unknown
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

# 3. WASM Caching Logic
echo "==> Packaging Frontend (wasm-bindgen)..."
mkdir -p dist

WASM_HASH=$(md5sum "${WASM_IN}" | awk '{print $1}')
LAST_HASH_FILE="${ROOT}/.wasm_hash"

if [[ -f "${LAST_HASH_FILE}" ]] && [[ "$(cat "${LAST_HASH_FILE}")" == "${WASM_HASH}" ]]; then
    echo "⚡ WASM hasn't changed. Skipping wasm-bindgen and brotli compression!"
    rsync -a assets/ dist/assets/
    cp web/sow.svg dist/sow.svg
else
    echo "🔄 WASM changed. Running wasm-bindgen and brotli..."
    rm -rf dist/*
    
    BUILD_TS=$(date +%s)
    JS_FILE="sow_client_${BUILD_TS}.js"
    WASM_FILE="sow_client_${BUILD_TS}_bg.wasm"
    
    ~/.cargo/bin/wasm-bindgen --out-dir dist --target web --out-name "sow_client_${BUILD_TS}" --no-typescript "${WASM_IN}"

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
    
    echo "${WASM_HASH}" > "${LAST_HASH_FILE}"
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
rsync -avz ${SERVER_BIN} ${VPS_USER}@${VPS_IP}:${BACKEND_DEST_DIR}/sow-server &
RSYNC_SERVER_PID=$!

rsync -avz ${RELAY_BIN} ${VPS_USER}@${VPS_IP}:${BACKEND_DEST_DIR}/sow-relay &
RSYNC_RELAY_PID=$!

ssh ${VPS_USER}@${VPS_IP} "mkdir -p /home/bizkit/shadowsofwar/assets/maps"
rsync -avz --exclude='*.bin' assets/maps/ ${VPS_USER}@${VPS_IP}:/home/bizkit/shadowsofwar/assets/maps/ &
RSYNC_ASSETS_PID=$!

wait $RSYNC_WEB_PID || { echo "❌ Error subiendo Frontend"; exit 1; }
wait $RSYNC_SERVER_PID || { echo "❌ Error subiendo Backend (Orchestrator)"; exit 1; }
wait $RSYNC_RELAY_PID || { echo "❌ Error subiendo Backend (Relay)"; exit 1; }
wait $RSYNC_ASSETS_PID || { echo "❌ Error subiendo Assets del servidor"; exit 1; }
echo "✅ VPS sync complete."

# 5. Restart Services
echo "==> Setting up systemd for sow-server on PTR if not exists..."
ssh ${VPS_USER}@${VPS_IP} "cat << 'SYSTEMD' | sudo tee /etc/systemd/system/sow-server.service > /dev/null
[Unit]
Description=Shadows of War Server
After=network.target

[Service]
KillMode=process
Type=simple
User=bizkit
WorkingDirectory=/home/bizkit/shadowsofwar
ExecStart=/home/bizkit/shadowsofwar/sow-server
Restart=always
RestartSec=3
Environment=\"RUST_LOG=info\"
Environment=\"SOW_WS_LISTEN=0.0.0.0:25565\"
Environment=\"SOW_MAPS_HTTP_LISTEN=0.0.0.0:25566\"

[Install]
WantedBy=multi-user.target
SYSTEMD"

echo "==> Disabling old darkrift-server and enabling new sow-server..."
ssh -t ${VPS_USER}@${VPS_IP} "sudo systemctl stop darkrift-server.service || true; sudo systemctl disable darkrift-server.service || true; which redis-server >/dev/null 2>&1 || sudo DEBIAN_FRONTEND=noninteractive apt-get install -yq redis-server; sudo systemctl daemon-reload; sudo systemctl enable --now sow-server.service; sudo systemctl restart sow-server.service" || { echo "❌ Error reiniciando el servicio"; exit 1; }

echo "========================================================="
echo "🎉 PTR Deployment Completed Successfully (v${CLEAN_VERSION})!"
echo "🕹️  Play live: https://darkrift.ai"
echo "========================================================="
