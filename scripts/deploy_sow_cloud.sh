#!/usr/bin/env bash
# Deploy sow-server (musl or gnu) + map assets to VPS; restart systemd unit.
# Same host/user/rsync pattern as dark-rift/scripts/deploy_cloud.sh — separate tree under ~/sow.
#
# Sample systemd unit (install as /etc/systemd/system/sow-server.service):
#   [Unit]
#   Description=Shadows of War relay server
#   After=network.target
#   [Service]
#   User=bizkit
#   WorkingDirectory=/home/bizkit/sow-prod
#   Environment=SOW_MAPS_ROOT=/home/bizkit/sow-prod/maps
#   Environment=SOW_WS_LISTEN=0.0.0.0:25565
#   Environment=SOW_MAPS_HTTP_LISTEN=0.0.0.0:25566
#   ExecStart=/home/bizkit/sow/sow-server
#   Restart=on-failure
#   [Install]
#   WantedBy=multi-user.target
#
# Note: port 25565 may conflict with darkrift-server on the same VPS — use different
# SOW_WS_LISTEN / SOW_MAPS_HTTP_LISTEN in the unit and matching client SOW_WS_URL / SOW_MAPS_URL.
#
# Firewall (manual): allow TCP for WS and maps ports, e.g.
#   sudo ufw allow 25565/tcp && sudo ufw allow 25566/tcp
#
# Local build deps: rustup target add x86_64-unknown-linux-musl (script falls back to gnu if musl fails).
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

VPS_IP="${VPS_IP:-74.208.246.177}"
VPS_USER="${VPS_USER:-bizkit}"
BACKEND_DEST_DIR="${BACKEND_DEST_DIR:-/home/bizkit/sow}"
MAPS_DEST_DIR="${MAPS_DEST_DIR:-/home/bizkit/sow-prod/maps}"
SYSTEMD_UNIT="${SYSTEMD_UNIT:-sow-server}"

# Align with client defaults when using standard ports (override via env on VPS).
SOW_WS_PORT="${SOW_WS_PORT:-25565}"
SOW_MAPS_PORT="${SOW_MAPS_PORT:-25566}"

export CARGO_TARGET_DIR="${ROOT}/target"

green() { echo -e "\e[32m$1\e[0m"; }
cyan() { echo -e "\e[36m$1\e[0m"; }
red() { echo -e "\e[31m$1\e[0m"; }

MAPS_SRC="${ROOT}/OpenFrontIO/resources/maps"
if [[ ! -d "${MAPS_SRC}" ]]; then
  red "Missing map source directory: ${MAPS_SRC}"
  exit 1
fi

MUSL_BIN="${CARGO_TARGET_DIR}/x86_64-unknown-linux-musl/release/sow-server"
GNU_BIN="${CARGO_TARGET_DIR}/x86_64-unknown-linux-gnu/release/sow-server"

echo "========================================================="
cyan "Shadows of War — deploy server to ${VPS_USER}@${VPS_IP}"
echo "Ports (reference for firewall/client): WS=${SOW_WS_PORT} maps HTTP=${SOW_MAPS_PORT}"
echo "========================================================="

cyan "==> Build sow-server (release)..."
set +e
cargo build --release -p sow-server --target x86_64-unknown-linux-musl
MUSL_OK=$?
set -e

REMOTE_BIN=""
if [[ "$MUSL_OK" -eq 0 ]] && [[ -f "${MUSL_BIN}" ]]; then
  REMOTE_BIN="${MUSL_BIN}"
  green "Built musl binary: ${REMOTE_BIN}"
else
  cyan "Musl build failed or unavailable; trying gnu target..."
  cargo build --release -p sow-server --target x86_64-unknown-linux-gnu
  REMOTE_BIN="${GNU_BIN}"
  green "Built gnu binary: ${REMOTE_BIN}"
fi

cyan "==> rsync server binary..."
ssh "${VPS_USER}@${VPS_IP}" "mkdir -p ${BACKEND_DEST_DIR}"
rsync -avz "${REMOTE_BIN}" "${VPS_USER}@${VPS_IP}:${BACKEND_DEST_DIR}/"

cyan "==> rsync map assets..."
ssh "${VPS_USER}@${VPS_IP}" "mkdir -p ${MAPS_DEST_DIR}"
rsync -avz "${MAPS_SRC}/" "${VPS_USER}@${VPS_IP}:${MAPS_DEST_DIR}/"

cyan "==> restart systemd unit: ${SYSTEMD_UNIT}"
ssh "${VPS_USER}@${VPS_IP}" "sudo systemctl restart ${SYSTEMD_UNIT}" || {
  red "systemctl restart failed — ensure ${SYSTEMD_UNIT} exists on the VPS"
  exit 1
}

echo "========================================================="
green "Deploy finished."
echo "Example client env (adjust host/ports if needed):"
echo "  export SOW_WS_URL=ws://${VPS_IP}:${SOW_WS_PORT}"
echo "  export SOW_MAPS_URL=http://${VPS_IP}:${SOW_MAPS_PORT}/maps"
echo "========================================================="
