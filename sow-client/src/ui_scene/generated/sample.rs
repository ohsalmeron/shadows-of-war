//! GENERATED-STYLE exemplar — `sample` scene.
//!
//! Hand-written for M0 to lock the *shape* the UI Scene Editor's exporter will emit: an absolute
//! root `Area`, a themed `Frame`, and children styled via `sow_ui_kit::theme` tokens — so the
//! authored UI inherits the real look and renders 1:1. The future exporter produces this same
//! form from `assets/ui/sample.json`.

use egui::{Id, RichText, pos2};
use sow_ui_kit::theme;

/// Render the `sample` scene. Absolute-positioned (Unity-style) via a fixed-pos `Area`, centered
/// in the current screen so it reads regardless of window size.
pub fn render(ctx: &egui::Context) {
    let screen = ctx.content_rect();
    let panel_w = 320.0_f32;
    let panel_h = 188.0_f32;
    let top_left = pos2(
        screen.center().x - panel_w * 0.5,
        screen.center().y - panel_h * 0.5,
    );

    egui::Area::new(Id::new("ui_scene.sample.root"))
        .fixed_pos(top_left)
        .show(ctx, |ui| {
            ui.set_width(panel_w);
            // Panel component → real theme frame (fill + stroke + corner radius + margins + shadow).
            theme::hud_panel_frame().show(ui, |ui| {
                ui.set_width(panel_w);
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    // Text component (title).
                    ui.label(
                        RichText::new("Sample Scene")
                            .size(26.0)
                            .strong()
                            .color(theme::palette::neon_gold()),
                    );
                    ui.add_space(6.0);
                    // Text component (subtitle).
                    ui.label(
                        RichText::new("Authored in the browser · rendered 1:1 by the real client")
                            .size(13.0)
                            .color(theme::palette::text_muted()),
                    );
                    ui.add_space(14.0);
                    // Buttons (static for M0 — layout only, no behavior).
                    let _ = ui.button(
                        RichText::new("Resume")
                            .size(15.0)
                            .color(theme::palette::text_normal()),
                    );
                    ui.add_space(6.0);
                    let _ = ui.button(
                        RichText::new("Quit")
                            .size(15.0)
                            .color(theme::palette::text_normal()),
                    );
                    ui.add_space(10.0);
                });
            });
        });
}
