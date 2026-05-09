#!/usr/bin/env python3
"""
Shadows of War Cluster Runner: Server + Web Client + Native Client.

Builds and runs:
1. wasm-pack build for the Web Worker
2. npm install and vite dev server for the UI
3. cargo build and execution for Server and Native Egui apps
"""
import os
import argparse
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

COLORS = {
    "SERVER": "\033[95m",
    "WEB-UI": "\033[96m",
    "NATIVE": "\033[92m",
    "BUILD": "\033[93m",
    "RESET": "\033[0m",
}

SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent

def _describe_exit(code: int) -> str:
    if code == 0:
        return ""
    if code < 0:
        sig = -code
        return f"stopped by signal {sig}"
    return f"exited with error code {code}"

def stream_output(process, prefix, color_key):
    color = COLORS.get(color_key, COLORS["RESET"])
    for line in iter(process.stdout.readline, ""):
        if not line:
            break
        print(f"{color}[{prefix}] {line.rstrip()}{COLORS['RESET']}", flush=True)

    code = process.wait()
    msg = _describe_exit(code)
    if code != 0:
        print(f"{color}[{prefix}] {msg}{COLORS['RESET']}", flush=True)

def ensure_wasm_pack_installed(env):
    try:
        subprocess.run(["wasm-pack", "--version"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, env=env)
    except (subprocess.CalledProcessError, FileNotFoundError):
        print("📥 wasm-pack not found. Installing wasm-pack via cargo...")
        subprocess.run(["cargo", "install", "wasm-pack"], check=True, env=env)

def main():
    parser = argparse.ArgumentParser(description="Run Shadows of War local cluster (server + web client + native client).")
    parser.add_argument("--release", action="store_true", help="Force release build/run.")
    args = parser.parse_args()

    os.chdir(PROJECT_ROOT)

    print("🚀 Starting Shadows of War Cluster...")
    
    print("🧹 Cleaning up any existing zombie processes...")
    try:
        subprocess.run(["pkill", "-f", "sow-server"], check=False, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        subprocess.run(["pkill", "-f", "sow-client"], check=False, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except Exception:
        pass

    env = os.environ.copy()
    env.setdefault("RUST_BACKTRACE", "1")
    env.setdefault("RUST_LOG", "info")
    cargo_bin = str(Path.home() / ".cargo" / "bin")
    if cargo_bin not in env.get("PATH", ""):
        env["PATH"] = cargo_bin + os.pathsep + env.get("PATH", "")

    use_release = args.release
    profile = "release" if use_release else "debug"

    server_bin = PROJECT_ROOT / "target" / profile / "sow-server"
    client_bin = PROJECT_ROOT / "target" / profile / "sow-client"

    def spawn_process(name, cmd, cwd=None):
        return subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
            env=env,
            cwd=cwd
        )

    processes = []
    handled_signal = {"num": None}

    def _signal_handler(signum, _frame):
        handled_signal["num"] = signum
        raise KeyboardInterrupt()

    signal.signal(signal.SIGINT, _signal_handler)
    signal.signal(signal.SIGTERM, _signal_handler)

    try:


        print(f"🛠️  Building native binaries ({profile})...")
        build_cmd = ["cargo", "build", "-p", "sow-server", "-p", "sow-client"]
        if use_release:
            build_cmd.append("--release")
        subprocess.run(build_cmd, env=env, check=True)

        print("🛠️  Launching Server...")
        server_p = spawn_process("SERVER", [str(server_bin)])
        processes.append(("SERVER", server_p))
        threading.Thread(target=stream_output, args=(server_p, "SERVER", "SERVER"), daemon=True).start()



        time.sleep(2)

        print("🎮 Launching Native Client 1...")
        client_p1 = spawn_process("NATIVE 1", [str(client_bin)])
        processes.append(("NATIVE 1", client_p1))
        threading.Thread(target=stream_output, args=(client_p1, "NATIVE 1", "NATIVE"), daemon=True).start()

        time.sleep(1)

        print("🎮 Launching Native Client 2...")
        client_p2 = spawn_process("NATIVE 2", [str(client_bin)])
        processes.append(("NATIVE 2", client_p2))
        threading.Thread(target=stream_output, args=(client_p2, "NATIVE 2", "NATIVE"), daemon=True).start()

        print("\n✅ Cluster fully booted! Press Ctrl+C to shutdown all instances.\n")
        server_p.wait()
        client_p1.wait()
        client_p2.wait()

    except KeyboardInterrupt:
        if handled_signal["num"] is not None:
            print(f"\n🛑 Shutdown sequence initiated (signal {handled_signal['num']} caught)... killing processes.")
        else:
            print("\n🛑 Shutdown sequence initiated (Ctrl+C caught)... killing processes.")
    except subprocess.CalledProcessError as e:
        print(f"\n❌ Build failed: {e}")
    finally:
        for name, p in processes:
            if p.poll() is None:
                print(f"Terminating {name}...")
                p.terminate()
                try:
                    p.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    print(f"Force killing {name}...")
                    p.kill()
        print("✨ Cluster shutdown complete.")

if __name__ == "__main__":
    main()
