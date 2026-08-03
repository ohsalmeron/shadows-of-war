use super::*;

const RELAY_HOST: &str = "relay";
const RELAY_ROOT: &str = "/home/azureuser/shadows-of-war";

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

pub(super) fn execute(paths: &Paths) -> Result<()> {
    let host = env_or("SOW_RELAY_HOST", RELAY_HOST);
    let root = env_or("SOW_RELAY_ROOT", RELAY_ROOT);

    let build_check = format!(
        "export PATH=$HOME/.cargo/bin:$PATH; test -d {} && test -f {}/Cargo.toml && command -v cargo && test -f {}/fstack-bridge/build.rs",
        shell_quote(&root),
        shell_quote(&root),
        shell_quote(&root),
    );
    run("ssh", &[&host, &build_check], None).context("relay VM not ready")?;

    let source = format!("{}/", paths.root.display());
    let destination = format!("{host}:{root}/");
    run(
        "rsync",
        &[
            "-azc",
            "--delete",
            "--exclude=.git",
            "--exclude=dist",
            "--exclude=target",
            "--exclude=sow-dist/.env",
            "--exclude=replays",
            &source,
            &destination,
        ],
        Some(&paths.root),
    )?;

    let build = format!(
        "set -eu; export PATH=$HOME/.cargo/bin:$PATH; cd {}; cargo build --release -p sow-relay",
        shell_quote(&root)
    );
    run("ssh", &[&host, &build], None)?;

    let drop_in = format!(
        "sudo mkdir -p /etc/systemd/system/sow-relay.service.d && \
         sudo tee /etc/systemd/system/sow-relay.service.d/override.conf >/dev/null <<'EOF'\n\
         [Service]\n\
         Environment=SOW_DB_URL={}\n\
         Environment=SOW_DB_SECRET={}\n\
         EOF\n\
         sudo systemctl daemon-reload",
        env_or("SOW_DB_URL", "http://74.208.246.177:80"),
        env_or("SOW_DB_SECRET", "sow_db_dev_secret_123_change_me_in_prod"),
    );
    run("ssh", &[&host, &drop_in], None)?;

    run(
        "ssh",
        &[
            &host,
            "sudo systemctl restart sow-relay && sleep 2 && sudo systemctl is-active sow-relay",
        ],
        None,
    )?;

    let verify = format!(
        "curl -fsS --max-time 5 http://127.0.0.1:8080/internal/lobbies >/dev/null && echo mgmt-ok && ps -o rss= -p $(pgrep -f sow-relay | head -1) && systemctl show sow-relay -p NRestarts"
    );
    run("ssh", &[&host, &verify], None)?;

    println!("✅ relay deployed to {host}");
    Ok(())
}
