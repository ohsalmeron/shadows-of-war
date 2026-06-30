use super::MainMenuState;
use crate::UiAction;
use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, Vec2};
use sow_ui_kit::theme::palette;

fn pill_toggle(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let (bg, text) = if active {
        (palette::neon_cyan(), Color32::BLACK)
    } else {
        (palette::button_inactive(), palette::text_muted())
    };
    
    // Ensure compact padding for these specific toggles
    ui.style_mut().spacing.button_padding = egui::vec2(8.0, 4.0);

    let btn = egui::Button::new(RichText::new(label).size(12.0).color(text).strong())
        .fill(bg)
        .stroke(Stroke::new(1.0, if active { palette::neon_cyan() } else { palette::field_border() }))
        .corner_radius(CornerRadius::same(6))
        .min_size(Vec2::new(0.0, 28.0));
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

fn draw_custom_slider<N>(ui: &mut egui::Ui, value: &mut N, range: std::ops::RangeInclusive<N>)
where
    N: egui::emath::Numeric + std::fmt::Display,
{
    ui.horizontal(|ui| {
        let total_w = ui.available_width();
        let qty_w = 48.0;
        let spacing = 6.0;
        let slider_w = (total_w - qty_w - spacing).max(30.0);
        
        ui.spacing_mut().slider_width = slider_w;
        ui.add(
            egui::Slider::new(value, range)
                .show_value(false)
                .trailing_fill(false),
        );
        ui.add_space(spacing);
        ui.label(RichText::new(value.to_string()).strong().size(12.0));
    });
}

fn panel_card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    let prepaint = ui.painter().add(egui::Shape::Noop);
    let frame = Frame::NONE
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            let inner_w = ui.available_width();
            ui.set_max_width(inner_w);
            ui.set_width(inner_w);
            ui.vertical(|ui| {
                content(ui);
            });
        });
    let rect = frame.response.rect;
    sow_ui_kit::theme::paint_hud_panel_gradient(
        ui, prepaint, rect, palette::field_border(), CornerRadius::same(8),
    );
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

    if root_ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        should_close = true;
    }

    sow_ui_kit::theme::draw_standard_modal(
        root_ui.ctx(),
        &mut is_open,
        "create_game",
        &strings.create_game_title,
        "",
        reduced_motion,
        |ui| {
            // Apply compact padding globally inside the entire modal to guarantee snug layout
            ui.style_mut().spacing.button_padding = egui::vec2(8.0, 4.0);

            let compact = sow_ui_kit::theme::compact_viewport(ui.ctx());
            let config = &mut state.create_game_config;
            let is_private = &mut state.create_game_is_private;
            let password = &mut state.create_game_password;
            let item_gap = if compact { 4.0 } else { 8.0 };

            if compact {
                // Mobile layout: neat vertical stack
                draw_map_preview(ui, config, asset_loader, strings);
                ui.add_space(item_gap);
                
                draw_map_selection_card(ui, config, asset_loader, strings);
                ui.add_space(item_gap);
                
                draw_lobby_difficulty_settings_card(ui, config, is_private, strings);
                ui.add_space(item_gap);
                
                draw_sliders_card(ui, config, strings);
                ui.add_space(item_gap);
                
                draw_security_card(ui, password, strings);
                ui.add_space(12.0);

                // Create Lobby Button
                let start_btn = crate::widgets::ThemeButton::new(&strings.create_lobby_btn)
                    .style(crate::widgets::ThemeButtonStyle::Secondary)
                    .min_size(Vec2::new(ui.available_width(), 36.0))
                    .text_size(20.0);
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
            } else {
                // Desktop layout: beautifully balanced 2-column layout with NO empty space at the bottom
                ui.columns(2, |columns| {
                    columns[0].vertical(|ui| {
                        draw_map_preview(ui, config, asset_loader, strings);
                        ui.add_space(item_gap);
                        
                        draw_map_selection_card(ui, config, asset_loader, strings);
                        ui.add_space(item_gap);
                        
                        draw_lobby_difficulty_settings_card(ui, config, is_private, strings);
                    });
                    columns[1].vertical(|ui| {
                        draw_sliders_card(ui, config, strings);
                        ui.add_space(item_gap);
                        
                        draw_security_card(ui, password, strings);
                        ui.add_space(16.0);

                        // Create Lobby Button (balances left column height perfectly, 60px fixed high-profile)
                        let start_btn = crate::widgets::ThemeButton::new(&strings.create_lobby_btn)
                            .style(crate::widgets::ThemeButtonStyle::Secondary)
                            .min_size(Vec2::new(ui.available_width(), 60.0))
                            .text_size(20.0);
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
                    });
                });
            }
        },
    );

    if should_close || !is_open {
        state.show_create_game = false;
    }
}

