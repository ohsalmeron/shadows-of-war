#!/usr/bin/env bash
# Shadows of War - Local Testing Script
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

echo "========================================================="
echo "🚀 Starting Local Environment"
echo "========================================================="

# Clean up function to kill child processes on exit
cleanup() {
    echo "🧹 Cleaning up background processes..."
    kill $SERVER_PID $CLIENT1_PID $CLIENT2_PID 2>/dev/null || true
    if [ -n "${REDIS_PID:-}" ]; then
        kill $REDIS_PID 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

# 1. Fast Compilation (Debug mode)
echo "==> Compiling server, relay, and client..."
# We compile without --release to build as fast as possible
cargo build -p sow-server -p sow-relay -p sow-client
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
