use egui::{vec2, Color32, RichText, Stroke};
use sow_i18n::Language;

use super::super::state::{get_player_display_name, HudState};

pub(in crate::ui::hud) enum DispatchKind {
    Incoming,
    Outgoing,
    Navy,
}

#[allow(clippy::type_complexity)]
pub(in crate::ui::hud) fn draw_battle_log_tab(
    ui: &mut egui::Ui,
    state: &HudState,
    width: f32,
    compact: bool,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
) {
    let my_pid = state.my_player_id;
    if my_pid == 0 {
        return;
    }

    let strings = &sow_i18n::get(lang).hud;
    let log_h = if compact { 120.0 } else { 140.0 };

    let mut rows: Vec<(DispatchKind, f64, String, Option<u64>, Option<u64>, bool)> = Vec::new();

    for attack in state.attacks.iter().filter(|a| a.target_owner == my_pid) {
        let name: String = get_player_display_name(
            &state.players,
            attack.owner_id,
            &strings.default_player_name,
        )
        .chars()
        .take(12)
        .collect();
        rows.push((
            DispatchKind::Incoming,
            attack.troops,
            format!("{name} → You"),
            Some(attack.id),
            None,
            attack.retreating,
        ));
    }
    for attack in state.attacks.iter().filter(|a| a.owner_id == my_pid) {
        let name: String = get_player_display_name(
            &state.players,
            attack.target_owner,
            &strings.wilderness_player_name,
        )
        .chars()
        .take(12)
        .collect();
        rows.push((
            DispatchKind::Outgoing,
            attack.troops,
            format!("You → {name}"),
            Some(attack.id),
            None,
            attack.retreating,
        ));
    }
    for fleet in state.fleets.iter().filter(|f| f.owner_id == my_pid) {
        rows.push((
            DispatchKind::Navy,
            fleet.troops,
            strings.naval_fleet_label.clone(),
            None,
            Some(fleet.id),
            fleet.retreating,
        ));
    }

    if rows.is_empty() {
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
            crate::widgets::try_paint_emoji(ui.painter(), "⚔", rect, Color32::GRAY);
            ui.label(
                RichText::new(&strings.battle_log_empty)
                    .size(11.0)
                    .color(Color32::GRAY)
                    .italics(),
            );
        });
        return;
    }

    egui::ScrollArea::vertical()
        .max_height(log_h)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = sow_ui_kit::theme::margin::TIGHT as f32;

            for (kind, troops, label, attack_id, fleet_id, retreating) in rows {
                let (icon, accent) = match kind {
                    DispatchKind::Incoming => ("⚔", sow_ui_kit::theme::palette::danger()),
                    DispatchKind::Outgoing => ("🛡", sow_ui_kit::theme::palette::neon_cyan()),
                    DispatchKind::Navy => ("⛴", sow_ui_kit::theme::palette::neon_cyan()),
                };

                egui::Frame::NONE
                    .fill(sow_ui_kit::theme::palette::surface_transparent())
                    .stroke(Stroke::new(
                        sow_ui_kit::theme::stroke::EMPHASIS,
                        accent.linear_multiply(0.55),
                    ))
                    .corner_radius(sow_ui_kit::theme::radius::sm())
                    .inner_margin(egui::Margin::symmetric(
                        sow_ui_kit::theme::margin::COZY,
                        sow_ui_kit::theme::margin::TIGHT,
                    ))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (icon_rect, _) =
                                ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                            crate::widgets::try_paint_emoji(ui.painter(), icon, icon_rect, accent);
                            ui.add_space(6.0);
                            ui.vertical(|ui| {
                                crate::widgets::emoji_label(
                                    ui,
                                    &label,
                                    egui::FontId::proportional(if compact { 11.0 } else { 12.0 }),
                                    accent,
                                );
                                ui.label(
                                    RichText::new(crate::utils::format_number(troops))
                                        .size(10.0)
                                        .color(sow_ui_kit::theme::palette::text_muted()),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if !retreating {
                                        match kind {
                                            DispatchKind::Incoming => {
                                                if let Some(aid) = attack_id {
                                                    let owner = state
                                                        .attacks
                                                        .iter()
                                                        .find(|a| a.id == aid)
                                                        .map(|a| a.owner_id)
                                                        .unwrap_or(0);
                                                    let btn = crate::widgets::ThemeButton::new("⚔")
                                                        .style(crate::widgets::ThemeButtonStyle::Danger)
                                                        .custom_fill(accent.linear_multiply(0.25))
                                                        .stroke(Stroke::new(
                                                            sow_ui_kit::theme::stroke::HAIRLINE,
                                                            sow_ui_kit::theme::palette::danger_border(),
                                                        ))
                                                        .min_size(vec2(28.0, 28.0))
                                                        .text_size(10.0);
                                                    if ui
                                                        .add(btn)
                                                        .on_hover_text(&strings.hover_retaliate)
                                                        .clicked()
                                                    {
                                                        let t =
                                                            state.troops * (state.attack_ratio as f64);
                                                        if t > 0.0 {
                                                            cancel_intents.push(
                                                                sow_core::protocol::GameplayIntent::Attack(
                                                                    sow_core::protocol::AttackIntent {
                                                                        target_owner: owner,
                                                                        troops: Some(t),
                                                                    },
                                                                ),
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                            DispatchKind::Outgoing => {
                                                if let Some(aid) = attack_id {
                                                    let cancel_btn = crate::widgets::ThemeButton::new("X")
                                                        .style(crate::widgets::ThemeButtonStyle::Tertiary)
                                                        .custom_fill(sow_ui_kit::theme::palette::button_inactive())
                                                        .min_size(vec2(28.0, 28.0))
                                                        .text_size(10.0);
                                                    if ui.add(cancel_btn).clicked()
                                                    {
                                                        cancel_intents.push(
                                                            sow_core::protocol::GameplayIntent::CancelAttack {
                                                                attack_id: aid,
                                                            },
                                                        );
                                                    }
                                                }
                                            }
                                            DispatchKind::Navy => {
                                                if let Some(fid) = fleet_id {
                                                    let cancel_btn = crate::widgets::ThemeButton::new("X")
                                                        .style(crate::widgets::ThemeButtonStyle::Tertiary)
                                                        .custom_fill(sow_ui_kit::theme::palette::button_inactive())
                                                        .min_size(vec2(28.0, 28.0))
                                                        .text_size(10.0);
                                                    if ui.add(cancel_btn).clicked()
                                                    {
                                                        cancel_intents.push(
                                                            sow_core::protocol::GameplayIntent::RecallFleet {
                                                                fleet_id: fid,
                                                            },
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                },
                            );
                        });
                    });
            }
        });
}
