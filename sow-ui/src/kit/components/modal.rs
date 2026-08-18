use egui::{Align2, Color32, Context, Margin, Order, Ui, Vec2, vec2};
use sow_ui_kit::theme::palette;
use sow_ui_kit::theme::radius;
use super::button::Button;
use super::typography::Heading;

pub struct Modal<'a> {
    id: &'a str,
    title: &'a str,
    min_width: f32,
    max_width: f32,
    show_close: bool,
    close_text: &'a str,
}

impl<'a> Modal<'a> {
    pub fn new(id: &'a str, title: &'a str) -> Self {
        Self {
            id,
            title,
            min_width: 380.0,
            max_width: 580.0,
            show_close: true,
            close_text: "BACK",
        }
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width;
        self
    }

    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = max_width;
        self
    }

    pub fn close_button(mut self, show_close: bool, text: &'a str) -> Self {
        self.show_close = show_close;
        self.close_text = text;
        self
    }

    pub fn show<R>(
        self,
        ctx: &Context,
        open: &mut bool,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> Option<R> {
        if !*open {
            return None;
        }

        let mut result = None;

        // 1. Fullscreen dim backdrop
        egui::Area::new(egui::Id::new(format!("{}_backdrop", self.id)))
            .order(Order::Middle)
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui| {
                let screen_rect = ctx.content_rect();
                let resp = ui.allocate_rect(screen_rect, egui::Sense::click());
                ui.painter().rect_filled(
                    screen_rect,
                    0.0,
                    Color32::from_black_alpha(190),
                );
                if resp.clicked() {
                    *open = false;
                }
            });

        // 2. Centered Modal Window
        let screen = ctx.content_rect();
        let is_mobile = screen.width() < 500.0;
        let modal_w = if is_mobile {
            screen.width() - 32.0
        } else {
            self.max_width.min(screen.width() - 48.0)
        };

        egui::Area::new(egui::Id::new(format!("{}_window", self.id)))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                let frame = egui::Frame::NONE
                    .fill(palette::surface())
                    .stroke(egui::Stroke::new(1.5_f32, palette::field_border()))
                    .corner_radius(radius::lg())
                    .inner_margin(Margin::same(20));

                frame.show(ui, |ui| {
                    ui.set_width(modal_w);

                    // Header
                    ui.horizontal(|ui| {
                        Heading::new(self.title).size(22.0).show(ui);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if Button::ghost("X").small().min_size(Vec2::splat(28.0)).show(ui).clicked() {
                                *open = false;
                            }
                        });
                    });

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);

                    // Body
                    result = Some(add_contents(ui));

                    // Footer / Close action if enabled
                    if self.show_close {
                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if Button::secondary(self.close_text).show(ui).clicked() {
                                    *open = false;
                                }
                            });
                        });
                    }
                });
            });

        result
    }
}
