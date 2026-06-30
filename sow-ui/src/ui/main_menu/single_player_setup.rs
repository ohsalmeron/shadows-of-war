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
    let btn = egui::Button::new(RichText::new(label).size(13.0).color(text).strong())
        .fill(bg)
        .stroke(Stroke::new(1.0, if active { palette::neon_cyan() } else { palette::field_border() }))
        .corner_radius(CornerRadius::same(6))
        .min_size(Vec2::new(0.0, 30.0));
    ui.add(btn).clicked()
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

fn panel_card(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    let prepaint = ui.painter().add(egui::Shape::Noop);
    let frame = Frame::NONE
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical(|ui| {
                ui.label(RichText::new(title).size(11.0).color(palette::text_muted()));
                ui.add_space(4.0);
                content(ui);
            });
        });
    let rect = frame.response.rect;
    sow_ui_kit::theme::paint_hud_panel_gradient(
        ui,
        prepaint,
        rect,
        palette::field_border(),
        CornerRadius::same(8),
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
    let is_open = state.show_single_player_setup;
    let mut should_close = false;

    if root_ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        should_close = true;
    }

    if !is_open {
        return;
    }

    let compact = sow_ui_kit::theme::compact_viewport(root_ui.ctx());
    let scale = sow_ui_kit::theme::viewport_scale(root_ui.ctx());
    let screen_rect = root_ui.ctx().input(|i| i.content_rect());

    sow_ui_kit::theme::paint_scrim(root_ui.ctx(), "solo_setup_scrim", 1.0);

    let margin = if compact { 0.0 } else { 16.0 };
    let panel_w = screen_rect.width() - margin * 2.0;
    let panel_h = screen_rect.height() - margin * 2.0;
    let x = screen_rect.min.x + margin;
    let y = screen_rect.min.y + margin;

    let panel_rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(panel_w, panel_h));

    let anim = root_ui.ctx().animate_bool_with_time(
        egui::Id::new("solo_setup_anim"),
        is_open,
        sow_ui_kit::theme::anim_duration(reduced_motion),
    );
    if anim <= 0.01 {
        return;
    }

    let base_y = if is_open {
        let t = anim;
        if t >= 1.0 { 0.0 } else { -40.0 * (1.0 - t) }
    } else {
        0.0
    };

    egui::Area::new(egui::Id::new("solo_setup_overlay"))
        .order(egui::Order::Foreground)
        .fixed_pos(panel_rect.min + egui::vec2(0.0, base_y))
        .show(root_ui.ctx(), |ui| {
            let prepaint = ui.painter().add(egui::Shape::Noop);
            let frame = Frame::NONE
                .inner_margin(if compact { Margin::same(12) } else { Margin::same(20) })
                .show(ui, |ui| {
                    ui.set_min_size(panel_rect.size());
                    ui.vertical(|ui| {
                        // Header
                        ui.horizontal(|ui| {
                            ui.set_width(ui.available_width());
                            sow_ui_kit::theme::outlined_label(
                                ui,
                                &strings.single_player_skirmish,
                                egui::FontId::proportional(if compact { 18.0 } else { 22.0 }),
                                Color32::WHITE,
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if sow_ui_kit::theme::modal_close_button(ui).clicked() {
                                    should_close = true;
                                }
                            });
                        });
                        ui.add_space(4.0);
                        ui.painter().line_segment(
                            [
                                egui::pos2(ui.min_rect().min.x + 4.0, ui.min_rect().max.y),
                                egui::pos2(ui.min_rect().max.x - 4.0, ui.min_rect().max.y),
                            ],
                            Stroke::new(1.0, palette::neon_cyan_glow()),
                        );
                        ui.add_space(if compact { 6.0 } else { 10.0 });

                        let config = &mut state.single_player_config;
                        let item_gap = if compact { 6.0 } else { 8.0 };

                        if compact {
                            let scroll_h = panel_h - 80.0;
                            egui::ScrollArea::vertical()
                                .id_salt("solo_setup_scroll")
                                .max_height(scroll_h)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    draw_map_preview(ui, config, asset_loader, strings, scale);
                                    ui.add_space(item_gap);
                                    draw_settings_column(ui, config, asset_loader, strings, item_gap, scale);
                                });
                            ui.add_space(8.0);
                            let start_btn = crate::widgets::ThemeButton::new(&strings.start_simulation)
                                .style(crate::widgets::ThemeButtonStyle::Primary)
                                .min_size(Vec2::new(ui.available_width(), 40.0));
                            if ui.add(start_btn).clicked() {
                                *action = Some(UiAction::StartSinglePlayer(Box::new(*config.clone())));
                                should_close = true;
                            }
                        } else {
                            ui.horizontal_top(|ui| {
                                let total_w = ui.available_width();
                                let left_w = (total_w * 0.42).min(360.0);
                                let right_w = (total_w - left_w - 12.0).max(280.0);

                                ui.allocate_ui_with_layout(
                                    Vec2::new(left_w, ui.available_height()),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        draw_map_preview(ui, config, asset_loader, strings, scale);
                                    },
                                );

                                ui.add_space(12.0);

                                ui.allocate_ui_with_layout(
                                    Vec2::new(right_w, ui.available_height()),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        draw_settings_column(ui, config, asset_loader, strings, item_gap, scale);
                                        ui.add_space(12.0);
                                        let start_btn = crate::widgets::ThemeButton::new(&strings.start_simulation)
                                            .style(crate::widgets::ThemeButtonStyle::Primary)
                                            .min_size(Vec2::new(ui.available_width(), 40.0));
                                        if ui.add(start_btn).clicked() {
                                            *action = Some(UiAction::StartSinglePlayer(Box::new(*config.clone())));
                                            should_close = true;
                                        }
                                    },
                                );
                            });
                        }
                    });
                });

            let radius = if compact { CornerRadius::ZERO } else { CornerRadius::same(12) };
            sow_ui_kit::theme::paint_hud_panel_gradient(
                ui,
                prepaint,
                frame.response.rect,
                palette::neon_cyan_glow(),
                radius,
            );
        });

    if should_close || !is_open {
        state.show_single_player_setup = false;
    }
}

