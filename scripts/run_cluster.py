#!/usr/bin/env python3
"""
Shadows of War cluster runner.

Deploys release server + maps to VPS and builds/deploys the WASM web client.
"""
import os
import subprocess
import time
import shutil
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent

# DEPLOYMENT CONFIGURATION
# Impersonating dark-rift for now. To migrate, change these variables!
VPS_HOST = "74.208.246.177"
VPS_USER = "bizkit"

# Server (Backend) Dirs
BACKEND_DEST = f"/home/{VPS_USER}/darkrift"
SYSTEMD_UNIT = "darkrift-server"
SERVER_MAPS_DEST = f"/home/{VPS_USER}/dark-rift-prod/assets/maps"

# Web (Frontend) Dirs
WEB_ROOT_DEST = "/var/www/darkrift.ai/html"
WEB_MAPS_DEST = f"{WEB_ROOT_DEST}/assets/maps"

def main():
    os.chdir(PROJECT_ROOT)
    
    print("🚀 Starting Shadows of War Cloud Deployment...")
    
    env = os.environ.copy()
    env.setdefault("RUST_BACKTRACE", "1")
    cargo_bin = str(Path.home() / ".cargo" / "bin")
    if cargo_bin not in env.get("PATH", ""):
        env["PATH"] = cargo_bin + os.pathsep + env.get("PATH", "")

    # 1. Build WASM Client
    print("🛠️  1. Building sow-client for WebAssembly (release)...")
    subprocess.run(["cargo", "build", "--release", "-p", "sow-client", "--target", "wasm32-unknown-unknown"], env=env, check=True)

    # 2. Build Backend Server
    print("☁️  2. Building sow-server for backend deployment (release)...")
    musl_cmd = ["cargo", "build", "--release", "-p", "sow-server", "--target", "x86_64-unknown-linux-musl"]
    gnu_cmd = ["cargo", "build", "--release", "-p", "sow-server", "--target", "x86_64-unknown-linux-gnu"]
    
    if subprocess.run(musl_cmd, env=env, check=False).returncode == 0:
        server_bin = PROJECT_ROOT / "target" / "x86_64-unknown-linux-musl" / "release" / "sow-server"
        print(f"✅ Built musl binary: {server_bin}")
    else:
        print("⚠️ Musl build failed, falling back to gnu target...")
        subprocess.run(gnu_cmd, env=env, check=True)
        server_bin = PROJECT_ROOT / "target" / "x86_64-unknown-linux-gnu" / "release" / "sow-server"
        print(f"✅ Built gnu binary: {server_bin}")

    # 3. Package WASM
    print("📦 3. Packaging WASM with wasm-bindgen...")
    wasm_in = PROJECT_ROOT / "target" / "wasm32-unknown-unknown" / "release" / "sow_client.wasm"
    dist_dir = PROJECT_ROOT / "dist"
    
    if dist_dir.exists():
        shutil.rmtree(dist_dir)
    dist_dir.mkdir()
    
    build_ts = str(int(time.time()))
    out_name = f"sow_client_{build_ts}"
    js_file = f"{out_name}.js"
    wasm_file = f"{out_name}_bg.wasm"
    
    subprocess.run([
        "wasm-bindgen",
        "--out-dir", str(dist_dir),
        "--target", "web",
        "--out-name", out_name,
        "--no-typescript",
        str(wasm_in)
    ], env=env, check=True)
    
    wasm_out_path = dist_dir / wasm_file
    if shutil.which("wasm-opt"):
        print("🗜️  Optimizing WASM with wasm-opt...")
        subprocess.run(["wasm-opt", "--all-features", "-O3", "-o", str(wasm_out_path), str(wasm_out_path)], check=True)
    else:
        print("⚠️  'wasm-opt' not found! Skipping advanced WASM optimization (sudo pacman -S binaryen)")

    # 4. Assemble Web Client (UI, sw.js, HTML)
    print("🎨 4. Assembling Web UI & Assets...")
    
    # Copy from local web assets
    web_dir = PROJECT_ROOT / "web"
    if (web_dir / "favicon_io").exists():
        shutil.copytree(web_dir / "favicon_io", dist_dir / "favicon_io", dirs_exist_ok=True)
        for ext in ["png", "ico", "json"]:
            for file in (dist_dir / "favicon_io").glob(f"*.{ext}"):
                shutil.copy2(file, dist_dir / file.name)
    if (web_dir / "sw.js").exists():
        shutil.copy2(web_dir / "sw.js", dist_dir / "sw.js")
    
    # Copy UI loader assets
    shutil.copytree(PROJECT_ROOT / "assets", dist_dir / "assets", dirs_exist_ok=True)
    
    # Template
    template_path = web_dir / "index.html.template"
    if not template_path.exists() and (web_dir / "index.html").exists():
        template_str = (web_dir / "index.html").read_text(encoding="utf-8")
    else:
        template_str = template_path.read_text(encoding="utf-8")
    
    version_file = PROJECT_ROOT / "version.txt"
    if version_file.exists():
        version = version_file.read_text(encoding="utf-8").strip()
    else:
        version = "0.1.0"
        
    parts = version.split('.')
    if len(parts) == 3:
        try:
            parts[2] = str(int(parts[2]) + 1)
            version = '.'.join(parts)
        except ValueError:
            pass
            
    version_file.write_text(version, encoding="utf-8")
    template_str = template_str.replace("__VERSION__", version)
    template_str = template_str.replace("__JS_FILE__", js_file)
    template_str = template_str.replace("__WASM_FILE__", wasm_file)
    template_str = template_str.replace("__BUILD_TS__", build_ts)
    
    (dist_dir / "index.html").write_text(template_str, encoding="utf-8")

    # 5. Compress
    print("🗜️  5. Compressing Web Assets (Brotli)...")
    if shutil.which("brotli"):
        subprocess.run(["brotli", "-f", "-Z", str(dist_dir / wasm_file)], check=False)
        subprocess.run(["brotli", "-f", "-Z", str(dist_dir / js_file)], check=False)
        
        # Compress maps too
        maps_dir = dist_dir / "assets" / "maps"
        if maps_dir.exists():
            for root, _, files in os.walk(maps_dir):
                for file in files:
                    if file.endswith((".bin", ".json", ".webp")):
                        subprocess.run(["brotli", "-f", "-Z", os.path.join(root, file)], check=False)
                        
        print("✅ Brotli compression finished.")
    else:
        print("⚠️ 'brotli' command not found, skipping compression.")

    # 6. Deploy
    print(f"☁️  6. Deploying to {VPS_USER}@{VPS_HOST}...")
    subprocess.run(["ssh", f"{VPS_USER}@{VPS_HOST}", f"mkdir -p {BACKEND_DEST} {SERVER_MAPS_DEST} {WEB_ROOT_DEST}"], check=True)
    
    print("   -> Uploading Backend Binary...")
    subprocess.run(["rsync", "-avz", str(server_bin), f"{VPS_USER}@{VPS_HOST}:{BACKEND_DEST}/dark-rift-server"], check=True)
    
    maps_src = dist_dir / "assets" / "maps"
    print("   -> Uploading Map Assets (Backend)...")
    subprocess.run(["rsync", "-avz", f"{maps_src}/", f"{VPS_USER}@{VPS_HOST}:{SERVER_MAPS_DEST}/"], check=True)
    
    print("   -> Uploading Frontend Web App...")
    subprocess.run(["rsync", "-avz", "--delete", "--exclude", "assets/maps", f"{dist_dir}/", f"{VPS_USER}@{VPS_HOST}:{WEB_ROOT_DEST}/"], check=True)
    
    print("   -> Uploading Map Assets (Web)...")
    subprocess.run(["rsync", "-avz", f"{maps_src}/", f"{VPS_USER}@{VPS_HOST}:{WEB_MAPS_DEST}/"], check=True)
    
    print(f"🔄 7. Restarting Backend Service: {SYSTEMD_UNIT}...")
    subprocess.run(["ssh", f"{VPS_USER}@{VPS_HOST}", f"sudo systemctl restart {SYSTEMD_UNIT}"], check=True)

    print("\n=========================================================")
    print("🎉 Master Deployment Completed Successfully!")
    print(f"🕹️  Play live: https://darkrift.ai")
    print("=========================================================\n")

if __name__ == "__main__":
    main()
