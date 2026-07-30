use super::*;

pub(super) fn execute(paths: &Paths, port: u16, build_only: bool) -> Result<()> {
    let version = fs::read_to_string(paths.root.join(".version"))
        .unwrap_or_default()
        .trim()
        .to_string();
    println!("==> local v{version}");

    compile_wasm(paths, true)?;
    let hash = file_sha256(&paths.wasm_input)?;
    let build = &hash[..10];

    if paths.dist_dev.exists() {
        fs::remove_dir_all(&paths.dist_dev)?;
    }
    fs::create_dir_all(&paths.dist_dev)?;
    run_bindgen(&paths.wasm_input, &paths.dist_dev, "sow_client")?;
    copy_shell(paths, &paths.dist_dev)?;

    let template = fs::read_to_string(paths.shell.join("index.html.template"))?;
    let html = template
        .replace("__VERSION__", &version)
        .replace("__JS_FILE__", "sow_client.js")
        .replace("__WASM_FILE__", "sow_client_bg.wasm")
        .replace("__BUILD_TS__", build)
        .replace("__ASSETS_UI_BASE__", "/assets/cdn/ui/");
    let index = paths.dist_dev.join("index.html");
    let mut lines: Vec<String> = html.lines().map(String::from).collect();
    for line in &mut lines {
        if line.contains("PORTAL_SDK_SLOT") {
            line.clear();
        } else if line.contains("PORTAL_BOOT_SLOT") {
            *line = "        window.SOW_PORTAL = \"site\"; window.SOW_WS_URL = \"wss://shadowsofwar.io/ws/\"; window.SOW_MAPS_URL = \"https://shadowsofwar.io/maps\"; window.SOW_ASSETS_URL = \"/assets\";".to_string();
        }
    }

    let loader =
        fs::read_to_string(paths.shell.join("loader.js"))?.replace("</script>", "<\\/script>");
    let mut html = lines.join("\n");
    let marker = "/* __INLINE_LOADER_JS__ */";
    if html.contains(marker) {
        html = html.replacen(marker, &loader, 1);
    } else if html.contains(r#"<script src="./loader.js"></script>"#) {
        html = html.replacen(
            r#"<script src="./loader.js"></script>"#,
            &format!("<script>{loader}</script>"),
            1,
        );
    }
    fs::write(index, html)?;

    println!("Local ready: {}", paths.dist_dev.display());
    if build_only {
        return Ok(());
    }
    serve(&paths.dist_dev, port)
}

fn serve(dir: &Path, port: u16) -> Result<()> {
    use axum::Router;
    use tower_http::services::ServeDir;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()?;
    runtime.block_on(async {
        let app = Router::new().nest_service("/", ServeDir::new(dir));
        let address = format!("127.0.0.1:{port}");
        let listener = tokio::net::TcpListener::bind(&address).await?;
        println!("  → http://{address}/");
        axum::serve(listener, app).await?;
        Ok::<_, anyhow::Error>(())
    })
}
