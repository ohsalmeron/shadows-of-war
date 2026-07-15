use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

const RELAY_PORT_MIN: u16 = 25590;
const RELAY_PORT_MAX: u16 = 26500;

struct InterfaceStats {
    rx_bytes: u64,
    tx_bytes: u64,
}

struct CpuTicks {
    user: u64,
    nice: u64,
    system: u64,
    intr: u64,
    idle: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut stdout = io::stdout();

    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    })?;

    print!("\x1B[?1049h\x1B[H\x1B[2J");
    stdout.flush()?;

    let mut last_cpu_ticks = get_cpu_ticks().ok();
    let mut last_if_stats = get_interface_stats().unwrap_or_default();
    let mut last_time = Instant::now();

    while running.load(std::sync::atomic::Ordering::SeqCst) {
        let now = Instant::now();
        let elapsed = now.duration_since(last_time).as_secs_f64();
        last_time = now;

        let current_cpu_ticks = get_cpu_ticks().ok();
        let current_if_stats = get_interface_stats().unwrap_or_default();
        let (mem_used_bytes, mem_total_bytes) = get_memory_info().unwrap_or((0, 0));
        let socket_info = get_socket_info().unwrap_or_default();
        let relay_process_count = get_relay_process_count().unwrap_or(0);
        let hostname = get_hostname().unwrap_or_else(|_| "unknown".to_string());
        let uptime_str = get_uptime().unwrap_or_else(|_| "unknown".to_string());

        let mut cpu_pct = 0.0;
        if let (Some(prev), Some(curr)) = (&last_cpu_ticks, &current_cpu_ticks) {
            let user_diff = curr.user.saturating_sub(prev.user);
            let nice_diff = curr.nice.saturating_sub(prev.nice);
            let sys_diff = curr.system.saturating_sub(prev.system);
            let intr_diff = curr.intr.saturating_sub(prev.intr);
            let idle_diff = curr.idle.saturating_sub(prev.idle);

            let active = user_diff + nice_diff + sys_diff + intr_diff;
            let total = active + idle_diff;
            if total > 0 {
                cpu_pct = (active as f64 / total as f64) * 100.0;
            }
        }
        last_cpu_ticks = current_cpu_ticks;

        let established_total = socket_info
            .state_counts
            .get("ESTABLISHED")
            .copied()
            .unwrap_or(0);
        let listen_total = socket_info
            .state_counts
            .get("LISTEN")
            .copied()
            .unwrap_or(0);
        let time_wait_total = socket_info
            .state_counts
            .get("TIME_WAIT")
            .copied()
            .unwrap_or(0);

        let relay_established: u32 = socket_info.port_conn_counts.values().map(|&c| c).sum();
        let total_sockets: u32 = socket_info.state_counts.values().sum();
        let maxfiles = get_maxfiles().unwrap_or(0);

        let used_gb = mem_used_bytes as f64 / 1_073_741_824.0;
        let total_gb = mem_total_bytes as f64 / 1_073_741_824.0;
        let mem_pct = if mem_total_bytes > 0 {
            (mem_used_bytes as f64 / mem_total_bytes as f64) * 100.0
        } else {
            0.0
        };

        // Build interface speed strings
        let mut vtnet_str = String::new();
        let mut lo_str = String::new();
        for (iface, curr_stats) in &current_if_stats {
            let last_stats = last_if_stats.get(iface);
            let (rx_speed, tx_speed) = if let Some(last) = last_stats {
                let r_bytes = curr_stats.rx_bytes.saturating_sub(last.rx_bytes);
                let t_bytes = curr_stats.tx_bytes.saturating_sub(last.tx_bytes);
                (format_bytes(r_bytes, elapsed), format_bytes(t_bytes, elapsed))
            } else {
                ("0.00 B/s".to_string(), "0.00 B/s".to_string())
            };
            let line = format!("{}  ↓ {}  ↑ {}", iface, rx_speed, tx_speed);
            if iface == "vtnet0" { vtnet_str = line; }
            else { lo_str = line; }
        }
        last_if_stats = current_if_stats;

        print!("\x1B[H\x1B[J");

        let bw = |s: &str| format!("\x1B[1;36m│\x1B[0m{:<70}\x1B[1;36m│\x1B[0m", s);

        print!("\x1B[1;36m┌──────────────────────────────────────────────────────────────────────────────┐\x1B[0m\n");
        print!("{}\n", bw(&format!(" HOST: \x1B[1;32m{} \x1B[0m    UPTIME: \x1B[1;33m{}", hostname, uptime_str)));
        print!("{}\n", bw(&format!(
            " CPU: \x1B[1;37m{:>5.1}%\x1B[0m  (4 cores)    MEM: \x1B[1;37m{:>5.1}%\x1B[0m  (\x1B[1;37m{:.2}G\x1B[0m/\x1B[1;37m{:.2}G\x1B[0m)",
            cpu_pct, mem_pct, used_gb, total_gb
        )));
        print!("\x1B[1;36m├──────────────────────────────────────────────────────────────────────────────┤\x1B[0m\n");

        print!("{}\n", bw(&format!(
            " MATCHES (sow-relay procs): \x1B[1;35m{:>5}\x1B[0m",
            relay_process_count
        )));
        print!("{}\n", bw(&format!(
            " PLAYERS IN RELAY (in-game): \x1B[1;32m{:>5}\x1B[0m",
            relay_established
        )));
        print!("{}\n", bw(&format!(
            " TCP SOCKETS  \x1B[1;37m{:>5}\x1B[0m / {}  (kern.maxfiles)",
            total_sockets, maxfiles
        )));
        print!("{}\n", bw(&format!(
            "   ESTABLISHED \x1B[1;32m{}\x1B[0m   LISTEN \x1B[1;36m{}\x1B[0m   TIME_WAIT \x1B[1;33m{}\x1B[0m",
            established_total, listen_total, time_wait_total
        )));
        print!("\x1B[1;36m├──────────────────────────────────────────────────────────────────────────────┤\x1B[0m\n");

        if !vtnet_str.is_empty() {
            print!("{}\n", bw(&format!(" {}", vtnet_str)));
        }
        if !lo_str.is_empty() {
            print!("{}\n", bw(&format!(" {}", lo_str)));
        }
        print!("\x1B[1;36m└──────────────────────────────────────────────────────────────────────────────┘\x1B[0m\n");

        stdout.flush()?;

        thread::sleep(Duration::from_millis(1000));
    }

    print!("\x1B[?1049l\x1B[2J\x1B[H");
    stdout.flush()?;
    println!("Exited Shadows of War Monitor cleanly.");
    Ok(())
}