fn draw_map_preview(
    ui: &mut egui::Ui,
    config: &mut sow_core::game_config::GameConfig,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    strings: &sow_i18n::MainMenuStrings,
    _scale: f32,
) {
    let compact = sow_ui_kit::theme::compact_viewport(ui.ctx());

    let thumbnail = asset_loader.thumbnail(&config.map_name);
    let aspect = thumbnail
        .map(|t| { let s = t.size_vec2(); if s.y > 0.0 { (s.x / s.y).clamp(0.5, 3.0) } else { 1.6 } })
        .unwrap_or(1.6);
    let w = ui.available_width();
    let max_h = if compact { 160.0 } else { ui.available_height() * 0.55 };
    let h = (w / aspect).clamp(48.0, max_h);

    let rect = ui.allocate_exact_size(Vec2::new(w, h), egui::Sense::hover()).0;

    if let Some(tex) = thumbnail {
        let uv = crate::ui::map_texture::cover_uv(rect.size(), tex.size_vec2());
        crate::ui::map_texture::draw_map_thumbnail_uv(
            ui.painter(), tex.id(), rect, uv, 1.0, CornerRadius::same(8),
        );
    } else {
        ui.painter().rect_filled(rect, 8.0, Color32::from_black_alpha(120));
        let status = if asset_loader.thumbnail_error(&config.map_name).is_some() {
            strings.no_preview.to_string()
        } else if asset_loader.thumbnail_in_flight(&config.map_name) {
            strings.loading_maps.to_string()
        } else {
            strings.no_preview.to_string()
        };
        sow_ui_kit::theme::paint_premium_glow_text(
            ui.painter(), rect.center(), egui::Align2::CENTER_CENTER,
            &status, egui::FontId::proportional(13.0), palette::text_muted(), Color32::BLACK,
        );
    }
    ui.painter().rect_stroke(
        rect, 8.0,
        Stroke::new(1.0, palette::neon_cyan_glow()),
        egui::StrokeKind::Inside,
    );

    let label = asset_loader.map_catalog.as_ref()
        .and_then(|c| sow_core::maps::catalog_lookup(c, &config.map_name))
        .map(|e| e.display_name.as_str())
        .unwrap_or(config.map_name.as_str());
    ui.add_space(4.0);
    ui.label(RichText::new(label.to_uppercase()).size(if compact { 14.0 } else { 16.0 }).strong().color(Color32::WHITE));
}