fn draw_map_preview(
    ui: &mut egui::Ui,
    config: &mut sow_core::game_config::GameConfig,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    strings: &sow_i18n::MainMenuStrings,
) {
    let compact = sow_ui_kit::theme::compact_viewport(ui.ctx());

    let thumbnail = asset_loader.thumbnail(&config.map_name);
    let aspect = 1.77_f32; // standard 16:9
    let w = ui.available_width();
    let h = (w / aspect).clamp(40.0, if compact { 90.0 } else { 160.0 });

    let rect = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover()).0;

    if let Some(tex) = thumbnail {
        let uv = crate::ui::map_texture::cover_uv(rect.size(), tex.size_vec2());
        crate::ui::map_texture::draw_map_thumbnail_uv(
            ui.painter(), tex.id(), rect, uv, 1.0, CornerRadius::same(6),
        );
    } else {
        ui.painter().rect_filled(rect, 6.0, Color32::from_black_alpha(120));
        let status = if asset_loader.thumbnail_error(&config.map_name).is_some() {
            strings.no_preview.to_string()
        } else if asset_loader.thumbnail_in_flight(&config.map_name) {
            strings.loading_maps.to_string()
        } else {
            strings.no_preview.to_string()
        };
        sow_ui_kit::theme::paint_premium_glow_text(
            ui.painter(), rect.center(), egui::Align2::CENTER_CENTER,
            &status, egui::FontId::proportional(12.0), palette::text_muted(), Color32::BLACK,
        );
    }
    ui.painter().rect_stroke(
        rect, 6.0,
        Stroke::new(1.0, palette::neon_cyan_glow()),
        egui::StrokeKind::Inside,
    );

    let label = asset_loader.map_catalog.as_ref()
        .and_then(|c| sow_core::maps::catalog_lookup(c, &config.map_name))
        .map(|e| e.display_name.as_str())
        .unwrap_or(config.map_name.as_str());
    ui.add_space(4.0);
    ui.label(RichText::new(label.to_uppercase()).size(13.0).strong().color(Color32::WHITE));
}

fn draw_map_selection_card(
    ui: &mut egui::Ui,
    config: &mut sow_core::game_config::GameConfig,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    strings: &sow_i18n::MainMenuStrings,
) {
    panel_card(ui, |ui| {
        let selected_label = asset_loader
            .map_catalog.as_ref()
            .and_then(|c| sow_core::maps::catalog_lookup(c, &config.map_name))
            .map(|e| e.display_name.as_str())
            .unwrap_or(config.map_name.as_str());
        egui::ComboBox::from_id_salt("cg_map")
            .width(ui.available_width())
            .selected_text(selected_label)
            .show_ui(ui, |ui| {
                ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                if let Some(catalog) = &asset_loader.map_catalog {
                    for map_entry in catalog {
                        let label = if asset_loader.has_map(&map_entry.key) {
                            format!("{}{}", map_entry.display_name, strings.map_offline_tag)
                        } else {
                            map_entry.display_name.clone()
                        };
                        ui.selectable_value(&mut config.map_name, map_entry.key.clone(), label);
                    }
                } else {
                    ui.label(&strings.loading_maps);
                }
            });
    });
}

