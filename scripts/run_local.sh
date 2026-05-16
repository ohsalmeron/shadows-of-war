#!/bin/bash
# Shadows of War - Local Orchestrator

# Fail fast, but allow cleanup
set -e

# --- Poka Yoke: Cleanup old instances ---
echo "🧹 Cleaning up old instances..."
pkill -f "sow-server" || true
pkill -f "cargo leptos" || true
pkill -f "sow-web" || true
# Ensure ports are freed
sleep 1

# --- Versioning ---
if [ ! -f .version ]; then
    echo "1" > .version
fi
VERSION=$(cat .version)
NEW_VERSION=$((VERSION + 1))
echo $NEW_VERSION > .version
export SOW_BUILD_VERSION="0.1.$NEW_VERSION"
echo "🏷️  Version bumped to 0.1.$NEW_VERSION"

echo "============================================="
echo "🎮 Starting SOW Local Orchestrator (v0.1.$NEW_VERSION)..."
echo "============================================="

# 1. Build the Relay Server
echo "⚙️  1/3 Compiling Relay Server (sow-server)..."
if ! cargo build --release -p sow-server -p sow-relay; then
    echo "❌ Build failed. Rolling back version..."
    echo $VERSION > .version
    exit 1
fi

mkdir -p .logs
echo "🗺️  Booting Local Map Server (Master Relay) on port 25566..."
SOW_MAPS_ROOT=dist/assets/maps nohup target/release/sow-server > .logs/sow-server.log 2>&1 &

# 2. Build the WebAssembly Client
echo "📦 2/2 Compiling WebAssembly Client (sow-client)..."
if ! cargo build --release -p sow-client --target wasm32-unknown-unknown; then
    echo "❌ Client build failed. Rolling back version..."
    echo $VERSION > .version
    exit 1
fi

echo "🧩 Generating JS bindings..."
mkdir -p sow-web/public/assets
if ! wasm-bindgen --target web --no-typescript \
    --out-dir sow-web/public/assets \
    target/wasm32-unknown-unknown/release/sow_client.wasm; then
    echo "❌ Client bindings generation failed. Rolling back version..."
    echo $VERSION > .version
    exit 1
fi

# 3. Start Leptos Watch Server
echo "🚀 3/3 Booting Leptos Orchestrator (sow-web)..."
echo "🌐 Open your browser to: http://localhost:3333"
echo "============================================="

cd sow-web
cargo leptos watch 2>&1 | tee ../.logs/sow-web.log
