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
    let mut c = Command::new(cmd);
    c.args(args);
    if let Some(dir) = cwd {
        c.current_dir(dir);
    }
    for (k, v) in env {
        c.env(k, v);
    }
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = c.spawn().with_context(|| format!("spawn {cmd}"))?;
    let out = child.wait_with_output().with_context(|| format!("wait {cmd}"))?;
    
    let stdout_str = String::from_utf8_lossy(&out.stdout);
    let stderr_str = String::from_utf8_lossy(&out.stderr);
    
    let mut has_warnings = false;
    let mut has_errors = false;
    
    for line in stdout_str.lines().chain(stderr_str.lines()) {
        let l_lower = line.to_lowercase();
        if l_lower.contains("error:") || l_lower.contains("error ") {
            has_errors = true;
        }
        if l_lower.contains("warning:") || l_lower.contains("warning ") || l_lower.contains("warn:") {
            has_warnings = true;
        }
    }
    
    if !out.status.success() {
        print_cmd_executed(cmd, args, env);
        if !stdout_str.is_empty() {
            println!("{}", stdout_str);
        }
        if !stderr_str.is_empty() {
            eprintln!("{}", stderr_str);
        }
        anyhow::bail!("{cmd} failed ({})", out.status);
    }
    
    if has_warnings || has_errors {
        print_cmd_executed(cmd, args, env);
        println!("⚠️  Warnings/Errors detected during execution of {cmd}:");
        if !stdout_str.is_empty() {
            println!("{}", stdout_str);
        }
        if !stderr_str.is_empty() {
            eprintln!("{}", stderr_str);
        }
    } else if cmd == "cargo" {
        for line in stderr_str.lines() {
            if line.contains("Finished") || line.contains("Compiling") {
                println!("{line}");
            }
        }
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
        anyhow::bail!(
            "{cmd} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