fn draw_lobby_difficulty_settings_card(
    ui: &mut egui::Ui,
    config: &mut sow_core::game_config::GameConfig,
    is_private: &mut bool,
    strings: &sow_i18n::MainMenuStrings,
) {
    panel_card(ui, |ui| {
        let side_by_side = ui.available_width() > 360.0;
        if side_by_side {
            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    ui.label(RichText::new(&strings.game_mode_label).size(10.0).color(palette::text_muted()));
                    ui.add_space(3.0);
                    let mode_idx = if config.game_mode == "Teams" { 1 } else { 0 };
                    if let Some(new_idx) = draw_pill_row(ui, &["FFA", "TEAMS"], mode_idx) {
                        config.game_mode = if new_idx == 1 { "Teams".to_string() } else { "FFA".to_string() };
                    }

                    ui.add_space(6.0);

                    ui.label(RichText::new("VISIBILITY").size(10.0).color(palette::text_muted()));
                    ui.add_space(3.0);
                    let vis_idx = if *is_private { 1 } else { 0 };
                    if let Some(new_idx) = draw_pill_row(ui, &[&strings.visibility_public, &strings.visibility_private], vis_idx) {
                        *is_private = new_idx == 1;
                    }
                });
                cols[1].vertical(|ui| {
                    ui.label(RichText::new(&strings.bot_difficulty).size(10.0).color(palette::text_muted()));
                    ui.add_space(3.0);
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

                    ui.add_space(6.0);

                    ui.label(RichText::new(&strings.random_spawning).size(10.0).color(palette::text_muted()));
                    ui.add_space(3.0);
                    let spawn_idx = if config.random_spawn { 0 } else { 1 };
                    if let Some(new_idx) = draw_pill_row(ui, &["ON", "OFF"], spawn_idx) {
                        config.random_spawn = new_idx == 0;
                    }
                });
            });
        } else {
            ui.label(RichText::new(&strings.game_mode_label).size(10.0).color(palette::text_muted()));
            ui.add_space(3.0);
            let mode_idx = if config.game_mode == "Teams" { 1 } else { 0 };
            if let Some(new_idx) = draw_pill_row(ui, &["FFA", "TEAMS"], mode_idx) {
                config.game_mode = if new_idx == 1 { "Teams".to_string() } else { "FFA".to_string() };
            }

            ui.add_space(6.0);

            ui.label(RichText::new("VISIBILITY").size(10.0).color(palette::text_muted()));
            ui.add_space(3.0);
            let vis_idx = if *is_private { 1 } else { 0 };
            if let Some(new_idx) = draw_pill_row(ui, &[&strings.visibility_public, &strings.visibility_private], vis_idx) {
                *is_private = new_idx == 1;
            }

            ui.add_space(8.0);

            ui.label(RichText::new(&strings.bot_difficulty).size(10.0).color(palette::text_muted()));
            ui.add_space(3.0);
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

            ui.add_space(6.0);

            ui.label(RichText::new(&strings.random_spawning).size(10.0).color(palette::text_muted()));
            ui.add_space(3.0);
            let spawn_idx = if config.random_spawn { 0 } else { 1 };
            if let Some(new_idx) = draw_pill_row(ui, &["ON", "OFF"], spawn_idx) {
                config.random_spawn = new_idx == 0;
            }
        }
    });
}

fn draw_sliders_card(
    ui: &mut egui::Ui,
    config: &mut sow_core::game_config::GameConfig,
    strings: &sow_i18n::MainMenuStrings,
) {
    panel_card(ui, |ui| {
        ui.label(RichText::new(&strings.max_players_label).size(10.0).color(palette::text_muted()));
        ui.add_space(2.0);
        draw_custom_slider(ui, &mut config.max_players, 2..=16);

        ui.add_space(6.0);

        ui.label(RichText::new(&strings.tribes_count).size(10.0).color(palette::text_muted()));
        ui.add_space(2.0);
        draw_custom_slider(ui, &mut config.bot_count, 0..=1000);

        ui.add_space(6.0);

        ui.label(RichText::new(&strings.nations_count).size(10.0).color(palette::text_muted()));
        ui.add_space(2.0);
        draw_custom_slider(ui, &mut config.nation_count, 0..=400);
    });
}

fn draw_security_card(
    ui: &mut egui::Ui,
    password: &mut String,
    strings: &sow_i18n::MainMenuStrings,
) {
    panel_card(ui, |ui| {
        ui.label(RichText::new(&strings.password_label).size(10.0).color(palette::text_muted()));
        ui.add_space(3.0);
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
                    .font(egui::FontId::proportional(12.0))
                    .text_color(egui::Color32::WHITE),
            );
        });
    });
}