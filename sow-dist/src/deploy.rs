use crate::paths::{Paths, PROD_HOST, PROD_USER};
use crate::process;
use anyhow::{bail, Context, Result};
use std::path::Path;

pub fn deploy_play(paths: &Paths) -> Result<()> {
    let u = PROD_USER;
    let h = PROD_HOST;
    let remote = format!("{u}@{h}");
    println!("==> Deploying play → play.shadowsofwar.io + marketing site → shadowsofwar.io");
    ensure_dirs(
        &remote,
        &[
            "/var/www/play.shadowsofwar.io/html",
            "/var/www/shadowsofwar.io/html",
            "/home/bizkit/shadowsofwar/assets/maps",
        ],
    )?;
    let (sb, rb) = build_server_binaries(paths)?;
    let dist = format!("{}/", paths.dist_play.display());
    let play_html = format!("{remote}:/var/www/play.shadowsofwar.io/html/");
    let main_html = format!("{remote}:/var/www/shadowsofwar.io/html/");
    let site = format!("{}/", paths.site_web.display());
    let maps = format!("{}/", paths.assets_static.join("maps").display());
    std::thread::scope(|s| -> Result<()> {
        let a = s.spawn(|| {
            process::run(
                "rsync",
                &["-avzL", "--delete", "--exclude=*.bin", &dist, &play_html],
                None,
            )
        });
        let b = s.spawn(|| process::run("rsync", &["-avz", &site, &main_html], None));
        let d = s.spawn(|| {
            process::run(
                "rsync",
                &["-avz", &sb, &format!("{remote}:/home/bizkit/shadowsofwar/sow-server")],
                None,
            )
        });
        let e = s.spawn(|| {
            process::run(
                "rsync",
                &["-avz", &rb, &format!("{remote}:/home/bizkit/shadowsofwar/sow-relay")],
                None,
            )
        });
        let f = s.spawn(|| {
            process::run(
                "rsync",
                &[
                    "-avz",
                    "--exclude=map.bin",
                    "--exclude=mini_map.bin",
                    "--exclude=manifest.json",
                    "--exclude=maps.json",
                    &maps,
                    &format!("{remote}:/home/bizkit/shadowsofwar/assets/maps/"),
                ],
                None,
            )
        });
        a.join().unwrap()?;
        b.join().unwrap()?;
        d.join().unwrap()?;
        e.join().unwrap()?;
        f.join().unwrap()?;
        Ok(())
    })?;
    sync_play_nginx(paths, &remote)?;
    sync_main_nginx(paths, &remote)?;
    process::run(
        "ssh",
        &[&remote, "sudo systemctl enable --now sow-redis 2>/dev/null; sudo systemctl restart sow-server"],
        None,
    )?;
    verify_play_host("https://play.shadowsofwar.io/")?;
    verify_marketing_embed("https://shadowsofwar.io/")?;
    verify_sitemap("https://shadowsofwar.io/sitemap.xml")?;
    Ok(())
}

pub fn deploy_ptr(paths: &Paths) -> Result<()> {
    let u = PROD_USER;
    let h = "shadowsofwar.io";
    let remote = format!("{u}@{h}");
    println!("==> Deploying ptr → ptr.shadowsofwar.io");
    ensure_dirs(&remote, &["/var/www/ptr.shadowsofwar.io/html"])?;
    let (sb, rb) = build_server_binaries(paths)?;
    let dist = format!("{}/", paths.dist_ptr.display());
    let html = format!("{remote}:/var/www/ptr.shadowsofwar.io/html/");
    process::run("ssh", &[&remote, "mkdir -p /home/bizkit/shadowsofwar-ptr"], None)?;
    let maps = format!("{}/", paths.assets_static.join("maps").display());
    std::thread::scope(|s| -> Result<()> {
        let a = s.spawn(|| process::run("rsync", &["-avzL", "--delete", "--exclude=*.bin", &dist, &html], None));
        let b = s.spawn(|| {
            process::run(
                "rsync",
                &["-avz", &sb, &format!("{remote}:/home/bizkit/shadowsofwar-ptr/sow-server")],
                None,
            )
        });
        let c = s.spawn(|| {
            process::run(
                "rsync",
                &["-avz", &rb, &format!("{remote}:/home/bizkit/shadowsofwar-ptr/sow-relay")],
                None,
            )
        });
        let d = s.spawn(|| {
            process::run(
                "rsync",
                &[
                    "-avz",
                    "--exclude=map.bin",
                    "--exclude=mini_map.bin",
                    "--exclude=manifest.json",
                    "--exclude=maps.json",
                    &maps,
                    &format!("{remote}:/home/bizkit/shadowsofwar-ptr/assets/maps/"),
                ],
                None,
            )
        });
        a.join().unwrap()?;
        b.join().unwrap()?;
        c.join().unwrap()?;
        d.join().unwrap()?;
        Ok(())
    })?;
    sync_ptr_nginx(paths, &remote)?;
    install_ptr_systemd(&remote)?;
    verify_play_host("https://ptr.shadowsofwar.io/")?;
    Ok(())
}

