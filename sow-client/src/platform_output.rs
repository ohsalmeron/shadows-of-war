use egui::PlatformOutput;

/// Drain egui platform commands (hyperlinks, clipboard) that egui-winit would handle for us.
pub fn handle_egui_platform_output(platform_output: &PlatformOutput) {
    for cmd in &platform_output.commands {
        match cmd {
            egui::OutputCommand::OpenUrl(open) => open_url(&open.url, open.new_tab),
            egui::OutputCommand::CopyText(text) => copy_text(text),
            egui::OutputCommand::CopyImage(_) => {}
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn copy_text(text: &str) {
    use wasm_bindgen::JsValue;

    let Some(window) = web_sys::window() else {
        return;
    };
    let clipboard = window.navigator().clipboard();
    // `navigator.clipboard` is undefined outside secure contexts (plain http);
    // calling into it would throw, so drop to the legacy path up front.
    let clipboard_val: &JsValue = clipboard.as_ref();
    if clipboard_val.is_undefined() {
        legacy_copy(text);
        return;
    }
    let fallback_text = text.to_string();
    // Async API can still reject (iframe without clipboard-write permission).
    let on_reject = wasm_bindgen::closure::Closure::wrap(Box::new(move |err: JsValue| {
        log::warn!("navigator.clipboard.writeText rejected ({err:?}); trying execCommand fallback");
        legacy_copy(&fallback_text);
    }) as Box<dyn FnMut(JsValue)>);
    let _ = clipboard.write_text(text).catch(&on_reject);
    // Leak the closure: it must outlive this frame for the promise to call it.
    on_reject.forget();
}

/// Legacy hidden-input + `document.execCommand("copy")` path.
#[cfg(target_arch = "wasm32")]
fn legacy_copy(text: &str) {
    use wasm_bindgen::JsCast;

    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(body) = document.body() else {
        return;
    };
    let Some(input) = document
        .create_element("input")
        .ok()
        .and_then(|el| el.dyn_into::<web_sys::HtmlInputElement>().ok())
    else {
        return;
    };
    input.set_value(text);
    let _ = body.append_child(&input);
    input.select();
    let copied = document
        .dyn_ref::<web_sys::HtmlDocument>()
        .and_then(|doc| doc.exec_command("copy").ok())
        .unwrap_or(false);
    if !copied {
        log::warn!("execCommand copy fallback failed — clipboard unavailable");
    }
    input.remove();
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "ios")))]
fn copy_text(text: &str) {
    let res = arboard::Clipboard::new().and_then(|mut c| c.set_text(text.to_string()));
    if let Err(e) = res {
        log::warn!("clipboard copy failed: {e}");
    }
}

fn open_url(url: &str, new_tab: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = new_tab;
        if let Some(window) = web_sys::window() {
            // Always new tab on web — game shell must not navigate away.
            match window.open_with_url_and_target(url, "_blank") {
                Ok(None) => log::warn!("popup blocked opening url {url}"),
                Err(e) => log::warn!("failed to open url {url}: {e:?}"),
                Ok(Some(_)) => {}
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
