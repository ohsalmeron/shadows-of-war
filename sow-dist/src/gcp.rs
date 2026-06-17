use crate::process;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct GcpConfig {
    pub project: String,
    pub zone: String,
    pub instance: String,
}

/// Remote directory sync options (replaces rsync flags).
#[derive(Clone, Debug, Default)]
pub struct SyncOpts {
    /// Delete remote children before upload (rsync `--delete`), except `preserve_basenames`.
    pub mirror: bool,
    /// Top-level basenames to keep when `mirror` (e.g. `*.bin` on play shell).
    pub preserve_basenames: Vec<String>,
    /// File basenames omitted from the upload archive.
    pub exclude_basenames: Vec<String>,
}

impl GcpConfig {
    fn ssh_base_args(&self) -> Vec<String> {
        vec![
            "compute".into(),
            "ssh".into(),
            self.instance.clone(),
            format!("--project={}", self.project),
            format!("--zone={}", self.zone),
            "--tunnel-through-iap".into(),
            "--quiet".into(),
            "--verbosity=error".into(),
        ]
    }

    pub fn ssh_prefix(&self) -> Vec<String> {
        self.ssh_base_args()
    }

    fn scp_base_args(&self) -> Vec<String> {
        vec![
            "compute".into(),
            "scp".into(),
            format!("--project={}", self.project),
            format!("--zone={}", self.zone),
            "--tunnel-through-iap".into(),
            "--quiet".into(),
            "--verbosity=error".into(),
        ]
    }

    pub fn run_remote(&self, script: &str) -> Result<()> {
        let mut args = self.ssh_prefix();
        args.push("--command".into());
        args.push(script.into());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut attempts = 0;
        loop {
            match process::run("gcloud", &refs, None) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    attempts += 1;
                    if attempts >= 3 {
                        return Err(e);
                    }
                    println!(
                        "⚠️ ssh warning: connection blip, retrying in 5s (attempt {attempts}/3)..."
                    );
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            }
        }
    }

    pub fn remote_output(&self, script: &str) -> Result<String> {
        let mut args = self.ssh_prefix();
        args.push("--command".into());
        args.push(script.into());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut attempts = 0;
        loop {
            match process::output("gcloud", &refs) {
                Ok(out) => return Ok(out),
                Err(e) => {
                    attempts += 1;
                    if attempts >= 3 {
                        return Err(e);
                    }
                    println!(
                        "⚠️ ssh warning: connection blip, retrying in 5s (attempt {attempts}/3)..."
                    );
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            }
        }
    }

    pub fn ssh_ready(&self) -> bool {
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
        if let Ok(h) = std::fs::read_to_string(cache_file) {
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
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(cache_file, format!("{home}\n"))?;
        Ok(home)
    }

    /// Upload a single file via `gcloud compute scp`.
    pub fn sync_file(&self, local: &Path, remote_path: &str) -> Result<()> {
        scp_to_instance(self, local, remote_path)
    }

    /// Upload a directory via tar pipe over `gcloud compute ssh` (needs `gcloud` + `tar` on host).
    pub fn sync_dir(&self, local_dir: &Path, remote_dir: &str, opts: &SyncOpts) -> Result<()> {
        if !local_dir.is_dir() {
            bail!("sync_dir: {} is not a directory", local_dir.display());
        }
        let remote = remote_dir.trim_end_matches('/');
        self.run_remote(&format!("mkdir -p {remote}"))?;
        if opts.mirror {
            self.remote_mirror_prepare(remote, opts)?;
        }
        self.tar_pipe_dir(local_dir, remote, &opts.exclude_basenames)
    }

    fn remote_mirror_prepare(&self, remote_dir: &str, opts: &SyncOpts) -> Result<()> {
        let script = if opts.preserve_basenames.is_empty() {
            format!("rm -rf {remote_dir:?}/*")
        } else {
            let mut prune = String::new();
            for pat in &opts.preserve_basenames {
                if !prune.is_empty() {
                    prune.push_str(" -o ");
                }
                prune.push_str(&format!("-name '{pat}'"));
            }
            format!(
                "find {remote_dir:?} -mindepth 1 -maxdepth 1 \\( {prune} \\) -prune -o -exec rm -rf {{}} +"
            )
        };
        self.run_remote(&script)
    }

    fn tar_pipe_dir(&self, local_dir: &Path, remote_dir: &str, excludes: &[String]) -> Result<()> {
        let local = local_dir
            .to_str()
            .with_context(|| format!("non-UTF-8 path {}", local_dir.display()))?;
        let remote = remote_dir.trim_end_matches('/');

        let mut tar = Command::new("tar");
        tar.arg("-C").arg(local).arg("-cf").arg("-");
        for ex in excludes {
            tar.arg("--exclude").arg(ex);
        }
        tar.arg(".");
        tar.stdout(Stdio::piped());
        tar.stderr(Stdio::inherit());

        let mut tar_child = tar.spawn().context("spawn tar")?;
        let tar_stdout = tar_child.stdout.take().context("tar stdout pipe")?;

        let remote_cmd = format!("tar -xf - -C {remote}");
        let mut gcloud_args = self.ssh_base_args();
        gcloud_args.push("--command".into());
        gcloud_args.push(remote_cmd);

        let refs: Vec<&str> = gcloud_args.iter().map(String::as_str).collect();
        let gcloud_status = Command::new("gcloud")
            .args(&refs)
            .stdin(tar_stdout)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("gcloud compute ssh tar extract")?;

        let tar_status = tar_child.wait().context("wait tar")?;

        if !tar_status.success() {
            bail!("tar failed ({tar_status})");
        }
        if !gcloud_status.success() {
            bail!("gcloud compute ssh tar extract failed ({gcloud_status})");
        }
        Ok(())
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
        let status = std::process::Command::new("gcloud")
            .args([
                "projects",
                "add-iam-policy-binding",
                project,
                &format!("--member=user:{account}"),
                "--role=roles/compute.osAdminLogin",
                "--condition=None",
                "--quiet",
            ])
            .status();
        match status {
            Ok(s) if s.success() => {}
            _ => {
                println!("⚠️ Warning: add-iam-policy-binding failed. This is expected if Service Usage API is disabled on the project. Proceeding anyway...");
            }
        }
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

pub fn create_debian_vm(project: &str, zone: &str, name: &str, static_ip: &str) -> Result<()> {
    println!("==> Creating Debian 13 VM {name} with static IP {static_ip}");
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
            "--image-family=debian-13",
            "--image-project=debian-cloud",
            &format!("--address={static_ip}"),
            "--tags=http-server,https-server,sow-game",
            "--metadata=enable-oslogin=TRUE",
        ],
        None,
    )?;
    Ok(())
}

pub fn scp_to_instance(gcp: &GcpConfig, local: &Path, remote_path: &str) -> Result<()> {
    let mut args = gcp.scp_base_args();
    args.push(local.to_string_lossy().into());
    args.push(format!("{}:{}", gcp.instance, remote_path));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    process::run("gcloud", &refs, None)
        .with_context(|| format!("scp {} → {}", local.display(), remote_path))
}