fn ensure_dirs(remote: &str, dirs: &[&str]) -> Result<()> {
    let list = dirs.join(" ");
    let user = remote.split('@').next().unwrap_or("bizkit");
    process::run(
        "ssh",
        &[
            remote,
            &format!("sudo mkdir -p {list} && sudo chown -R {user}:{user} {list}"),
        ],
        None,
    )?;
    Ok(())
}

fn build_server_binaries(paths: &Paths) -> Result<(String, String)> {
    let musl_sb = paths
        .cargo_target
        .join("x86_64-unknown-linux-musl/release/sow-server");
    let musl_rb = paths
        .cargo_target
        .join("x86_64-unknown-linux-musl/release/sow-relay");
    let musl_ok = process::run(
        "cargo",
        &[
            "build",
            "--release",
            "-p",
            "sow-server",
            "--target",
            "x86_64-unknown-linux-musl",
        ],
        Some(&paths.root),
    )
    .is_ok()
        && process::run(
            "cargo",
            &[
                "build",
                "--release",
                "-p",
                "sow-relay",
                "--target",
                "x86_64-unknown-linux-musl",
            ],
            Some(&paths.root),
        )
        .is_ok()
        && musl_sb.is_file()
        && musl_rb.is_file();
    if musl_ok {
        return Ok((
            musl_sb.to_string_lossy().into_owned(),
            musl_rb.to_string_lossy().into_owned(),
        ));
    }
    process::run(
        "cargo",
        &["build", "--release", "-p", "sow-server", "--target", "x86_64-unknown-linux-gnu"],
        Some(&paths.root),
    )?;
    process::run(
        "cargo",
        &["build", "--release", "-p", "sow-relay", "--target", "x86_64-unknown-linux-gnu"],
        Some(&paths.root),
    )?;
    Ok((
        paths
            .cargo_target
            .join("x86_64-unknown-linux-gnu/release/sow-server")
            .to_string_lossy()
            .into_owned(),
        paths
            .cargo_target
            .join("x86_64-unknown-linux-gnu/release/sow-relay")
            .to_string_lossy()
            .into_owned(),
    ))
}

fn sync_play_nginx(paths: &Paths, remote: &str) -> Result<()> {
    let conf = paths.deploy_nginx.join("play.conf");
    scp_nginx(remote, &conf)?;
    Ok(())
}

fn sync_main_nginx(paths: &Paths, remote: &str) -> Result<()> {
    let conf = paths.deploy_nginx.join("shadowsofwar.io.conf");
    scp_nginx_site(remote, &conf, "shadowsofwar.io")?;
    Ok(())
}

fn sync_ptr_nginx(paths: &Paths, remote: &str) -> Result<()> {
    let conf = paths.deploy_nginx.join("ptr.conf");
    scp_nginx_site(remote, &conf, "ptr.shadowsofwar.io")?;
    Ok(())
}

fn scp_nginx(remote: &str, local: &Path) -> Result<()> {
    scp_nginx_site(remote, local, "play.shadowsofwar.io")
}

fn scp_nginx_site(remote: &str, local: &Path, site: &str) -> Result<()> {
    let local_s = local.to_string_lossy();
    let hash = process::output("md5sum", &[local_s.as_ref()])?;
    let hash = hash.split_whitespace().next().context("md5")?;
    process::run(
        "scp",
        &[local_s.as_ref(), &format!("{remote}:/tmp/sow-nginx.conf")],
        None,
    )?;
    let nginx_site = format!("/etc/nginx/sites-available/{site}");
    let script = format!(
        "set -euo pipefail; export PATH=/usr/sbin:/usr/bin:/sbin:/bin:$PATH; \
         NGINX_SITE={nginx_site:?}; LOCAL_HASH={hash:?}; \
         remote_hash=; [[ -f \"$NGINX_SITE\" ]] && remote_hash=$(md5sum \"$NGINX_SITE\" | awk '{{print $1}}'); \
         if [[ \"$remote_hash\" != \"$LOCAL_HASH\" ]]; then \
           sudo cp /tmp/sow-nginx.conf \"$NGINX_SITE\"; \
           sudo ln -sf \"$NGINX_SITE\" /etc/nginx/sites-enabled/$(basename \"$NGINX_SITE\"); \
           sudo nginx -t && sudo systemctl reload nginx; \
         else echo '✅ Nginx config unchanged.'; fi"
    );
    process::run("ssh", &[remote, &script], None)?;
    Ok(())
}

