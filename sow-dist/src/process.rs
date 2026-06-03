use anyhow::{Context, Result};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

pub fn run(cmd: &str, args: &[&str], cwd: Option<&Path>) -> Result<()> {
    let mut c = Command::new(cmd);
    c.args(args);
    if let Some(dir) = cwd {
        c.current_dir(dir);
    }
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().with_context(|| format!("spawn {cmd}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    if let Some(out) = stdout {
        for l in BufReader::new(out).lines().map_while(Result::ok) {
            println!("{l}");
        }
    }
    if let Some(err) = stderr {
        for l in BufReader::new(err).lines().map_while(Result::ok) {
            eprintln!("{l}");
        }
    }
    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("{cmd} failed ({status})");
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
        anyhow::bail!(
            "{cmd} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
