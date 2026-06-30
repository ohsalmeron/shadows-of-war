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
    state.single_player_config.player_leader = state.selected_leader;
    state.single_player_config.player_civilization = state.selected_civilization;

    if let Some(catalog) = &asset_loader.map_catalog {
        state.apply_map_catalog(catalog);
    }

    let strings = &sow_i18n::get(lang).main_menu;
    let mut is_open = state.show_single_player_setup;
    let mut should_close = false;

    if root_ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        should_close = true;
    }

    sow_ui_kit::theme::draw_standard_modal(
        root_ui.ctx(),
        &mut is_open,
        "single_player_setup",
        &strings.single_player_skirmish,
        "",
        reduced_motion,
        |ui| {
            // Apply compact padding globally inside the entire modal to guarantee snug layout
            ui.style_mut().spacing.button_padding = egui::vec2(8.0, 4.0);

            let compact = sow_ui_kit::theme::compact_viewport(ui.ctx());
            let config = &mut state.single_player_config;
            let item_gap = if compact { 4.0 } else { 8.0 };

            if compact {
                // Mobile layout: neat vertical stack
                draw_map_preview(ui, config, asset_loader, strings);
                ui.add_space(item_gap);
                
                draw_map_selection_card(ui, config, asset_loader, strings);
                ui.add_space(item_gap);
                
                draw_difficulty_spawning_card(ui, config, strings);
                ui.add_space(item_gap);
                
                draw_sliders_card(ui, config, strings);
                ui.add_space(12.0);

                // Start simulation button (LAUNCH) - always at the bottom of the scroll on mobile
                let start_btn = crate::widgets::ThemeButton::new(&strings.start_simulation)
                    .style(crate::widgets::ThemeButtonStyle::Secondary)
                    .min_size(Vec2::new(ui.available_width(), 36.0))
                    .text_size(20.0);
                if ui.add(start_btn).clicked() {
                    *action = Some(UiAction::StartSinglePlayer(Box::new(*config.clone())));
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
                        
                        draw_difficulty_spawning_card(ui, config, strings);
                    });
                    columns[1].vertical(|ui| {
                        draw_sliders_card(ui, config, strings);
                        ui.add_space(12.0);

                        // Start simulation button (balances left column height perfectly, 60px fixed high-profile)
                        let start_btn = crate::widgets::ThemeButton::new(&strings.start_simulation)
                            .style(crate::widgets::ThemeButtonStyle::Secondary)
                            .min_size(Vec2::new(ui.available_width(), 60.0))
                            .text_size(20.0);
                        if ui.add(start_btn).clicked() {
                            *action = Some(UiAction::StartSinglePlayer(Box::new(*config.clone())));
                            should_close = true;
                        }
                    });
                });
            }
        },
    );

    if should_close || !is_open {
        state.show_single_player_setup = false;
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
        egui::ComboBox::from_id_salt("sp_map")
            .width(ui.available_width())
            .selected_text(selected_label)
            .show_ui(ui, |ui| {
                ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                if let Some(catalog) = &asset_loader.map_catalog {
                    if catalog.is_empty() {
                        ui.label(&strings.no_maps_found);
                    } else {
                        for map_entry in catalog {
                            let label = if asset_loader.has_map(&map_entry.key) {
                                format!("{}{}", map_entry.display_name, strings.map_offline_tag)
                            } else {
                                map_entry.display_name.clone()
                            };
                            ui.selectable_value(&mut config.map_name, map_entry.key.clone(), label);
                        }
                    }
                } else {
                    ui.label(&strings.loading_maps);
                }
            });
    });
}

fn draw_difficulty_spawning_card(
    ui: &mut egui::Ui,
    config: &mut sow_core::game_config::GameConfig,
    strings: &sow_i18n::MainMenuStrings,
) {
    panel_card(ui, |ui| {
        let side_by_side = ui.available_width() > 360.0;
        if side_by_side {
            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    ui.label(RichText::new(&strings.bot_difficulty).size(10.0).color(palette::text_muted()));
                    ui.add_space(3.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let is_vanilla = config.bot_difficulty == sow_core::game_config::BotDifficulty::Vanilla;
                        if pill_toggle(ui, "VANILLA", is_vanilla) {
                            config.bot_difficulty = sow_core::game_config::BotDifficulty::Vanilla;
                        }
                        if pill_toggle(ui, "TERMINATOR", !is_vanilla) {
                            config.bot_difficulty = sow_core::game_config::BotDifficulty::Terminator;
                        }
                    });
                });
                cols[1].vertical(|ui| {
                    ui.label(RichText::new(&strings.random_spawning).size(10.0).color(palette::text_muted()));
                    ui.add_space(3.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if pill_toggle(ui, "ON", config.random_spawn) { config.random_spawn = true; }
                        if pill_toggle(ui, "OFF", !config.random_spawn) { config.random_spawn = false; }
                    });
                });
            });
        } else {
            ui.label(RichText::new(&strings.bot_difficulty).size(10.0).color(palette::text_muted()));
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let is_vanilla = config.bot_difficulty == sow_core::game_config::BotDifficulty::Vanilla;
                if pill_toggle(ui, "VANILLA", is_vanilla) {
                    config.bot_difficulty = sow_core::game_config::BotDifficulty::Vanilla;
                }
                if pill_toggle(ui, "TERMINATOR", !is_vanilla) {
                    config.bot_difficulty = sow_core::game_config::BotDifficulty::Terminator;
                }
            });

            ui.add_space(6.0);

            ui.label(RichText::new(&strings.random_spawning).size(10.0).color(palette::text_muted()));
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if pill_toggle(ui, "ON", config.random_spawn) { config.random_spawn = true; }
                if pill_toggle(ui, "OFF", !config.random_spawn) { config.random_spawn = false; }
            });
        }
    });
}

fn draw_sliders_card(
    ui: &mut egui::Ui,
    config: &mut sow_core::game_config::GameConfig,
    strings: &sow_i18n::MainMenuStrings,
) {
    panel_card(ui, |ui| {
        ui.label(RichText::new(&strings.tribes_count).size(10.0).color(palette::text_muted()));
        ui.add_space(2.0);
        draw_custom_slider(ui, &mut config.bot_count, 0..=1000);

        ui.add_space(6.0);

        ui.label(RichText::new(&strings.nations_count).size(10.0).color(palette::text_muted()));
        ui.add_space(2.0);
        draw_custom_slider(ui, &mut config.nation_count, 0..=400);

        ui.add_space(6.0);

        ui.label(RichText::new("Seed").size(10.0).color(palette::text_muted()));
        ui.add_space(2.0);
        draw_custom_slider(ui, &mut config.seed, 1..=9999);
    });
}