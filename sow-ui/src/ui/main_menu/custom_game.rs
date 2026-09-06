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
    ui.style_mut().spacing.button_padding = egui::vec2(8.0, 4.0);
    let btn = egui::Button::new(RichText::new(label).size(12.0).color(text).strong())
        .fill(bg)
        .stroke(Stroke::new(
            1.0_f32,
            if active {
                palette::neon_cyan()
            } else {
                palette::field_border()
            },
        ))
        .corner_radius(CornerRadius::same(6))
        .min_size(Vec2::new(0.0, 44.0));
    ui.add(btn).clicked()
}

fn pill_row(ui: &mut egui::Ui, options: &[&str], selected: usize) -> Option<usize> {
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
            ui.set_max_width(ui.available_width());
            ui.set_width(ui.available_width());
            ui.vertical(|ui| content(ui));
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
    _reduced_motion: bool,
) {
    if root_ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        state.go_home();
    }

    let strings = &sow_i18n::get(lang).main_menu;
    let compact = super::layout::main_menu_metrics(root_ui.ctx()).is_compact();

    // Sync leader + catalog
    state.custom_game_config.player_leader = state.selected_leader;
    state.custom_game_config.player_civilization = state.selected_civilization;
    if let Some(catalog) = &asset_loader.map_catalog {
        let cfg = &mut state.custom_game_config;
        cfg.map_name = sow_core::maps::resolve_map_name(catalog, &cfg.map_name);
        sow_core::maps::apply_catalog_dimensions(
            catalog,
            &mut cfg.map_name,
            &mut cfg.map_width,
            &mut cfg.map_height,
        );
    }

    // Only lobby maps get their thumbnails prefetched (via get_assets_to_fetch), so a
    // catalog map picked here would otherwise never load its preview. Request the
    // selected map's thumbnail on demand; request_thumbnail is idempotent and we skip
    // maps already loaded, in-flight, or previously failed to avoid re-fetch churn.
    {
        let map_name = state.custom_game_config.map_name.clone();
        if asset_loader.thumbnail(&map_name).is_none()
            && !asset_loader.thumbnail_in_flight(&map_name)
            && asset_loader.thumbnail_error(&map_name).is_none()
        {
            asset_loader.request_thumbnail(&map_name);
        }
    }

    // ── Top: title + SP/MP toggle ─────────────────────────────────
    Frame::NONE
        .inner_margin(egui::Margin::symmetric(16, 8))
        .show(root_ui, |ui| {
            if compact {
                ui.vertical(|ui| {
                    ui.heading(&strings.custom_game_title);
                    ui.add_space(4.0);
                    mode_toggle(ui, state, strings);
                });
            } else {
                ui.horizontal(|ui| {
                    ui.heading(&strings.custom_game_title);
                    ui.add_space(16.0);
                    mode_toggle(ui, state, strings);
                });
            }
        });

    // ── Central: settings panels ───────────────────────────────────
    Frame::NONE
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(root_ui, |ui| {
            let is_sp = state.custom_game_is_sp;
            let item_gap = if compact { 4.0 } else { 8.0 };

            // Outcome hint below the toggle
            let hint = if is_sp {
                &strings.custom_offline_hint
            } else {
                &strings.custom_online_hint
            };
            ui.label(
                RichText::new(hint)
                    .size(11.0)
                    .color(palette::text_muted())
                    .italics(),
            );
            ui.add_space(item_gap * 2.0);

            if compact {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        draw_map_preview(ui, state, asset_loader, strings);
                        ui.add_space(item_gap);
                        draw_map_selection_card(ui, state, asset_loader, strings);
                        ui.add_space(item_gap);
                        draw_difficulty_spawning_card(ui, state, strings);
                        ui.add_space(item_gap);
                        if is_sp {
                            draw_seed_card(ui, state, strings);
                            ui.add_space(item_gap);
                        } else {
                            draw_visibility_card(ui, state, strings);
                            ui.add_space(item_gap);
                        }
                        draw_sliders_card(ui, state, strings);
                        ui.add_space(12.0);
                        draw_action_button(ui, state, action, is_sp, strings, compact);
                    });
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("custom_game_body_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.columns(2, |columns| {
                            columns[0].vertical(|ui| {
                                draw_map_preview(ui, state, asset_loader, strings);
                                ui.add_space(item_gap);
                                draw_map_selection_card(ui, state, asset_loader, strings);
                                ui.add_space(item_gap);
                                draw_difficulty_spawning_card(ui, state, strings);
                                if is_sp {
                                    ui.add_space(item_gap);
                                    draw_seed_card(ui, state, strings);
                                } else {
                                    ui.add_space(item_gap);
                                    draw_visibility_card(ui, state, strings);
                                }
                            });
                            columns[1].vertical(|ui| {
                                draw_sliders_card(ui, state, strings);
                                ui.add_space(16.0);
                                draw_action_button(ui, state, action, is_sp, strings, compact);
                            });
                        });
                    });
            }
        });
}

