use egui::PlatformOutput;

/// Drain egui platform commands (hyperlinks, etc.) that egui-winit would handle for us.
pub fn handle_egui_platform_output(platform_output: &PlatformOutput) {
    for cmd in &platform_output.commands {
        if let egui::OutputCommand::OpenUrl(open) = cmd {
            open_url(&open.url, open.new_tab);
        }
    }
}

fn open_url(url: &str, new_tab: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = new_tab;
        if let Some(window) = web_sys::window() {
            let target = if new_tab { "_blank" } else { "_self" };
            if let Err(e) = window.open_with_url_and_target(url, target) {
                log::warn!("failed to open url {url}: {e:?}");
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = new_tab;
        if let Err(e) = open::that(url) {
            log::warn!("failed to open url {url}: {e}");
        }
    }
}
