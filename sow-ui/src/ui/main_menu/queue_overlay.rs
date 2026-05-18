use crate::UiAction;
use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, Ui};
use super::MainMenuState;

pub fn draw_queue_overlay(
    ui: &mut Ui,
    state: &MainMenuState,
    status_large: f32,
    section_gap: f32,
    action_min_h: f32,
    action: &mut Option<UiAction>,
) {
    let pad = if super::lobby_compact_layout(ui.ctx()) { 16.0 } else { 24.0 };

    Frame::new()
        .fill(crate::ui::theme::menu_backdrop())
        .inner_margin(pad)
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("WAITING FOR PLAYERS…")
                        .size(status_large)
                        .strong()
                        .color(Color32::WHITE),
                );
                ui.add_space(section_gap);
                ui.label(
                    RichText::new(format!("Starting in: {:.1}s", state.wait_timer_secs))
                        .size(20.0)
                        .color(Color32::from_rgb(255, 210, 120)),
                );
                ui.add_space(section_gap);

                if let Some(lobby_id) = state.joined_lobby_id.or(state.pending_join_lobby_id) {
                    if let Some(lobby) = state.lobbies.iter().find(|l| l.id == lobby_id) {
                        ui.label(
                            RichText::new(format!(
                                "Connected Players ({}/{})",
                                lobby.num_players, lobby.max_players
                            ))
                            .strong()
                            .color(crate::ui::theme::text_secondary()),
                        );
                        ui.add_space(8.0);
                        for p in &lobby.players {
                            Frame::new()
                                .fill(crate::ui::theme::menu_secondary_button())
                                .stroke(Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border()))
                                .corner_radius(CornerRadius::same(6))
                                .inner_margin(Margin::same(10))
                                .show(ui, |ui| {
                                    ui.set_width(220.0);
                                    ui.horizontal(|ui| {
                                        let map_ready = p.download_progress == 100 || p.is_ready;
                                        if map_ready {
                                            ui.label(RichText::new("✔").color(Color32::GREEN));
                                        } else {
                                            ui.add(egui::Spinner::new().size(12.0));
                                        }
                                        ui.add_space(8.0);
                                        ui.label(
                                            RichText::new(&p.name).size(16.0).color(Color32::WHITE),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if !map_ready && p.download_progress > 0 {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{}%",
                                                            p.download_progress
                                                        ))
                                                        .size(12.0)
                                                        .color(crate::ui::theme::text_secondary()),
                                                    );
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(6.0);
                        }
                    }
                }

                ui.add_space(section_gap * 1.5);

                let cancel = egui::Button::new(RichText::new("CANCEL").color(Color32::WHITE))
                    .fill(crate::ui::theme::accent_danger())
                    .stroke(Stroke::new(1.0_f32, crate::ui::theme::accent_danger_border()))
                    .min_size(egui::vec2(200.0, action_min_h));
                if ui.add(cancel).clicked() {
                    *action = Some(UiAction::LeaveLobby);
                }
            });
        });
}