fn mode_toggle(ui: &mut egui::Ui, state: &mut MainMenuState, strings: &sow_i18n::MainMenuStrings) {
    ui.horizontal(|ui| {
        if pill_toggle(ui, &strings.custom_offline_toggle, state.custom_game_is_sp) {
            state.custom_game_is_sp = true;
            state.custom_game_password.clear();
            state.custom_game_is_private = false;
        }
        if pill_toggle(ui, &strings.custom_online_toggle, !state.custom_game_is_sp) {
            state.custom_game_is_sp = false;
        }
    });
}

fn draw_action_button(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    action: &mut Option<UiAction>,
    is_sp: bool,
    strings: &sow_i18n::MainMenuStrings,
    _compact: bool,
) {
    let btn_label = if is_sp {
        &strings.start_simulation
    } else {
        &strings.create_lobby_btn
    };
    let start = crate::widgets::ThemeButton::new(btn_label)
        .style(crate::widgets::ThemeButtonStyle::Secondary)
        .min_size(Vec2::new(ui.available_width(), 52.0))
        .text_size(20.0);
    if ui.add(start).clicked() {
        action_btn_clicked(state, action, is_sp);
    }
}

fn action_btn_clicked(state: &mut MainMenuState, action: &mut Option<UiAction>, is_sp: bool) {
    if is_sp {
        let cfg = *state.custom_game_config.clone();
        state.go_home();
        *action = Some(UiAction::StartSinglePlayer(Box::new(cfg)));
    } else {
        let cfg = *state.custom_game_config.clone();
        let password = if state.custom_game_password.is_empty() {
            None
        } else {
            Some(state.custom_game_password.clone())
        };
        state.go_home();
        *action = Some(UiAction::CreateGame {
            config: Box::new(cfg),
            is_private: state.custom_game_is_private,
            password,
        });
    }
}

fn draw_map_preview(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    strings: &sow_i18n::MainMenuStrings,
) {
    let config = &state.custom_game_config;
    let compact = super::layout::main_menu_metrics(ui.ctx()).is_compact();
    let thumbnail = asset_loader.thumbnail(&config.map_name);
    let w = ui.available_width();
    let h = (w / 1.77).clamp(40.0, if compact { 90.0 } else { 160.0 });
    let rect = ui
        .allocate_exact_size(Vec2::new(w, h), egui::Sense::hover())
        .0;

    if let Some(tex) = thumbnail {
        let uv = crate::ui::map_texture::cover_uv(rect.size(), tex.size_vec2());
        crate::ui::map_texture::draw_map_thumbnail_uv(
            ui.painter(),
            tex.id(),
            rect,
            uv,
            1.0,
            CornerRadius::same(6),
        );
    } else {
        ui.painter()
            .rect_filled(rect, 6.0, Color32::from_black_alpha(120));
        let status = if asset_loader.thumbnail_error(&config.map_name).is_some() {
            strings.no_preview.to_string()
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
            egui::FontId::proportional(12.0),
            palette::text_muted(),
            Color32::BLACK,
        );
    }
    ui.painter().rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0_f32, palette::neon_cyan_glow()),
        egui::StrokeKind::Inside,
    );

    let label = asset_loader
        .map_catalog
        .as_ref()
        .and_then(|c| sow_core::maps::catalog_lookup(c, &config.map_name))
        .map(|e| e.display_name.as_str())
        .unwrap_or(config.map_name.as_str());
    ui.add_space(4.0);
    ui.label(
        RichText::new(label.to_uppercase())
            .size(13.0)
            .strong()
            .color(Color32::WHITE),
    );
}

fn draw_map_selection_card(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    strings: &sow_i18n::MainMenuStrings,
) {
    panel_card(ui, |ui| {
        let config = &mut state.custom_game_config;
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
                ui.style_mut().override_text_style = Some(egui::TextStyle::Small);
                if let Some(catalog) = &asset_loader.map_catalog {
                    for map_entry in catalog {
                        ui.selectable_value(
                            &mut config.map_name,
                            map_entry.key.clone(),
                            &map_entry.display_name,
                        );
                    }
                } else {
                    ui.label(&strings.loading_maps);
                }
            });
    });
}