fn install_ptr_systemd(remote: &str) -> Result<()> {
    let unit = r#"[Unit]
Description=Shadows of War Server (PTR)
After=network.target
[Service]
KillMode=process
Type=simple
User=bizkit
WorkingDirectory=/home/bizkit/shadowsofwar-ptr
ExecStart=/home/bizkit/shadowsofwar-ptr/sow-server
Restart=always
RestartSec=3
Environment=RUST_LOG=info
Environment=SOW_WS_LISTEN=0.0.0.0:25575
Environment=SOW_MAPS_HTTP_LISTEN=0.0.0.0:25576
[Install]
WantedBy=multi-user.target
"#;
    let path = std::env::temp_dir().join("sow-server-ptr.service");
    std::fs::write(&path, unit)?;
    process::run(
        "scp",
        &[
            path.to_string_lossy().as_ref(),
            &format!("{remote}:/tmp/sow-server-ptr.service"),
        ],
        None,
    )?;
    process::run(
        "ssh",
        &[
            remote,
            "sudo mv /tmp/sow-server-ptr.service /etc/systemd/system/sow-server-ptr.service && \
             sudo systemctl daemon-reload && sudo systemctl enable --now sow-server-ptr && \
             sudo systemctl restart sow-server-ptr",
        ],
        None,
    )?;
    Ok(())
}

pub fn verify_play_host(play_url: &str) -> Result<()> {
    println!("==> Verifying {play_url}");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let manifest_url = format!("{play_url}game-manifest.json");
    let manifest = client.get(&manifest_url).send().context("manifest")?;
    if !manifest.status().is_success() {
        bail!("game-manifest.json failed");
    }
    let body = manifest.text()?;
    if !body.contains("sow_client") {
        bail!("invalid game-manifest.json");
    }
    let html = client.get(play_url).send().context("index")?.text()?;
    if !html.contains("web-loader") || !html.contains("hideWebLoader") {
        bail!("index.html missing loader");
    }
    if !html.contains("sow_client") && !body.contains("_bg.wasm") {
        bail!("missing sow_client bundle reference");
    }
    println!("✅ Play host OK");
    Ok(())
}

pub fn verify_marketing_embed(home_url: &str) -> Result<()> {
    println!("==> Verifying marketing embed at {home_url}");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let html = client.get(home_url).send().context("index")?.text()?;
    if !html.contains("sow-game-stage") || !html.contains("game-embed.js") {
        bail!("marketing index missing iframe embed");
    }
    if !html.contains("iframe") {
        bail!("marketing index missing iframe player");
    }
    if html.contains("sow_client_") {
        bail!("marketing index must not reference WASM bundle before Play click");
    }
    let embed = client
        .get(format!("{home_url}game-embed.js"))
        .send()
        .context("game-embed.js")?;
    if !embed.status().is_success() {
        bail!("game-embed.js missing on marketing host");
    }
    println!("✅ Marketing embed OK");
    Ok(())
}

pub fn verify_sitemap(sitemap_url: &str) -> Result<()> {
    println!("==> Verifying sitemap at {sitemap_url}");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let res = client
        .get(sitemap_url)
        .header("User-Agent", "Googlebot")
        .send()
        .context("sitemap fetch")?;
    if !res.status().is_success() {
        bail!("sitemap.xml returned {}", res.status());
    }
    let ctype = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    if !ctype.contains("xml") {
        bail!("sitemap Content-Type must be xml, got: {ctype}");
    }
    let body = res.text()?;
    if !body.contains("<urlset") || !body.contains("</urlset>") {
        bail!("sitemap.xml is not a valid urlset document");
    }
    let url_count = body.matches("<loc>").count();
    if url_count < 3 {
        bail!("sitemap.xml expected at least 3 URLs, found {url_count}");
    }
    for path in ["/", "/privacy", "/terms"] {
        let loc = format!("https://shadowsofwar.io{path}");
        if !body.contains(&loc) {
            bail!("sitemap.xml missing {loc}");
        }
    }
    println!("✅ Sitemap OK ({url_count} URLs, Content-Type: {ctype})");
    Ok(())
}
