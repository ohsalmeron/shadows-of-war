use super::MainMenuState;
use crate::UiAction;
use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke};
use sow_ui_kit::theme::palette;

fn setting_card(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    let frame = Frame::NONE
        .fill(palette::field_bg())
        .stroke(Stroke::new(1.0_f32, palette::field_border()))
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

fn pill_toggle(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let (bg, text) = if active {
        (palette::neon_cyan(), Color32::BLACK)
    } else {
        (palette::button_inactive(), palette::text_muted())
    };
    let btn = egui::Button::new(RichText::new(label).size(13.0).color(text).strong())
        .fill(bg)
        .stroke(Stroke::new(
            1.0_f32,
            if active {
                palette::neon_cyan()
            } else {
                palette::field_border()
            },
        ))
        .corner_radius(CornerRadius::same(6));
    ui.add(btn).clicked()
}

fn draw_pill_row(ui: &mut egui::Ui, options: &[&str], selected: usize) -> Option<usize> {
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (i, label) in options.iter().enumerate() {
            if pill_toggle(ui, label, selected == i) {
                chosen = Some(i);
            }
        }
    });
    chosen
}

pub fn draw(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    action: &mut Option<UiAction>,
    lang: sow_i18n::Language,
    reduced_motion: bool,
) {
    if let Some(catalog) = &asset_loader.map_catalog {
        state.apply_map_catalog_create(catalog);
    }

    let strings = &sow_i18n::get(lang).main_menu;
    let mut is_open = state.show_create_game;
    let mut should_close = false;

    sow_ui_kit::theme::draw_standard_modal(
        root_ui.ctx(),
        &mut is_open,
        "create_game",
        &strings.create_game_title,
        &strings.back,
        reduced_motion,
        |ui| {
            let map_name = state.create_game_config.map_name.clone();
            asset_loader.request_thumbnail(&map_name);
            let config = &mut state.create_game_config;
            let is_private = &mut state.create_game_is_private;
            let password = &mut state.create_game_password;
            let item_gap = 10.0;

            // Map preview
            let thumbnail = asset_loader.thumbnail(&config.map_name);
            let aspect = 1.77_f32;
            let w = ui.available_width();
            let h = (w / aspect).clamp(48.0, 220.0);
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
                    egui::CornerRadius::same(8),
                );
            } else {
                ui.painter()
                    .rect_filled(rect, 8.0, Color32::from_black_alpha(120));
                sow_ui_kit::theme::paint_premium_glow_text(
                    ui.painter(),
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &strings.loading_maps,
                    egui::FontId::proportional(13.0),
                    palette::text_muted(),
                    Color32::BLACK,
                );
            }
            ui.painter().rect_stroke(
                rect,
                8.0_f32,
                Stroke::new(1.0_f32, palette::neon_cyan_glow()),
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
                egui::ComboBox::from_id_salt("cg_map")
                    .width(ui.available_width())
                    .selected_text(selected_label)
                    .show_ui(ui, |ui| {
                        if let Some(catalog) = &asset_loader.map_catalog {
                            for map_entry in catalog {
                                let label = if asset_loader.has_map(&map_entry.key) {
                                    format!("{}{}", map_entry.display_name, strings.map_offline_tag)
                                } else {
                                    map_entry.display_name.clone()
                                };
                                ui.selectable_value(
                                    &mut config.map_name,
                                    map_entry.key.clone(),
                                    label,
                                );
                            }
                        } else {
                            ui.label(&strings.loading_maps);
                        }
                    });
            });

            ui.add_space(item_gap);
            draw_settings_column(ui, config, is_private, password, strings);

            ui.add_space(16.0);
            let start_btn = crate::widgets::ThemeButton::new(&strings.create_lobby_btn)
                .style(crate::widgets::ThemeButtonStyle::Primary)
                .min_size(egui::vec2(ui.available_width(), 40.0));
            if ui.add(start_btn).clicked() {
                let config_copy = *config.clone();
                let password_copy = if password.is_empty() {
                    None
                } else {
                    Some(password.clone())
                };
                *action = Some(UiAction::CreateGame {
                    config: Box::new(config_copy),
                    is_private: *is_private,
                    password: password_copy,
                });
                should_close = true;
            }
        },
    );

    if should_close || !is_open {
        state.show_create_game = false;
    }
}

fn draw_settings_column(
    ui: &mut egui::Ui,
    config: &mut sow_core::game_config::GameConfig,
    is_private: &mut bool,
    password: &mut String,
    strings: &sow_i18n::MainMenuStrings,
) {
    // Game Mode
    setting_card(ui, &strings.game_mode_label, |ui| {
        let mode_idx = if config.game_mode == "Teams" { 1 } else { 0 };
        if let Some(new_idx) = draw_pill_row(ui, &["FFA", "TEAMS"], mode_idx) {
            config.game_mode = if new_idx == 1 {
                "Teams".to_string()
            } else {
                "FFA".to_string()
            };
        }
    });

    // Visibility
    setting_card(ui, "VISIBILITY", |ui| {
        let vis_idx = if *is_private { 1 } else { 0 };
        if let Some(new_idx) = draw_pill_row(
            ui,
            &[&strings.visibility_public, &strings.visibility_private],
            vis_idx,
        ) {
            *is_private = new_idx == 1;
        }
    });

    // Password
    setting_card(ui, &strings.password_label, |ui| {
        let field_frame = Frame::NONE
            .fill(palette::field_bg())
            .stroke(egui::Stroke::new(1.0_f32, palette::field_border()))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(8, 4));
        field_frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(
                egui::TextEdit::singleline(password)
                    .hint_text(&strings.password_hint)
                    .password(true)
                    .desired_width(f32::INFINITY)
                    .frame(Frame::NONE)
                    .font(egui::FontId::proportional(14.0))
                    .text_color(egui::Color32::WHITE),
            );
        });
    });

    // Max Players
    setting_card(ui, &strings.max_players_label, |ui| {
        draw_custom_slider(ui, &mut config.max_players, 2..=16);
    });

    // AI Tribes
    setting_card(ui, &strings.tribes_count, |ui| {
        draw_custom_slider(ui, &mut config.bot_count, 0..=1000);
    });

    // AI Nations
    setting_card(ui, &strings.nations_count, |ui| {
        draw_custom_slider(ui, &mut config.nation_count, 0..=400);
    });

    // Difficulty
    setting_card(ui, &strings.bot_difficulty, |ui| {
        let diff_idx = match config.bot_difficulty {
            sow_core::game_config::BotDifficulty::Terminator => 1,
            _ => 0,
        };
        if let Some(new_idx) = draw_pill_row(ui, &["VANILLA", "TERMINATOR"], diff_idx) {
            config.bot_difficulty = if new_idx == 1 {
                sow_core::game_config::BotDifficulty::Terminator
            } else {
                sow_core::game_config::BotDifficulty::Vanilla
            };
        }
    });

    // Random Spawn
    setting_card(ui, &strings.random_spawning, |ui| {
        let spawn_idx = if config.random_spawn { 0 } else { 1 };
        if let Some(new_idx) = draw_pill_row(ui, &["ON", "OFF"], spawn_idx) {
            config.random_spawn = new_idx == 0;
        }
    });
}
