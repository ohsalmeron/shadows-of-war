use egui::{pos2, Color32};
use sow_i18n::Language;

use super::super::state::HudState;

pub(in crate::ui::hud) fn draw_mobile_selection_bar(
    ui: &mut egui::Ui,
    state: &HudState,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
) {
    if let Some(tile_info) = &state.selected_tile {
        use crate::ui::theme::palette;
        use egui::RichText;
        let strings = &sow_i18n::get(lang).hud;

        if tile_info.is_spawning {
            return;
        }

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let text_color = if tile_info.is_own_territory {
                    palette::neon_gold()
                } else if tile_info.is_friendly {
                    palette::neon_cyan()
                } else {
                    palette::danger()
                };
                let status_text = if tile_info.is_own_territory {
                    &strings.status_own
                } else if tile_info.is_friendly {
                    &strings.status_ally
                } else if tile_info.owner_id != 0 {
                    &strings.status_enemy
                } else {
                    &strings.status_neutral
                };
                ui.label(
                    RichText::new(format!(
                        "{}{}-{}",
                        strings.status_tile_prefix, tile_info.tile_idx, status_text
                    ))
                    .strong()
                    .size(11.0)
                    .color(text_color),
                );
            });

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                let btn_w = (ui.available_width() - 12.0) / 4.0;
                let btn_h = 48.0;

                // 1. Info Button
                let info_btn = crate::widgets::ThemeButton::new(&strings.btn_info)
                    .style(crate::widgets::ThemeButtonStyle::Tertiary)
                    .custom_fill(palette::button_inactive())
                    .stroke(egui::Stroke::new(1.0_f32, palette::text_muted()))
                    .min_size(egui::vec2(btn_w, btn_h))
                    .text_size(13.0);
                let _ = ui.add(info_btn);

                // 2. Fleet / Delete Button
                let right_fill = if tile_info.is_own_territory {
                    palette::danger()
                } else {
                    palette::neon_cyan()
                };
                let right_glow = if tile_info.is_own_territory {
                    palette::danger_border()
                } else {
                    palette::neon_cyan_hover()
                };
                let right_label = if tile_info.is_own_territory {
                    &strings.btn_delete
                } else {
                    &strings.btn_fleft
                };

                let fleet_btn = crate::widgets::ThemeButton::new(right_label)
                    .style(crate::widgets::ThemeButtonStyle::Primary)
                    .custom_fill(right_fill.linear_multiply(0.3))
                    .stroke(egui::Stroke::new(1.2_f32, right_glow))
                    .min_size(egui::vec2(btn_w, btn_h))
                    .text_size(13.0);

                if ui.add(fleet_btn).clicked() {
                    let troops = Some(state.troops * (state.attack_ratio as f64));
                    cancel_intents.push(sow_core::protocol::GameplayIntent::LaunchFleet {
                        target_tile: tile_info.tile_idx,
                        troops,
                    });
                }

                // 3. Ally Button
                let ally_btn = crate::widgets::ThemeButton::new(&strings.btn_ally)
                    .style(crate::widgets::ThemeButtonStyle::Tertiary)
                    .custom_fill(palette::button_inactive())
                    .stroke(egui::Stroke::new(1.0_f32, palette::neon_cyan()))
                    .min_size(egui::vec2(btn_w, btn_h))
                    .text_size(13.0);
                let _ = ui.add(ally_btn);

                // 4. Build / Attack Button
                let left_fill = if tile_info.is_own_territory {
                    palette::neon_gold()
                } else {
                    palette::danger()
                };
                let left_glow = if tile_info.is_own_territory {
                    palette::neon_gold_hover()
                } else {
                    palette::danger_border()
                };
                let left_label = if tile_info.is_own_territory {
                    &strings.btn_build
                } else {
                    &strings.btn_attack
                };

                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(btn_w, btn_h), egui::Sense::click());
                let is_hovered = resp.hovered();
                let fill = if is_hovered {
                    left_fill.linear_multiply(0.4)
                } else {
                    left_fill.linear_multiply(0.3)
                };
                ui.painter().rect(
                    rect,
                    6.0,
                    fill,
                    egui::Stroke::new(1.2_f32, left_glow),
                    egui::StrokeKind::Inside,
                );

                let font_id = egui::FontId::proportional(13.0);
                let galley = ui.painter().layout_no_wrap(
                    left_label.to_owned(),
                    font_id.clone(),
                    Color32::WHITE,
                );
                let start_x = rect.center().x - galley.rect.width() / 2.0;
                ui.painter().galley(
                    pos2(start_x, rect.center().y - galley.rect.height() / 2.0),
                    galley,
                    Color32::WHITE,
                );

                if resp.clicked() && !tile_info.is_own_territory {
                    let troops = state.troops * (state.attack_ratio as f64);
                    if troops > 0.0 {
                        let attack = sow_core::protocol::AttackIntent {
                            target_owner: tile_info.owner_id,
                            troops: Some(troops),
                        };
                        cancel_intents.push(sow_core::protocol::GameplayIntent::Attack(attack));
                    }
                }
            });
            ui.add_space(4.0);
        });
    }
}
