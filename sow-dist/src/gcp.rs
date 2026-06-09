use crate::process;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct GcpConfig {
    pub project: String,
    pub zone: String,
    pub instance: String,
}

impl GcpConfig {
    pub fn ssh_prefix(&self) -> Vec<String> {
        vec![
            "compute".into(),
            "ssh".into(),
            self.instance.clone(),
            format!("--project={}", self.project),
            format!("--zone={}", self.zone),
            "--quiet".into(),
        ]
    }

    pub fn rsync_shell(&self, cache_dir: &Path) -> Result<String> {
        let script = cache_dir.join(".sow-gcloud-rsync-sh");
        if let Some(parent) = script.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = format!(
            "#!/bin/bash\nset -euo pipefail\nhost=\"$1\"\nshift\nexec gcloud compute ssh \"$host\" \
             --project={} --zone={} --quiet -- \"$@\"\n",
            self.project, self.zone
        );
        fs::write(&script, content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755))?;
        }
        Ok(script.to_string_lossy().into())
    }

    pub fn rsync_with_opts(
        &self,
        cache_dir: &Path,
        local: &str,
        remote_path: &str,
        opts: &[&str],
    ) -> Result<()> {
        let remote = format!("{}:{}", self.instance, remote_path);
        let shell = self.rsync_shell(cache_dir)?;
        let mut args: Vec<&str> = vec!["-e", &shell];
        args.extend_from_slice(opts);
        args.push(local);
        args.push(&remote);
        process::run("rsync", &args, None)
    }

    pub fn rsync_dir_with_opts(
        &self,
        cache_dir: &Path,
        local_dir: &str,
        remote_path: &str,
        opts: &[&str],
    ) -> Result<()> {
        let local = format!("{}/", local_dir.trim_end_matches('/'));
        let remote = format!("{}:{}/", self.instance, remote_path.trim_end_matches('/'));
        let shell = self.rsync_shell(cache_dir)?;
        let mut args: Vec<&str> = vec!["-e", &shell];
        args.extend_from_slice(opts);
        args.push(&local);
        args.push(&remote);
        process::run("rsync", &args, None)
    }

    pub fn rsync(&self, cache_dir: &Path, local: &str, remote_path: &str) -> Result<()> {
        self.rsync_with_opts(cache_dir, local, remote_path, &["-avz"])
    }

    pub fn run_remote(&self, script: &str) -> Result<()> {
        let mut args: Vec<String> = self.ssh_prefix();
        args.push("--command".into());
        args.push(script.into());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        process::run("gcloud", &refs, None)
    }

    pub fn remote_output(&self, script: &str) -> Result<String> {
        let mut args: Vec<String> = self.ssh_prefix();
        args.push("--command".into());
        args.push(script.into());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        process::output("gcloud", &refs)
    }

    pub fn ssh_ready(&self) -> bool {
        use std::process::{Command, Stdio};
        let mut args = self.ssh_prefix();
        args.push("--command".into());
        args.push("echo ok".into());
        Command::new("gcloud")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    pub fn remote_home(&self, cache_file: &Path) -> Result<String> {
        if let Ok(h) = fs::read_to_string(cache_file) {
            let h = h.trim().to_string();
            if !h.is_empty() {
                return Ok(h);
            }
        }
        let home = self.remote_output("echo $HOME")?.trim().to_string();
        if home.is_empty() {
            bail!("could not resolve remote $HOME via gcloud compute ssh");
        }
        if let Some(parent) = cache_file.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(cache_file, format!("{home}\n"))?;
        Ok(home)
    }
}

pub fn enable_os_login(project: &str) -> Result<()> {
    println!("==> Enabling OS Login on project {project}");
    process::run(
        "gcloud",
        &[
            "compute",
            "project-info",
            "add-metadata",
            &format!("--project={project}"),
            "--metadata",
            "enable-oslogin=TRUE",
        ],
        None,
    )?;
    let account = process::output(
        "gcloud",
        &[
            "auth",
            "list",
            "--filter=status:ACTIVE",
            "--format=value(account)",
        ],
    )?;
    if !account.is_empty() {
        println!("==> Granting OS Admin Login to {account}");
        process::run(
            "gcloud",
            &[
                "projects",
                "add-iam-policy-binding",
                project,
                &format!("--member=user:{account}"),
                "--role=roles/compute.osAdminLogin",
                "--condition=None",
            ],
            None,
        )?;
    }
    Ok(())
}

pub fn delete_instance(project: &str, zone: &str, name: &str) -> Result<()> {
    println!("==> Deleting VM {name} ({zone})");
    gcloud_allow_missing(&[
        "compute",
        "instances",
        "delete",
        name,
        &format!("--project={project}"),
        &format!("--zone={zone}"),
        "--quiet",
    ])
}

pub fn release_static_ip(project: &str, region: &str, name: &str) -> Result<()> {
    println!("==> Releasing static IP {name}");
    gcloud_allow_missing(&[
        "compute",
        "addresses",
        "delete",
        name,
        &format!("--project={project}"),
        &format!("--region={region}"),
        "--quiet",
    ])
}

fn gcloud_allow_missing(args: &[&str]) -> Result<()> {
    use std::process::Command;
    let out = Command::new("gcloud")
        .args(args)
        .output()
        .context("spawn gcloud")?;
    if out.status.success() {
        return Ok(());
    }
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if combined.contains("was not found") || combined.contains("NOT_FOUND") {
        println!("    (already gone)");
        return Ok(());
    }
    eprintln!("{combined}");
    bail!("gcloud failed ({})", out.status);
}

pub fn create_fedora_vm(project: &str, zone: &str, name: &str, static_ip: &str) -> Result<()> {
    println!("==> Creating Fedora VM {name} with static IP {static_ip}");
    process::run(
        "gcloud",
        &[
            "compute",
            "instances",
            "create",
            name,
            &format!("--project={project}"),
            &format!("--zone={zone}"),
            "--machine-type=e2-small",
            "--boot-disk-size=30GB",
            "--image-family=fedora-cloud-44-x86-64",
            "--image-project=fedora-cloud",
            &format!("--address={static_ip}"),
            "--tags=http-server,https-server",
            "--metadata=enable-oslogin=TRUE",
        ],
        None,
    )?;
    Ok(())
}

pub fn scp_to_instance(gcp: &GcpConfig, local: &Path, remote_path: &str) -> Result<()> {
    process::run(
        "gcloud",
        &[
            "compute",
            "scp",
            &local.to_string_lossy(),
            &format!("{}:{}", gcp.instance, remote_path),
            &format!("--project={}", gcp.project),
            &format!("--zone={}", gcp.zone),
            "--quiet",
        ],
        None,
    )
    .with_context(|| format!("scp {} → {}", local.display(), remote_path))
}
