#!/usr/bin/env bash
# Shadows of War - Local Testing Script
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

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

echo "========================================================="
echo "🚀 Starting Local Environment (v${CLEAN_VERSION})"
echo "========================================================="

# Clean up stale processes from previous runs to prevent Address In Use errors
echo "==> Cleaning up any stale game processes..."
killall sow-server sow-client sow-relay 2>/dev/null || true

# Clean up function to kill child processes on exit
cleanup() {
    echo "🧹 Cleaning up background processes..."
    if command -v redis-cli >/dev/null 2>&1; then
        redis-cli DEL sow:ports >/dev/null 2>&1 || true
    elif command -v valkey-cli >/dev/null 2>&1; then
        valkey-cli DEL sow:ports >/dev/null 2>&1 || true
    fi
    kill $SERVER_PID $CLIENT1_PID $CLIENT2_PID 2>/dev/null || true
    if [ -n "${REDIS_PID:-}" ]; then
        kill $REDIS_PID 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

# 1. Fast Compilation (Debug mode)
echo "==> Compiling server, relay, and client..."
# We compile without --release to build as fast as possible
cargo build --features sow-core/mem_profiler -p sow-server -p sow-relay -p sow-client
echo "✅ Compilation successful."

# 2. Check for Redis
REDIS_PID=""
if ! command -v redis-cli >/dev/null 2>&1 || ! redis-cli ping >/dev/null 2>&1; then
    echo "⚠️  Redis server is not responding. The server requires Redis."
    if command -v valkey-server >/dev/null 2>&1; then
        echo "==> Attempting to start valkey-server..."
        valkey-server &
        REDIS_PID=$!
    elif command -v redis-server >/dev/null 2>&1; then
        echo "==> Attempting to start redis-server..."
        redis-server &
        REDIS_PID=$!
    else
        echo "❌ redis-server not found in PATH! Please install and start Redis if connection fails."
    fi
    sleep 1 # Give redis a moment to start
fi

# Reset Redis ports so relay allocation starts fresh now that Redis/Valkey is running
if command -v redis-cli >/dev/null 2>&1; then
    redis-cli DEL sow:ports >/dev/null 2>&1 || true
elif command -v valkey-cli >/dev/null 2>&1; then
    valkey-cli DEL sow:ports >/dev/null 2>&1 || true
fi

# 3. Start the Server
echo "==> Starting Local Server..."
export SOW_MAPS_ROOT="${ROOT}/assets/maps"
export SOW_WS_LISTEN="127.0.0.1:25565"
export SOW_MAPS_HTTP_LISTEN="127.0.0.1:25566"
export RUST_LOG="info"

# Run from target/debug so it can find ./sow-relay
cd "${ROOT}/target/debug"
./sow-server &
SERVER_PID=$!

# Give the server a moment to bind its ports
sleep 1

# 4. Start 2 Clients
echo "==> Starting 2 Native Clients..."
export SOW_WS_URL="ws://127.0.0.1:25565"
export SOW_MAPS_URL="http://127.0.0.1:25566/maps"

./sow-client &
CLIENT1_PID=$!

sleep 0.5

./sow-client &
CLIENT2_PID=$!

echo "✅ All services started locally."
echo "🛑 Press Ctrl+C to stop all services."

# Wait for all background jobs to finish (or until interrupted)
wait
