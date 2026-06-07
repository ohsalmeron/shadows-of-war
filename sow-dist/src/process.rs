use anyhow::{Context, Result};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

fn echo_cmd(cmd: &str, args: &[&str]) {
    let line: Vec<String> = std::iter::once(cmd.to_string())
        .chain(args.iter().map(|a| shell_quote(a)))
        .collect();
    println!("+ {}", line.join(" "));
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
    let mut c = Command::new(cmd);
    c.args(args);
    if let Some(dir) = cwd {
        c.current_dir(dir);
    }
    for (k, v) in env {
        c.env(k, v);
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
    echo_cmd(cmd, args);
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
