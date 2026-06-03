use crate::ui::theme::{compact_viewport, fullscreen_screen_frame, viewport_scale};
use egui::{Context, Id, ScrollArea, Ui};

pub struct FullscreenLayout<'a> {
    pub ui: &'a mut Ui,
    pub scale: f32,
    pub content_w: f32,
}

/// Edge-to-edge opaque overlay: header, scroll body, pinned footer.
pub fn show(
    ctx: &Context,
    id: Id,
    footer_h: f32,
    header: impl FnOnce(&mut Ui, f32, f32),
    body: impl FnOnce(FullscreenLayout<'_>),
    footer: impl FnOnce(&mut Ui, f32, f32),
) {
    let compact = compact_viewport(ctx);
    let scale = viewport_scale(ctx);
    let screen = ctx.content_rect();

    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .movable(false)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            ui.set_max_size(screen.size());

            fullscreen_screen_frame(compact).show(ui, |ui| {
                let content_w = ui.available_width();

                header(ui, scale, content_w);

                ui.add_space(8.0 * scale);
                ui.separator();
                ui.add_space(8.0 * scale);

                let scroll_h = (ui.available_height() - footer_h).max(0.0);

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(scroll_h)
                    .show(ui, |ui| {
                        ui.set_width(content_w);
                        body(FullscreenLayout {
                            ui,
                            scale,
                            content_w,
                        });
                    });

                footer(ui, scale, content_w);
            });
        });
}
