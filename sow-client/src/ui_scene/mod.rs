//! UI Scene Editor runtime — the **client side** of the web authoring loop.
//!
//! The web tool (`tools/ui-editor`, served by `./sow ui`) lets you lay out a screen visually,
//! saves it as `assets/editor/ui/<scene>.json` (the single source of truth), and on Export *generates*
//! Rust into [`generated`] that calls our real theme components. This module is what the running
//! client uses to **render those generated scenes 1:1** — the browser is the fast sketchpad, the
//! compiled client is the truth.
//!
//! ## Isolation contract (same discipline as the tutorial)
//! Scene preview is **dev-gated** and reachable only when `$SOW_UI_SCENE` is set on native — it
//! can never paint over a normal play / menu session, and it adds no branch to any sim / income /
//! render hot loop. The one render gate lives in `render/frame/ui.rs`; everything else here is
//! pure egui driven by generated code. Keep it that way.

mod generated;

use std::sync::OnceLock;

/// The scene to preview this session, resolved **once** from `$SOW_UI_SCENE` on native.
/// `None` in normal play and on web — so preview is structurally impossible outside dev use.
fn preview_scene() -> Option<&'static str> {
    static SCENE: OnceLock<Option<String>> = OnceLock::new();
    SCENE
        .get_or_init(|| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                std::env::var("SOW_UI_SCENE")
                    .ok()
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
            }
            #[cfg(target_arch = "wasm32")]
            {
                None
            }
        })
        .as_deref()
}

/// `true` only when this session was launched as a scene preview (dev). The render loop checks
/// this to swap normal UI for the scene; it is the single chokepoint that keeps preview isolated.
pub fn preview_active() -> bool {
    preview_scene().is_some()
}

/// Render the active preview scene over a dim backdrop. No-op if preview is inactive. If the
/// requested scene name isn't registered, paints a visible hint rather than a confusing blank.
pub fn render_preview(ctx: &egui::Context) {
    let Some(name) = preview_scene() else {
        return;
    };

    // Dim backdrop so the scene reads as an isolated preview regardless of the client phase
    // underneath it. Background order keeps it behind the scene's own areas.
    egui::Area::new(egui::Id::new("ui_scene.preview.backdrop"))
        .order(egui::Order::Background)
        .fixed_pos(egui::Pos2::ZERO)
        .show(ctx, |ui| {
            ui.painter()
                .rect_filled(ctx.content_rect(), 0.0, egui::Color32::from_rgb(12, 14, 18));
        });

    if !generated::render(name, ctx) {
        egui::Area::new(egui::Id::new("ui_scene.preview.unknown"))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!(
                    "ui_scene: unknown scene '{name}'.\navailable: {:?}",
                    generated::names()
                ));
            });
    }
}

#[cfg(test)]
mod tests {
    /// Poka-yoke: the committed sample scene must stay registered, so the call site this whole
    /// feature hangs off is always exercised by the build.
    #[test]
    fn sample_scene_is_registered() {
        assert!(super::generated::names().contains(&"sample"));
    }

    /// Smoke test: a generated scene must lay out without panicking in a headless egui context.
    /// Guards the contract that generated code only calls APIs that exist and compose cleanly.
    #[test]
    fn sample_scene_renders_without_panic() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ctx| {
            assert!(super::generated::render("sample", ctx));
        });
    }
}
