use crate::app::SowApp;
use egui::Color32;

use super::popovers::ContextMenuTileOpts;

impl SowApp {
    pub(super) fn draw_missile_popover(
        &mut self,
        _ui: &mut egui::Ui,
        ctx: &egui::Context,
        opts: &ContextMenuTileOpts,
    ) {
        let tile_idx = opts.tile_idx;
        let center = opts.center;
        let scale = opts.scale;
        let compact = opts.compact;
        let screen = opts.screen;
        let outer_r = opts.outer_r;
        let is_own_territory = opts.is_own_territory;
        let radial_missile_active = opts.radial_missile_active;
        let missile_active_id = opts.missile_active_id;
        // Render Missile sub-popover
        if radial_missile_active && !is_own_territory {
            let mut area = egui::Area::new(egui::Id::new("radial_missile_popover"))
                .order(egui::Order::Tooltip);

            if compact {
                area = area
                    .fixed_pos(screen.center())
                    .pivot(egui::Align2::CENTER_CENTER);
            } else {
                area = area.fixed_pos(center - egui::vec2(outer_r + 240.0, 100.0));
            }

            let theme_color = Color32::from_rgb(239, 68, 68); // Red

            area.show(ctx, |ui| {
                let response_rect = ui.min_rect();
                ctx.data_mut(|d| {
                    d.insert_temp(egui::Id::new("missile_popover_rect"), response_rect)
                });

                egui::Frame::window(&ctx.global_style())
                    .fill(sow_ui_kit::theme::panel_bg())
                    .stroke(egui::Stroke::new(1.8_f32, theme_color))
                    .corner_radius(16)
                    .inner_margin(if compact { 16 } else { 12 })
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new("NUCLEAR STRIKE")
                                        .strong()
                                        .color(theme_color)
                                        .size(13.0),
                                );
                            });
                            ui.add_space(8.0);

                            let card_w = if compact { 280.0 } else { 220.0 };
                            let card_h = 50.0;

                            let nukes =
                                [(sow_core::game::NukeKind::AtomBomb, "Missile Strike", "☢️")];

                            for &(kind, label, icon) in &nukes {
                                let cost = kind.gold_cost(0);
                                let is_disabled = self.ui.app.hud_state.gold < cost;

                                let (rect, resp) = ui.allocate_exact_size(
                                    egui::vec2(card_w, card_h),
                                    egui::Sense::click(),
                                );
                                let is_hovered = resp.hovered() && !is_disabled;
                                let hover_id = ui.make_persistent_id(("popover_hover", label));
                                let hover_t =
                                    ui.ctx().animate_bool_with_time(hover_id, is_hovered, 0.15);

                                let border_glow = theme_color.linear_multiply(0.3 + 0.7 * hover_t);
                                let bg_fill = if is_disabled {
                                    Color32::from_rgba_unmultiplied(20, 20, 20, 180)
                                } else if is_hovered {
                                    theme_color.linear_multiply(0.12)
                                } else {
                                    Color32::from_rgba_unmultiplied(10, 15, 30, 220)
                                };

                                ui.painter().rect(
                                    rect,
                                    8.0,
                                    bg_fill,
                                    egui::Stroke::new(1.0_f32 + hover_t * 1.0_f32, border_glow),
                                    egui::StrokeKind::Inside,
                                );

                                // Icon
                                ui.painter().text(
                                    rect.min + egui::vec2(20.0, card_h / 2.0),
                                    egui::Align2::CENTER_CENTER,
                                    icon,
                                    egui::FontId::proportional((22.0 + 4.0 * hover_t) * scale),
                                    if is_disabled {
                                        Color32::GRAY
                                    } else {
                                        Color32::WHITE
                                    },
                                );

                                // Label
                                ui.painter().text(
                                    rect.min + egui::vec2(44.0, card_h / 2.0 - 8.0),
                                    egui::Align2::LEFT_CENTER,
                                    label,
                                    egui::FontId::proportional(13.0),
                                    if is_disabled {
                                        Color32::GRAY
                                    } else {
                                        Color32::WHITE
                                    },
                                );

                                // Cost
                                ui.painter().text(
                                    rect.min + egui::vec2(44.0, card_h / 2.0 + 8.0),
                                    egui::Align2::LEFT_CENTER,
                                    format!("{}g", cost as u32),
                                    egui::FontId::proportional(10.5),
                                    if is_disabled {
                                        Color32::from_rgb(180, 100, 100)
                                    } else {
                                        Color32::from_rgb(251, 191, 36)
                                    },
                                );

                                if !is_disabled && resp.clicked() {
                                    self.send_intent(
                                        sow_core::protocol::GameplayIntent::LaunchNuke {
                                            kind,
                                            target_tile: tile_idx,
                                        },
                                    );
                                    ctx.data_mut(|d| d.insert_temp(missile_active_id, false));
                                    self.input.map_context_menu = None;
                                }
                                ui.add_space(4.0);
                            }
                        });
                    });
            });
        }
    }
}