fn draw_difficulty_spawning_card(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    strings: &sow_i18n::MainMenuStrings,
) {
    panel_card(ui, |ui| {
        let config = &mut state.custom_game_config;
        let side = ui.available_width() > 360.0;
        if side {
            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    ui.label(
                        RichText::new(&strings.bot_difficulty)
                            .size(10.0)
                            .color(palette::text_muted()),
                    );
                    ui.add_space(3.0);
                    let idx = if config.bot_difficulty
                        == sow_core::game_config::BotDifficulty::Terminator
                    {
                        1
                    } else {
                        0
                    };
                    if let Some(n) = pill_row(ui, &["VANILLA", "TERMINATOR"], idx) {
                        config.bot_difficulty = if n == 1 {
                            sow_core::game_config::BotDifficulty::Terminator
                        } else {
                            sow_core::game_config::BotDifficulty::Vanilla
                        };
                    }
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(&strings.random_spawning)
                            .size(10.0)
                            .color(palette::text_muted()),
                    );
                    ui.add_space(3.0);
                    if let Some(n) =
                        pill_row(ui, &["ON", "OFF"], if config.random_spawn { 0 } else { 1 })
                    {
                        config.random_spawn = n == 0;
                    }
                });
                cols[1].vertical(|ui| {
                    ui.label(
                        RichText::new(&strings.game_mode_label)
                            .size(10.0)
                            .color(palette::text_muted()),
                    );
                    ui.add_space(3.0);
                    let mode_idx = if config.game_mode == "Teams" {
                        1
                    } else if config.game_mode == "HumansVsNations" {
                        2
                    } else {
                        0
                    };
                    if let Some(n) = pill_row(ui, &["FFA", "TEAMS", "HVN"], mode_idx) {
                        config.game_mode = match n {
                            1 => "Teams".to_string(),
                            2 => "HumansVsNations".to_string(),
                            _ => "FFA".to_string(),
                        };
                    }
                });
            });
        } else {
            ui.label(
                RichText::new(&strings.bot_difficulty)
                    .size(10.0)
                    .color(palette::text_muted()),
            );
            ui.add_space(3.0);
            let idx = if config.bot_difficulty == sow_core::game_config::BotDifficulty::Terminator {
                1
            } else {
                0
            };
            if let Some(n) = pill_row(ui, &["VANILLA", "TERMINATOR"], idx) {
                config.bot_difficulty = if n == 1 {
                    sow_core::game_config::BotDifficulty::Terminator
                } else {
                    sow_core::game_config::BotDifficulty::Vanilla
                };
            }
            ui.add_space(6.0);
            ui.label(
                RichText::new(&strings.random_spawning)
                    .size(10.0)
                    .color(palette::text_muted()),
            );
            ui.add_space(3.0);
            if let Some(n) = pill_row(ui, &["ON", "OFF"], if config.random_spawn { 0 } else { 1 }) {
                config.random_spawn = n == 0;
            }
            ui.add_space(6.0);
            ui.label(
                RichText::new(&strings.game_mode_label)
                    .size(10.0)
                    .color(palette::text_muted()),
            );
            ui.add_space(3.0);
            let mode_idx = if config.game_mode == "Teams" { 1 } else { 0 };
            if let Some(n) = pill_row(ui, &["FFA", "TEAMS"], mode_idx) {
                config.game_mode = if n == 1 {
                    "Teams".to_string()
                } else {
                    "FFA".to_string()
                };
            }
        }
    });
}

fn draw_sliders_card(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    strings: &sow_i18n::MainMenuStrings,
) {
    panel_card(ui, |ui| {
        let config = &mut state.custom_game_config;
        ui.label(
            RichText::new("MAX PLAYERS")
                .size(10.0)
                .color(palette::text_muted()),
        );
        ui.add_space(2.0);
        draw_custom_slider(ui, &mut config.max_players, 2..=16);

        ui.add_space(6.0);
        ui.label(
            RichText::new(&strings.tribes_count)
                .size(10.0)
                .color(palette::text_muted()),
        );
        ui.add_space(2.0);
        draw_custom_slider(ui, &mut config.bot_count, 0..=1000);

        ui.add_space(6.0);
        ui.label(
            RichText::new(&strings.nations_count)
                .size(10.0)
                .color(palette::text_muted()),
        );
        ui.add_space(2.0);
        draw_custom_slider(ui, &mut config.nation_count, 0..=400);
    });
}

fn draw_seed_card(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    strings: &sow_i18n::MainMenuStrings,
) {
    panel_card(ui, |ui| {
        let config = &mut state.custom_game_config;
        ui.label(
            RichText::new(&strings.seed_label)
                .size(10.0)
                .color(palette::text_muted()),
        );
        ui.add_space(2.0);
        draw_custom_slider(ui, &mut config.seed, 1..=9999);
    });
}

fn draw_visibility_card(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    strings: &sow_i18n::MainMenuStrings,
) {
    panel_card(ui, |ui| {
        ui.label(
            RichText::new(&strings.visibility_label)
                .size(10.0)
                .color(palette::text_muted()),
        );
        ui.add_space(3.0);
        let vis_idx = if state.custom_game_is_private { 1 } else { 0 };
        if let Some(n) = pill_row(
            ui,
            &[&strings.visibility_public, &strings.visibility_private],
            vis_idx,
        ) {
            state.custom_game_is_private = n == 1;
        }

        if state.custom_game_is_private {
            ui.add_space(6.0);
            ui.label(
                RichText::new(&strings.password_label)
                    .size(10.0)
                    .color(palette::text_muted()),
            );
            ui.add_space(3.0);
            let frame = Frame::NONE
                .fill(palette::field_bg())
                .stroke(Stroke::new(1.0_f32, palette::field_border()))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(8, 4));
            frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(
                    egui::TextEdit::singleline(&mut state.custom_game_password)
                        .hint_text(&strings.password_hint)
                        .password(true)
                        .desired_width(f32::INFINITY)
                        .frame(Frame::NONE)
                        .font(egui::FontId::proportional(12.0))
                        .text_color(egui::Color32::WHITE),
                );
            });
        }
    });
}
