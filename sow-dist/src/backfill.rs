use super::*;

const BUILD_HOST: &str = "freebsd";
const BUILD_ROOT: &str = "/home/bizkit/shadows-of-war";
const DEFAULT_BACKFILL_HOSTS: &str = "sow-backfill1,sow-backfill2,ionos,clouding";

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn get_backfill_hosts() -> Vec<String> {
    env::var("SOW_BACKFILL_HOSTS")
        .or_else(|_| env::var("SOW_BACKFILL_HOST"))
        .unwrap_or_else(|_| DEFAULT_BACKFILL_HOSTS.to_string())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub(super) fn execute(
    paths: &Paths,
    build_only: bool,
    min_fill: usize,
    max_fill: usize,
    url: &str,
) -> Result<()> {
    let build_host = env_or("SOW_BUILD_HOST", BUILD_HOST);
    let build_root = env_or("SOW_BUILD_ROOT", BUILD_ROOT);
    let hosts = get_backfill_hosts();

    if hosts.is_empty() {
        bail!("No backfill hosts. Set SOW_BACKFILL_HOSTS env or use defaults.");
    }

    println!("==> Backfill build on {build_host}");

    let build_check = format!(
        "test -d {} && command -v cargo >/dev/null",
        shell_quote(&build_root)
    );
    run("ssh", &[&build_host, &build_check], None).context("FreeBSD build VM not ready")?;

    let source = format!("{}/", paths.root.display());
    let destination = format!("{build_host}:{build_root}/");
    run(
        "rsync",
        &[
            "-azc",
            "--delete",
            "--exclude=.git",
            "--exclude=dist",
            "--exclude=target",
            "--exclude=sow-dist/.env",
            &source,
            &destination,
        ],
        Some(&paths.root),
    )?;

    let root = shell_quote(&build_root);
    let command = format!(
        "set -eu; cd {root}; cargo build --locked --release -p sow-backfill --manifest-path sow-backfill/Cargo.toml"
    );
    run("ssh", &[&build_host, &command], None)?;

    let remote_bin = format!("{build_root}/sow-backfill/target/release/sow-backfill");
    let rc_d_src = paths.root.join("sow-dist/deploy/freebsd/rc.d/sow_backfill");

    if build_only {
        println!("✅ Build done: {build_host}:{remote_bin}");
        return Ok(());
    }

    for host in &hosts {
        println!("==> Deploy to {host}");

        run(
            "ssh",
            &[
                host,
                "sudo install -d -o root -g wheel -m 0755 /usr/local/libexec /usr/local/share/sow/maps/world /usr/local/etc",
            ],
            None,
        )?;

        let rc_d_remote = "/tmp/sow_backfill.rc";
        run(
            "scp",
            &[
                rc_d_src.to_str().context("rc.d path")?,
                &format!("{host}:{rc_d_remote}"),
            ],
            None,
        )?;
        run(
            "ssh",
            &[
                host,
                &format!(
                    "sudo install -o root -g wheel -m 0755 {rc_d_remote} /usr/local/etc/rc.d/sow_backfill && rm {rc_d_remote}"
                ),
            ],
            None,
        )?;

        run(
            "scp",
            &[
                "-3",
                &format!("{build_host}:{remote_bin}"),
                &format!("{host}:/tmp/sow-backfill"),
            ],
            None,
        )?;
        run(
            "ssh",
            &[
                host,
                "sudo install -o root -g wheel -m 0555 /tmp/sow-backfill /usr/local/libexec/sow-backfill && rm /tmp/sow-backfill",
            ],
            None,
        )?;

        let map_src = paths.root.join("assets/maps/world/map.bin");
        if map_src.is_file() {
            run(
                "scp",
                &[
                    map_src.to_str().context("map path")?,
                    &format!("{host}:/tmp/map.bin"),
                ],
                None,
            )?;
            run(
                "ssh",
                &[
                    host,
                    "sudo install -o root -g wheel -m 0644 /tmp/map.bin /usr/local/share/sow/maps/world/map.bin && rm /tmp/map.bin",
                ],
                None,
            )?;
        }

        let args = format!(
            "--min-fill {min_fill} --max-fill {max_fill} --url {url} --maps-root /usr/local/share/sow/maps --build-version 0.1.2"
        );
        let conf = format!("sow_backfill_enable=\"YES\"\nsow_backfill_args=\"{args}\"\n");
        run(
            "ssh",
            &[
                host,
                &format!(
                    "sudo tee /usr/local/etc/sow-backfill.conf >/dev/null <<'CONF'\n{conf}CONF"
                ),
            ],
            None,
        )?;

        run(
            "ssh",
            &[
                host,
                "sudo service sow_backfill stop || true; sudo pkill -9 -f sow-backfill || true; sudo pkill -9 daemon || true; sudo rm -f /var/run/sow_backfill.pid /var/run/sow_backfill.child.pid && sudo sysrc -q sow_backfill_enable=YES && sudo service sow_backfill start && sleep 2 && sudo service sow_backfill status",
            ],
            None,
        )?;

        println!("✅ {host} running");
    }

    println!("✅ All backfill hosts done");
    Ok(())
}
