use crate::paths::Paths;
use anyhow::{Context, Result};
use std::fs;

pub fn load(paths: &Paths) -> Result<String> {
    Ok(fs::read_to_string(&paths.version_file)
        .unwrap_or_else(|_| "0.1.0\n".into())
        .trim()
        .to_string())
}

pub fn increment(paths: &Paths) -> Result<String> {
    let cur = load(paths)?;
    let patch = cur
        .rsplit('.')
        .next()
        .and_then(|p| p.parse::<u32>().ok())
        .unwrap_or(0);
    let next = format!("0.1.{}", patch + 1);
    fs::write(&paths.version_file, format!("{next}\n")).context("write .version")?;
    println!("Version → {next}");
    Ok(next)
}
