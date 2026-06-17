use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};

fn echo_cmd(cmd: &str, args: &[&str]) {
    let line: Vec<String> = std::iter::once(cmd.to_string())
        .chain(args.iter().map(|a| shell_quote(a)))
        .collect();
    println!("+ {}", line.join(" "));
}

fn print_cmd_executed(cmd: &str, args: &[&str], env: &[(&str, &str)]) {
    if env.is_empty() {
        echo_cmd(cmd, args);
    } else {
        let prefix: Vec<String> = env
            .iter()
            .map(|(k, v)| format!("{k}={}", shell_quote(v)))
            .collect();
        let line: Vec<String> = prefix
            .into_iter()
            .chain(std::iter::once(cmd.to_string()))
            .chain(args.iter().map(|a| shell_quote(a)))
            .collect();
        println!("+ {}", line.join(" "));
    }
}

fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "./_=-+:,".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

pub fn run(cmd: &str, args: &[&str], cwd: Option<&Path>) -> Result<()> {
    run_env(cmd, args, cwd, &[])
}

pub fn run_env(cmd: &str, args: &[&str], cwd: Option<&Path>, env: &[(&str, &str)]) -> Result<()> {
    print_cmd_executed(cmd, args, env);
    let mut c = Command::new(cmd);
    c.args(args);
    if let Some(dir) = cwd {
        c.current_dir(dir);
    }
    for (k, v) in env {
        c.env(k, v);
    }
    // Inherit stdio — raw output goes straight to the terminal in real time.
    c.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    let status = c.spawn().with_context(|| format!("spawn {cmd}"))?.wait()?;
    if !status.success() {
        anyhow::bail!("{cmd} failed ({})", status);
    }
    Ok(())
}

pub fn which(cmd: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(cmd);
        if p.is_file() {
            return p.into_os_string().into_string().ok();
        }
    }
    None
}

pub fn output(cmd: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .with_context(|| format!("run {cmd}"))?;
    if !out.status.success() {
        print_cmd_executed(cmd, args, &[]);
        anyhow::bail!("{cmd} failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(unix)]
pub fn is_locked(path: &Path) -> bool {
    use std::os::unix::io::AsRawFd;
    if !path.exists() {
        return false;
    }
    if let Ok(file) = std::fs::File::open(path) {
        unsafe {
            let fd = file.as_raw_fd();
            // LOCK_EX (2) | LOCK_NB (4)
            let res = libc::flock(fd, 2 | 4);
            if res == 0 {
                libc::flock(fd, 8); // LOCK_UN
                false
            } else {
                true
            }
        }
    } else {
        false
    }
}

#[cfg(not(unix))]
pub fn is_locked(_path: &Path) -> bool {
    false
}

pub fn check_any_cargo_lock(cargo_target: &Path) -> bool {
    if !cargo_target.is_dir() {
        return false;
    }
    for entry in walkdir::WalkDir::new(cargo_target)
        .min_depth(1)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let name = entry.file_name().to_string_lossy();
        if (name == ".cargo-lock" || name == ".cargo-build-lock") && is_locked(entry.path()) {
            return true;
        }
    }
    false
}

/// Block until no other cargo build holds the target-directory lock.
/// Prints a status line every `interval` seconds so the terminal never looks hung.
pub fn wait_for_cargo_unlock(cargo_target: &Path) {
    if !check_any_cargo_lock(cargo_target) {
        return;
    }
    println!("==> Waiting for another cargo build to finish (target dir is locked)...");
    let interval = std::time::Duration::from_secs(3);
    let start = std::time::Instant::now();
    let mut warned_secs = 0u64;
    loop {
        std::thread::sleep(interval);
        if !check_any_cargo_lock(cargo_target) {
            let elapsed = start.elapsed().as_secs();
            println!("==> Cargo lock released after {elapsed}s — proceeding.");
            return;
        }
        let elapsed = start.elapsed().as_secs();
        // Print a reminder every 15 s so the user knows we're still alive.
        if elapsed / 15 > warned_secs / 15 {
            println!("    still waiting... ({elapsed}s elapsed)");
            warned_secs = elapsed;
        }
    }
}