fn format_bytes(bytes: u64, elapsed: f64) -> String {
    let rate = bytes as f64 / elapsed;
    if rate >= 1_048_576.0 {
        format!("{:.2} MB/s", rate / 1_048_576.0)
    } else if rate >= 1024.0 {
        format!("{:.2} KB/s", rate / 1024.0)
    } else {
        format!("{:.2} B/s", rate)
    }
}

fn get_relay_process_count() -> Result<u32, Box<dyn Error>> {
    let output = Command::new("pgrep").arg("sow-relay").output();
    match output {
        Ok(out) => {
            let out_str = String::from_utf8_lossy(&out.stdout);
            let count = out_str.lines().filter(|l| !l.is_empty()).count() as u32;
            Ok(count)
        }
        Err(_) => Ok(0),
    }
}

fn get_maxfiles() -> Result<u32, Box<dyn Error>> {
    let output = Command::new("sysctl").args(&["-n", "kern.maxfiles"]).output()?;
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(s.parse().unwrap_or(0))
}

#[derive(Default)]
struct SocketInfo {
    state_counts: HashMap<String, u32>,
    port_conn_counts: HashMap<u16, u32>,
}

fn get_socket_info() -> Result<SocketInfo, Box<dyn Error>> {
    let mut info = SocketInfo::default();

    let cmd_output = Command::new("netstat").arg("-an").output();

    let output_bytes = match cmd_output {
        Ok(out) => out.stdout,
        Err(_) => return Ok(info),
    };

    let output_str = String::from_utf8_lossy(&output_bytes);

    for line in output_str.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 && parts[0].starts_with("tcp") {
            let local_addr = parts[3];
            let state = parts.last().cloned().unwrap_or("");

            if let Some(port) = parse_port(local_addr) {
                let clean_state = state.trim().to_uppercase();
                *info.state_counts.entry(clean_state.clone()).or_insert(0) += 1;

                if clean_state == "ESTABLISHED" && port >= RELAY_PORT_MIN && port <= RELAY_PORT_MAX {
                    *info.port_conn_counts.entry(port).or_insert(0) += 1;
                }
            }
        }
    }

    Ok(info)
}

fn parse_port(addr: &str) -> Option<u16> {
    let last_delim = addr.rfind('.').or_else(|| addr.rfind(':'))?;
    let port_str = &addr[last_delim + 1..];
    port_str.parse::<u16>().ok()
}

