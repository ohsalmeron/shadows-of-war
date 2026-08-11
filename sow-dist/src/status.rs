use super::*;
use std::thread;

pub(super) fn execute() -> Result<()> {
    let prod_host = env::var("SOW_PROD_HOST").unwrap_or_else(|_| "sow".into());

    let azure = collect_azure(&prod_host);
    let local = collect_local();

    println!("─── SOW MONITOR ───────────────────────────────────────────");
    println!();
    println!("Azure ({prod_host})");
    for line in &azure.lines {
        if !line.is_empty() {
            println!("  {line}");
        }
    }
    println!();
    println!("  {local}");
    println!();
    println!("───────────────────────────────────────────────────────────");
    Ok(())
}

#[derive(Default)]
struct AzureInfo {
    lines: Vec<String>,
}

const AZURE_PY_SCRIPT: &str = r#"import subprocess

try:
    cp_time = subprocess.check_output(["sysctl", "-n", "kern.cp_time"]).decode().split()
    parts = [float(x) for x in cp_time]
    active = sum(parts[:4])
    total = active + parts[4]
    cpu_str = f"{(active/total)*100:.1f}%" if total > 0 else "0%"
except Exception:
    cpu_str = "?"

try:
    mem_out = subprocess.check_output(["sysctl", "-n", "vm.stats.vm.v_active_count", "vm.stats.vm.v_wire_count", "hw.physmem"]).decode().splitlines()
    active_p = float(mem_out[0])
    wire_p = float(mem_out[1])
    phys_b = float(mem_out[2])
    used_mb = (active_p + wire_p) * 4096 / (1024 * 1024)
    total_mb = phys_b / (1024 * 1024)
    mem_str = f"{used_mb:.0f}M/{total_mb:.0f}M"
except Exception:
    mem_str = "?"

try:
    zfs_str = subprocess.check_output(["sudo", "zpool", "status", "-x"]).decode().splitlines()[0].strip()
except Exception:
    zfs_str = "?"

try:
    errors_raw = subprocess.check_output(["sudo", "tail", "-50", "/var/log/sow/server.log"]).decode()
    errors_str = str(sum(1 for line in errors_raw.splitlines() if "error" in line.lower() or "panic" in line.lower()))
except Exception:
    errors_str = "0"

try:
    valkey_raw = subprocess.check_output(["valkey-cli", "-h", "127.0.0.1", "info", "stats"]).decode()
    valkey_ops = next((line.split(":")[1].strip() for line in valkey_raw.splitlines() if "instantaneous_ops_per_sec" in line), "?")
    valkey_mem_raw = subprocess.check_output(["valkey-cli", "-h", "127.0.0.1", "info", "memory"]).decode()
    valkey_mem = next((line.split(":")[1].strip() for line in valkey_mem_raw.splitlines() if "used_memory_human" in line), "?")
except Exception:
    valkey_ops, valkey_mem = "?", "?"

try:
    ps_out = subprocess.check_output(["ps", "aux"]).decode()
    relays = []
    for line in ps_out.splitlines():
        if "sow-relay --port" in line:
            parts = line.split()
            pid = parts[1]
            try:
                port_idx = parts.index("--port") + 1
                port = int(parts[port_idx])
                relays.append((pid, port))
            except Exception:
                pass
except Exception:
    relays = []

try:
    netstat_out = subprocess.check_output(["netstat", "-an"]).decode()
    port_socket_counts = {}
    total_tcp_sockets = 0
    worker_bot_sockets = 0
    worker_ips = {"74.208.246.177", "20.187.76.160", "185.166.215.112", "13.70.37.120"}

    for line in netstat_out.splitlines():
        if "ESTABLISHED" in line and "tcp" in line:
            total_tcp_sockets += 1
            parts = line.split()
            if len(parts) >= 5:
                local_addr = parts[3]
                foreign_addr = parts[4]
                port_str = local_addr.rpartition(".")[2] if "." in local_addr else local_addr.rpartition(":")[2]
                if port_str.isdigit():
                    p = int(port_str)
                    if 25590 <= p <= 26500:
                        port_socket_counts[p] = port_socket_counts.get(p, 0) + 1
                
                foreign_ip = foreign_addr.rpartition(".")[0] if "." in foreign_addr else foreign_addr.rpartition(":")[0]
                if foreign_ip in worker_ips:
                    worker_bot_sockets += 1
except Exception:
    port_socket_counts = {}
    total_tcp_sockets = 0
    worker_bot_sockets = 0

total_relay_procs = len(relays)
healthy_relays = 0
zombie_relays = 0
counts = []

for pid, port in relays:
    c = port_socket_counts.get(port, 0)
    if c > 0:
        healthy_relays += 1
        counts.append(c)
    else:
        zombie_relays += 1

in_game_players = sum(counts)
min_p = min(counts) if counts else 0
max_p = max(counts) if counts else 0
avg_p = sum(counts) / len(counts) if counts else 0.0

print(f"CPU: {cpu_str}  RAM: {mem_str}  ZFS: {zfs_str}  Errors: {errors_str}")
print(f"Relays: {total_relay_procs} Total ({healthy_relays} Healthy, {zombie_relays} Zombie/Empty) | In-Game Relay WS: {in_game_players} (Total System TCP: {total_tcp_sockets})")
print(f"Players per Relay: Min {min_p} | Max {max_p} | Avg {avg_p:.1f} players/relay")
print(f"In-Game Active Players: {in_game_players} (connected to {healthy_relays} live relays)")
print(f"Worker Connections to Server: {worker_bot_sockets} ESTABLISHED sockets | Valkey: {valkey_ops} ops/s, {valkey_mem}")
"#;



fn b64_encode(data: &str) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = data.as_bytes();
    let mut res = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i] as u32;
        let b1 = if i + 1 < bytes.len() {
            bytes[i + 1] as u32
        } else {
            0
        };
        let b2 = if i + 2 < bytes.len() {
            bytes[i + 2] as u32
        } else {
            0
        };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        res.push(CHARS[((triple >> 18) & 63) as usize] as char);
        res.push(CHARS[((triple >> 12) & 63) as usize] as char);
        if i + 1 < bytes.len() {
            res.push(CHARS[((triple >> 6) & 63) as usize] as char);
        } else {
            res.push('=');
        }
        if i + 2 < bytes.len() {
            res.push(CHARS[(triple & 63) as usize] as char);
        } else {
            res.push('=');
        }
        i += 3;
    }
    res
}

fn run_b64_py(host: &str, code: &str) -> Option<String> {
    let b64 = b64_encode(code);
    let remote_cmd =
        format!("python3 -c \"import base64; exec(base64.b64decode('{b64}').decode())\"");
    let output = Command::new("ssh")
        .args([
            "-o",
            "ConnectTimeout=5",
            "-o",
            "BatchMode=yes",
            host,
            &remote_cmd,
        ])
        .output();

    match output {
        Ok(o) if o.status.success() => Some(String::from_utf8_lossy(&o.stdout).trim().to_string()),
        _ => None,
    }
}

fn collect_azure(host: &str) -> AzureInfo {
    if let Some(res) = run_b64_py(host, AZURE_PY_SCRIPT) {
        let lines = res.lines().map(String::from).collect();
        AzureInfo { lines }
    } else {
        AzureInfo {
            lines: vec!["UNREACHABLE".to_string()],
        }
    }
}


fn collect_local() -> String {
    let n = Command::new("/bin/sh")
        .args(["-c", "pgrep -f 'sow-(server|relay|database)' 2>/dev/null | wc -l"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "0".into());

    format!("local: {} sow processes", n.trim())
}
