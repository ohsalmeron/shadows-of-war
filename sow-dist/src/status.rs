use super::*;

pub(super) fn execute() -> Result<()> {
    let prod_host = env::var("SOW_PROD_HOST").unwrap_or_else(|_| "ionos".into());
    let relay_host = env::var("SOW_RELAY_HOST_SSH").unwrap_or_else(|_| "relay".into());

    let production = collect_production(&prod_host);
    let relay = collect_relay(&relay_host);
    let local = collect_local();

    println!("─── SOW MONITOR ───────────────────────────────────────────");
    println!();
    println!("IONOS production ({prod_host})");
    for line in &production.lines {
        if !line.is_empty() {
            println!("  {line}");
        }
    }
    println!();
    println!("Azure F-Stack relay ({relay_host})");
    for line in &relay.lines {
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
struct HostInfo {
    lines: Vec<String>,
}

const PRODUCTION_PY_SCRIPT: &str = r#"import subprocess

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
    netstat_out = subprocess.check_output(["netstat", "-an"]).decode()
    total_tcp_sockets = 0
    for line in netstat_out.splitlines():
        if "ESTABLISHED" in line and "tcp" in line:
            total_tcp_sockets += 1
except Exception:
    total_tcp_sockets = 0

print(f"CPU: {cpu_str}  RAM: {mem_str}  ZFS: {zfs_str}  Errors: {errors_str}")
print(f"Established TCP sockets: {total_tcp_sockets} | Valkey: {valkey_ops} ops/s, {valkey_mem}")
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

fn collect_production(host: &str) -> HostInfo {
    if let Some(res) = run_b64_py(host, PRODUCTION_PY_SCRIPT) {
        let lines = res.lines().map(String::from).collect();
        HostInfo { lines }
    } else {
        HostInfo {
            lines: vec!["UNREACHABLE".to_string()],
        }
    }
}

fn collect_relay(host: &str) -> HostInfo {
    let remote_cmd = concat!(
        "systemctl is-active sow-relay@0 sow-relay@1 sow-relay@2 sow-relay@3; ",
        "for p in 8080 8081 8082 8083; do ",
        "curl -kfsS --max-time 3 https://127.0.0.1:$p/healthz >/dev/null ",
        "&& printf 'mgmt-%s=ok\\n' $p || printf 'mgmt-%s=failed\\n' $p; done"
    );
    let output = Command::new("ssh")
        .args([
            "-o", "ConnectTimeout=5", "-o", "BatchMode=yes", host,
            remote_cmd,
        ])
        .output();
    match output {
        Ok(o) if o.status.success() => HostInfo {
            lines: String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(String::from)
                .collect(),
        },
        _ => HostInfo {
            lines: vec!["UNREACHABLE".to_string()],
        },
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
