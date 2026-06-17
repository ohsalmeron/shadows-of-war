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
