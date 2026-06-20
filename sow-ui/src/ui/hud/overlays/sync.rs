use egui::{vec2, Color32, Context, RichText};
use sow_i18n::Language;

use super::super::state::HudState;

pub(in crate::ui::hud) fn draw_sync_overlay(ctx: &Context, state: &HudState, lang: Language) {
    if let Some(sync) = &state.sync_state {
        let strings = &sow_i18n::get(lang).hud;
        let screen_rect = ctx.content_rect();
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("sync_overlay"),
        ))
        .rect_filled(screen_rect, 0.0, Color32::from_black_alpha(180));

        egui::Window::new(&strings.overlay_waiting_players)
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    if sync.is_starting {
                        sow_ui_kit::theme::outlined_label(
                            ui,
                            &strings.overlay_all_ready,
                            egui::FontId::proportional(24.0),
                            Color32::GREEN,
                        );
                        ui.label(
                            RichText::new(&strings.overlay_stabilizing)
                                .size(16.0)
                                .color(Color32::LIGHT_GRAY),
                        );
                    } else {
                        sow_ui_kit::theme::outlined_label(
                            ui,
                            &strings.overlay_waiting_players,
                            egui::FontId::proportional(24.0),
                            Color32::WHITE,
                        );
                        ui.label(
                            RichText::new(format!(
                                "{}{:.1}{}",
                                strings.overlay_starting_in,
                                sync.time_remaining,
                                strings.overlay_seconds_short
                            ))
                            .size(18.0)
                            .color(Color32::YELLOW),
                        );
                    }

                    ui.add_space(20.0);
                    let total = sync.players.len();
                    let ready = sync.players.iter().filter(|p| p.is_ready).count();
                    let ratio = if total == 0 {
                        0.0
                    } else {
                        ready as f32 / total as f32
                    };
                    ui.add(egui::ProgressBar::new(ratio).text(format!(
                        "{}/{} {}",
                        ready, total, strings.overlay_players_ready
                    )));

                    ui.add_space(15.0);
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for p in &sync.players {
                                ui.horizontal(|ui| {
                                    if p.is_ready {
                                        ui.label(RichText::new("✔").color(Color32::GREEN));
                                    } else {
                                        ui.add(
                                            egui::Spinner::new()
                                                .size(14.0)
                                                .color(Color32::LIGHT_GRAY),
                                        );
                                    }
                                    ui.label(RichText::new(&p.name).color(Color32::WHITE));
                                });
                            }
                        });
                });
            });
    }
}
