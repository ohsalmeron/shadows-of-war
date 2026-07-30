use super::*;
use std::thread;

pub(super) fn execute() -> Result<()> {
    let prod_host = env::var("SOW_PROD_HOST").unwrap_or_else(|_| "sow".into());
    let backfill_hosts: Vec<String> = env::var("SOW_BACKFILL_HOSTS")
        .or_else(|_| env::var("SOW_BACKFILL_HOST"))
        .unwrap_or_else(|_| "sow-backfill1,sow-backfill2,ionos,clouding".into())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let (azure, workers) = thread::scope(|scope| {
        let azure_handle = scope.spawn(|| collect_azure(&prod_host));
        let worker_handles: Vec<_> = backfill_hosts
            .iter()
            .map(|host| scope.spawn(move || collect_worker(host)))
            .collect();

        let azure = azure_handle.join().unwrap_or_default();
        let workers: Vec<_> = worker_handles
            .into_iter()
            .map(|h| h.join().unwrap_or_default())
            .collect();
        (azure, workers)
    });

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
    println!("Workers");
    for w in &workers {
        println!("  {w}");
    }
    println!("  {local}");
    println!();
    println!("───────────────────────────────────────────────────────────");
    Ok(())
}

#[derive(Default)]
struct AzureInfo {
    lines: Vec<String>,
}

fn collect_azure(host: &str) -> AzureInfo {
    let script = r#"python3 -c '
import json, subprocess

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

try:
    status_raw = subprocess.check_output(["fetch", "-qo", "-", "http://127.0.0.1:25566/admin/api/status"]).decode()
    status = json.loads(status_raw)
    lobbies = status.get("lobbies", [])
    pregame_lobbies = len(lobbies)
    pregame_players = sum(len(l.get("players", [])) for l in lobbies)
except Exception:
    pregame_lobbies, pregame_players = 0, 0

total_active_players = pregame_players + in_game_players

print(f"CPU: {cpu_str}  RAM: {mem_str}  ZFS: {zfs_str}  Errors: {errors_str}")
print(f"Relays: {total_relay_procs} Total ({healthy_relays} Healthy, {zombie_relays} Zombie/Empty) | Sockets: {in_game_players} Relay WS (Total TCP: {total_tcp_sockets})")
print(f"Players per Relay: Min {min_p} | Max {max_p} | Avg {avg_p:.1f} players/relay")
print(f"Total Active Players: {total_active_players} (Pre-game: {pregame_players} in {pregame_lobbies} lobbies | In-Game: {in_game_players} in {healthy_relays} relays)")
print(f"Backfill Bot WebSockets: {worker_bot_sockets} ESTABLISHED sockets | Valkey: {valkey_ops} ops/s, {valkey_mem}")
'
"#;

    let output = Command::new("ssh")
        .args(["-o", "ConnectTimeout=5", "-o", "BatchMode=yes", host, script])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let lines = text.lines().map(String::from).collect();
            AzureInfo { lines }
        }
        _ => AzureInfo {
            lines: vec!["UNREACHABLE".to_string()],
        },
    }
}

fn collect_worker(host: &str) -> String {
    let script = r#"python3 -c '
import subprocess
try:
    ps_out = subprocess.check_output(["ps", "aux"]).decode()
    sup_count = 0
    for line in ps_out.splitlines():
        if "sow-backfill" in line and "--min-fill" in line:
            sup_count += 1
    
    netstat_out = subprocess.check_output(["netstat", "-an"]).decode()
    active_ws = 0
    for line in netstat_out.splitlines():
        if "ESTABLISHED" in line and "tcp" in line:
            if ".80" in line or ".443" in line or ":80" in line or ":443" in line:
                active_ws += 1

    sup_str = "1 Active Daemon" if sup_count >= 1 else "OFFLINE"
    print(f"Supervisor: {sup_str} | Active Bot WebSockets: {active_ws} ESTABLISHED sockets")
except Exception as e:
    print(f"ERROR: {e}")
'
"#;

    let output = Command::new("ssh")
        .args(["-o", "ConnectTimeout=5", "-o", "BatchMode=yes", host, script])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
            format!("{host}: {text}")
        }
        _ => format!("{host}: UNREACHABLE"),
    }
}

fn collect_local() -> String {
    let sup = Command::new("/bin/sh")
        .args(["-c", "pgrep -f 'sow-backfill.*--url' 2>/dev/null | wc -l"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "0".into());

    format!("local: {} sup daemon", sup.trim())
}
