use super::MainMenuState;
use crate::UiAction;
use egui::{Color32, CornerRadius, Margin, RichText, Stroke};
use sow_ui_kit::theme::palette;

fn setting_card(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    let frame = egui::Frame::NONE
        .fill(sow_ui_kit::theme::palette::field_bg())
        .stroke(Stroke::new(
            1.0_f32,
            sow_ui_kit::theme::palette::field_border(),
        ))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 8));

    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical(|ui| {
            ui.label(RichText::new(title).small().color(palette::text_muted()));
            ui.add_space(4.0);
            content(ui);
        });
    });
}

fn draw_custom_slider<N>(ui: &mut egui::Ui, value: &mut N, range: std::ops::RangeInclusive<N>)
where
    N: egui::emath::Numeric + std::fmt::Display,
{
    ui.horizontal(|ui| {
        let total_w = ui.available_width();
        let qty_w = 52.0;
        let spacing = 8.0;
        let slider_w = (total_w - qty_w - spacing).max(40.0);

        ui.scope(|ui| {
            ui.spacing_mut().slider_width = slider_w;
            ui.add(
                egui::Slider::new(value, range)
                    .show_value(false)
                    .trailing_fill(true),
            );
        });

        ui.add_space(spacing);
        ui.label(RichText::new(value.to_string()).strong());
    });
}

pub fn draw(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    action: &mut Option<UiAction>,
    lang: sow_i18n::Language,
    reduced_motion: bool,
) {
    state.single_player_config.player_leader = state.selected_leader;
    state.single_player_config.player_civilization = state.selected_civilization;

    if let Some(catalog) = &asset_loader.map_catalog {
        state.apply_map_catalog(catalog);
    }

    let strings = &sow_i18n::get(lang).main_menu;
    let mut is_open = state.show_single_player_setup;
    let mut should_close = false;

    sow_ui_kit::theme::draw_standard_modal(
        root_ui.ctx(),
        &mut is_open,
        "single_player_setup",
        &strings.single_player_skirmish,
        &strings.back,
        reduced_motion,
        |ui| {
            let config = &mut state.single_player_config;
            let item_gap = 10.0;

            // Map preview
            let map_name = config.map_name.clone();
            asset_loader.request_thumbnail(&map_name);
            let thumbnail = asset_loader.thumbnail(&config.map_name);
            let aspect = 1.77_f32;
            let w = ui.available_width();
            let h = (w / aspect).clamp(48.0, 200.0);
            let rect = ui
                .allocate_exact_size(egui::vec2(w, h), egui::Sense::hover())
                .0;

            if let Some(tex) = thumbnail {
                let uv = crate::ui::map_texture::cover_uv(rect.size(), tex.size_vec2());
                crate::ui::map_texture::draw_map_thumbnail_uv(
                    ui.painter(),
                    tex.id(),
                    rect,
                    uv,
                    1.0,
                );
            } else {
                ui.painter()
                    .rect_filled(rect, 8.0, Color32::from_black_alpha(120));
                let status = if let Some(err) = asset_loader.thumbnail_error(&config.map_name) {
                    format!("Thumbnail: {err}")
                } else if asset_loader.thumbnail_in_flight(&config.map_name) {
                    strings.loading_maps.to_string()
                } else {
                    strings.no_preview.to_string()
                };
                sow_ui_kit::theme::paint_premium_glow_text(
                    ui.painter(),
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &status,
                    egui::FontId::proportional(13.0),
                    palette::text_muted(),
                    Color32::BLACK,
                );
            }

            ui.painter().rect_stroke(
                rect,
                8.0_f32,
                Stroke::new(1.0_f32, sow_ui_kit::theme::palette::neon_cyan_glow()),
                egui::StrokeKind::Inside,
            );

            ui.add_space(item_gap);

            // Map picker
            setting_card(ui, &strings.map_selection, |ui| {
                let selected_label = asset_loader
                    .map_catalog
                    .as_ref()
                    .and_then(|c| sow_core::maps::catalog_lookup(c, &config.map_name))
                    .map(|e| e.display_name.as_str())
                    .unwrap_or(config.map_name.as_str());
                egui::ComboBox::from_id_salt("sp_map")
                    .width(ui.available_width())
                    .selected_text(selected_label)
                    .show_ui(ui, |ui| {
                        if let Some(catalog) = &asset_loader.map_catalog {
                            if catalog.is_empty() {
                                ui.label(&strings.no_maps_found);
                            } else {
                                for map_entry in catalog {
                                    let label = if asset_loader.has_map(&map_entry.key) {
                                        format!(
                                            "{}{}",
                                            map_entry.display_name, strings.map_offline_tag
                                        )
                                    } else {
                                        map_entry.display_name.clone()
                                    };
                                    ui.selectable_value(
                                        &mut config.map_name,
                                        map_entry.key.clone(),
                                        label,
                                    );
                                }
                            }
                        } else {
                            ui.label(&strings.loading_maps);
                        }
                    });
            });

            ui.add_space(item_gap);

            // Difficulty
            setting_card(ui, &strings.bot_difficulty, |ui| {
                egui::ComboBox::from_id_salt("sp_diff")
                    .width(ui.available_width())
                    .selected_text(format!("{:?}", config.bot_difficulty))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut config.bot_difficulty,
                            sow_core::game_config::BotDifficulty::Vanilla,
                            "Vanilla",
                        );
                        ui.selectable_value(
                            &mut config.bot_difficulty,
                            sow_core::game_config::BotDifficulty::Terminator,
                            "Terminator",
                        );
                    });
            });

            ui.add_space(item_gap);

            // Tribes
            setting_card(ui, &strings.tribes_count, |ui| {
                draw_custom_slider(ui, &mut config.bot_count, 0..=1000);
            });

            ui.add_space(item_gap);

            // Nations
            setting_card(ui, &strings.nations_count, |ui| {
                draw_custom_slider(ui, &mut config.nation_count, 0..=400);
            });

            ui.add_space(item_gap);

            // Spawn
            setting_card(ui, &strings.random_spawning, |ui| {
                let btn_text = if config.random_spawn { "ON" } else { "OFF" };
                let btn = egui::Button::new(btn_text).fill(if config.random_spawn {
                    sow_ui_kit::theme::palette::neon_cyan()
                } else {
                    sow_ui_kit::theme::palette::button_inactive()
                });
                if ui.add(btn).clicked() {
                    config.random_spawn = !config.random_spawn;
                }
            });

            ui.add_space(item_gap);

            // World seed
            setting_card(ui, "WORLD SEED", |ui| {
                draw_custom_slider(ui, &mut config.seed, 1..=9999);
            });

            ui.add_space(16.0);

            // Start button
            let start_btn = crate::widgets::ThemeButton::new(&strings.start_simulation)
                .style(crate::widgets::ThemeButtonStyle::Primary)
                .min_size(egui::vec2(ui.available_width(), 40.0));
            if ui.add(start_btn).clicked() {
                *action = Some(UiAction::StartSinglePlayer(Box::new(*config.clone())));
                should_close = true;
            }
        },
    );

    if should_close || !is_open {
        state.show_single_player_setup = false;
    }
}