fn draw_settings_column(
    ui: &mut egui::Ui,
    config: &mut sow_core::game_config::GameConfig,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    strings: &sow_i18n::MainMenuStrings,
    item_gap: f32,
    _scale: f32,
) {
    // Map picker
    panel_card(ui, &strings.map_selection, |ui| {
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
                    }
                } else {
                    ui.label(&strings.loading_maps);
                }
            });
    });

    ui.add_space(item_gap);

    // Difficulty
    panel_card(ui, &strings.bot_difficulty, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let is_vanilla = config.bot_difficulty == sow_core::game_config::BotDifficulty::Vanilla;
            if pill_toggle(ui, "VANILLA", is_vanilla) {
                config.bot_difficulty = sow_core::game_config::BotDifficulty::Vanilla;
            }
            if pill_toggle(ui, "TERMINATOR", !is_vanilla) {
                config.bot_difficulty = sow_core::game_config::BotDifficulty::Terminator;
            }
        });
    });

    ui.add_space(item_gap);

    // Tribes + Nations side by side if wide enough
    let side_by_side = ui.available_width() > 360.0;
    if side_by_side {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = item_gap;
            let half = (ui.available_width() - item_gap) * 0.5;
            ui.allocate_ui_with_layout(Vec2::new(half, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                panel_card(ui, &strings.tribes_count, |ui| {
                    draw_custom_slider(ui, &mut config.bot_count, 0..=1000);
                });
            });
            ui.allocate_ui_with_layout(Vec2::new(half, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                panel_card(ui, &strings.nations_count, |ui| {
                    draw_custom_slider(ui, &mut config.nation_count, 0..=400);
                });
            });
        });
    } else {
        panel_card(ui, &strings.tribes_count, |ui| {
            draw_custom_slider(ui, &mut config.bot_count, 0..=1000);
        });
        ui.add_space(item_gap);
        panel_card(ui, &strings.nations_count, |ui| {
            draw_custom_slider(ui, &mut config.nation_count, 0..=400);
        });
    }

    ui.add_space(item_gap);

    // Spawn + Seed side by side if wide enough
    if side_by_side {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = item_gap;
            let half = (ui.available_width() - item_gap) * 0.5;
            ui.allocate_ui_with_layout(Vec2::new(half, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                panel_card(ui, &strings.random_spawning, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if pill_toggle(ui, "ON", config.random_spawn) {
                            config.random_spawn = true;
                        }
                        if pill_toggle(ui, "OFF", !config.random_spawn) {
                            config.random_spawn = false;
                        }
                    });
                });
            });
            ui.allocate_ui_with_layout(Vec2::new(half, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                panel_card(ui, "WORLD SEED", |ui| {
                    draw_custom_slider(ui, &mut config.seed, 1..=9999);
                });
            });
        });
    } else {
        panel_card(ui, &strings.random_spawning, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if pill_toggle(ui, "ON", config.random_spawn) {
                    config.random_spawn = true;
                }
                if pill_toggle(ui, "OFF", !config.random_spawn) {
                    config.random_spawn = false;
                }
            });
        });
        ui.add_space(item_gap);
        panel_card(ui, "WORLD SEED", |ui| {
            draw_custom_slider(ui, &mut config.seed, 1..=9999);
        });
    }
}