fn get_interface_stats() -> Result<HashMap<String, InterfaceStats>, Box<dyn Error>> {
    let mut stats = HashMap::new();

    if cfg!(target_os = "freebsd") {
        let output = Command::new("netstat")
            .args(&["-i", "-b", "-n"])
            .output()?;
        let output_str = String::from_utf8_lossy(&output.stdout);

        for line in output_str.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 11
                && (parts[0] == "vtnet0" || parts[0] == "lo0")
                && parts[2].starts_with("<Link")
            {
                let iface = parts[0].to_string();
                let rx_bytes = parts[7].parse().unwrap_or(0);
                let tx_bytes = parts[10].parse().unwrap_or(0);

                stats.insert(
                    iface,
                    InterfaceStats {
                        rx_bytes,
                        tx_bytes,
                    },
                );
            }
        }
    } else {
        if let Ok(content) = fs::read_to_string("/proc/net/dev") {
            for line in content.lines() {
                if let Some(pos) = line.find(':') {
                    let iface = line[..pos].trim().to_string();
                    if iface == "lo"
                        || iface.starts_with("eth")
                        || iface.starts_with("en")
                        || iface.starts_with("wl")
                    {
                        let parts: Vec<&str> = line[pos + 1..].split_whitespace().collect();
                        if parts.len() >= 16 {
                            let rx_bytes = parts[0].parse().unwrap_or(0);
                            let tx_bytes = parts[8].parse().unwrap_or(0);

                            stats.insert(
                                iface,
                                InterfaceStats {
                                    rx_bytes,
                                    tx_bytes,
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(stats)
}

fn get_cpu_ticks() -> Result<CpuTicks, Box<dyn Error>> {
    if cfg!(target_os = "freebsd") {
        let output = Command::new("sysctl")
            .args(&["-n", "kern.cp_time"])
            .output()?;
        let output_str = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = output_str.split_whitespace().collect();
        if parts.len() >= 5 {
            return Ok(CpuTicks {
                user: parts[0].parse().unwrap_or(0),
                nice: parts[1].parse().unwrap_or(0),
                system: parts[2].parse().unwrap_or(0),
                intr: parts[3].parse().unwrap_or(0),
                idle: parts[4].parse().unwrap_or(0),
            });
        }
    } else {
        if let Ok(content) = fs::read_to_string("/proc/stat") {
            if let Some(line) = content.lines().next() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 && parts[0] == "cpu" {
                    let user = parts[1].parse().unwrap_or(0);
                    let nice = parts[2].parse().unwrap_or(0);
                    let system = parts[3].parse().unwrap_or(0);
                    let idle = parts[4].parse().unwrap_or(0);
                    return Ok(CpuTicks {
                        user,
                        nice,
                        system,
                        intr: 0,
                        idle,
                    });
                }
            }
        }
    }
    Err("Failed to get CPU ticks".into())
}

fn get_memory_info() -> Result<(u64, u64), Box<dyn Error>> {
    if cfg!(target_os = "freebsd") {
        let output = Command::new("sysctl")
            .args(&[
                "-n",
                "vm.stats.vm.v_page_count",
                "vm.stats.vm.v_free_count",
                "vm.stats.vm.v_wire_count",
                "vm.stats.vm.v_active_count",
                "hw.physmem",
            ])
            .output()?;
        let output_str = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = output_str.lines().collect();
        if lines.len() >= 5 {
            let wire_pages: u64 = lines[2].parse().unwrap_or(0);
            let active_pages: u64 = lines[3].parse().unwrap_or(0);
            let total_bytes: u64 = lines[4].parse().unwrap_or(0);

            let used_pages = wire_pages + active_pages;
            let used_bytes = used_pages * 4096;

            return Ok((used_bytes, total_bytes));
        }
    } else {
        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            let mut total = 0;
            let mut free = 0;
            let mut buffers = 0;
            let mut cached = 0;
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if parts[0] == "MemTotal:" {
                        total = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                    } else if parts[0] == "MemFree:" {
                        free = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                    } else if parts[0] == "Buffers:" {
                        buffers = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                    } else if parts[0] == "Cached:" {
                        cached = parts[1].parse::<u64>().unwrap_or(0) * 1024;
                    }
                }
            }
            let used = total.saturating_sub(free + buffers + cached);
            return Ok((used, total));
        }
    }
    Err("Failed to get memory info".into())
}

fn get_hostname() -> Result<String, Box<dyn Error>> {
    let output = Command::new("hostname").output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn get_uptime() -> Result<String, Box<dyn Error>> {
    if cfg!(target_os = "freebsd") {
        let output = Command::new("uptime").output()?;
        let uptime_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Some(pos) = uptime_str.find("up") {
            if let Some(end_pos) = uptime_str.find(",  ") {
                return Ok(uptime_str[pos + 3..end_pos].to_string());
            }
            if let Some(end_pos) = uptime_str.find(", ") {
                return Ok(uptime_str[pos + 3..end_pos].to_string());
            }
        }
        Ok(uptime_str)
    } else {
        if let Ok(content) = fs::read_to_string("/proc/uptime") {
            if let Some(secs_str) = content.split_whitespace().next() {
                let secs: f64 = secs_str.parse().unwrap_or(0.0);
                let h = (secs / 3600.0) as u32;
                let m = ((secs % 3600.0) / 60.0) as u32;
                return Ok(format!("{} hours, {} minutes", h, m));
            }
        }
        Ok("unknown".to_string())
    }
}
