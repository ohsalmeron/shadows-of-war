use super::MainMenuState;
use crate::ui::theme;
use crate::UiAction;
use egui::{Color32, CornerRadius, Margin, RichText, ScrollArea, Stroke};

fn setting_card(
    ui: &mut egui::Ui,
    title: &str,
    is_mobile: bool,
    content: impl FnOnce(&mut egui::Ui),
) {
    let frame = egui::Frame::NONE
        .fill(theme::nickname_field_bg())
        .stroke(egui::Stroke::new(1.0_f32, theme::nickname_field_border()))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(14, if is_mobile { 10 } else { 8 }));

    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical(|ui| {
            crate::ui::theme::outlined_label(
                ui,
                title,
                egui::FontId::proportional(15.0),
                theme::text_secondary(),
            );
            ui.add_space(4.0);
            content(ui);
        });
    });
}

fn draw_custom_slider(ui: &mut egui::Ui, value: &mut u32, range: std::ops::RangeInclusive<u32>) {
    ui.horizontal_centered(|ui| {
        ui.set_width(ui.available_width());
        let total_w = ui.available_width();
        let qty_w = 64.0;
        let spacing = 12.0;
        let slider_w = (total_w - qty_w - spacing - 8.0).max(50.0);

        ui.scope(|ui| {
            ui.spacing_mut().slider_rail_height = 16.0;
            ui.spacing_mut().slider_width = slider_w;
            ui.spacing_mut().interact_size.y = 32.0;

            ui.add(
                egui::Slider::new(value, range)
                    .show_value(false)
                    .trailing_fill(true),
            );
        });

        ui.add_space(spacing);

        let frame = egui::Frame::NONE
            .fill(theme::nickname_field_bg())
            .stroke(egui::Stroke::new(1.0_f32, theme::nickname_field_border()))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(10, 6));

        frame.show(ui, |ui| {
            ui.set_min_size(egui::vec2(qty_w, 32.0));
            ui.vertical_centered(|ui| {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(value.to_string())
                        .font(egui::FontId::proportional(16.0))
                        .color(Color32::WHITE)
                        .strong(),
                );
            });
        });
    });
}

fn draw_custom_slider_u64(ui: &mut egui::Ui, value: &mut u64, range: std::ops::RangeInclusive<u64>) {
    ui.horizontal_centered(|ui| {
        ui.set_width(ui.available_width());
        let total_w = ui.available_width();
        let qty_w = 64.0;
        let spacing = 12.0;
        let slider_w = (total_w - qty_w - spacing - 8.0).max(50.0);

        ui.scope(|ui| {
            ui.spacing_mut().slider_rail_height = 16.0;
            ui.spacing_mut().slider_width = slider_w;
            ui.spacing_mut().interact_size.y = 32.0;

            ui.add(
                egui::Slider::new(value, range)
                    .show_value(false)
                    .trailing_fill(true),
            );
        });

        ui.add_space(spacing);

        let frame = egui::Frame::NONE
            .fill(theme::nickname_field_bg())
            .stroke(egui::Stroke::new(1.0_f32, theme::nickname_field_border()))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(10, 6));

        frame.show(ui, |ui| {
            ui.set_min_size(egui::vec2(qty_w, 32.0));
            ui.vertical_centered(|ui| {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(value.to_string())
                        .font(egui::FontId::proportional(16.0))
                        .color(Color32::WHITE)
                        .strong(),
                );
            });
        });
    });
}


pub fn draw_modal(
    ctx: &egui::Context,
    state: &mut MainMenuState,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    action: &mut Option<UiAction>,
    lang: sow_i18n::Language,
    is_mobile: bool,
) {
    state.single_player_config.player_leader = state.selected_leader;
    state.single_player_config.player_civilization = state.selected_civilization;
    let strings = &sow_i18n::get(lang).main_menu;
    let mut close = false;
    let scale = theme::viewport_scale(ctx);

    egui::Area::new(egui::Id::new("single_player_setup_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let screen_rect = ctx.content_rect();
            ui.painter()
                .rect_filled(screen_rect, 0.0, crate::ui::theme::menu_backdrop());
        });

    let screen_rect = ctx.content_rect();
    let pad = if is_mobile { 32.0 } else { 50.0 };
    let panel_size = if is_mobile {
        screen_rect.size()
    } else {
        screen_rect.size() - egui::vec2(pad, pad)
    };
    let action_bar_h = if is_mobile {
        (44.0 + 8.0 + 44.0) * scale
    } else {
        62.0 * scale
    };
    let section_gap = 12.0 * scale;
    let item_gap = 12.0 * scale;
    let use_two_columns = !is_mobile || screen_rect.height() < 520.0;

    let anchor = if is_mobile {
        egui::Align2::LEFT_TOP
    } else {
        egui::Align2::CENTER_CENTER
    };

    egui::Window::new("single_player_setup_modal")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .order(egui::Order::Foreground)
        .anchor(anchor, egui::vec2(0.0, 0.0))
        .fixed_size(panel_size)
        .frame(theme::standard_panel_frame(is_mobile))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                crate::ui::theme::outlined_label(
                    ui,
                    &strings.single_player_skirmish,
                    egui::FontId::proportional(if is_mobile { 24.0 } else { 32.0 } * scale),
                    Color32::WHITE,
                );
                ui.label(
                    egui::RichText::new(&strings.config_simulation)
                        .size(12.0 * scale)
                        .color(theme::text_secondary())
                        .strong(),
                );
            });

            ui.add_space(section_gap);

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height((ui.available_height() - action_bar_h - section_gap).max(0.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, item_gap);
                    if let Some(catalog) = &asset_loader.map_catalog {
                        state.apply_map_catalog(catalog);
                    }
                    let map_name = state.single_player_config.map_name.clone();
                    asset_loader.request_thumbnail(&map_name);
                    let loader = &*asset_loader;
                    let config = &mut state.single_player_config;

                    let draw_preview =
                        |ui: &mut egui::Ui,
                         config: &sow_core::game_config::GameConfig,
                         max_h: f32| {
                            let thumbnail = loader.thumbnail(&config.map_name);
                            let aspect = 1.77_f32;
                            let w = ui.available_width();
                            let h = (w / aspect).min(max_h.max(48.0));
                            let rect = ui
                                .allocate_exact_size(egui::vec2(w, h), egui::Sense::hover())
                                .0;

                            if let Some(tex) = thumbnail {
                                crate::ui::map_texture::draw_map_thumbnail(
                                    ui.painter(),
                                    tex.id(),
                                    rect,
                                    1.0,
                                );
                            } else {
                                ui.painter().rect_filled(
                                    rect,
                                    12.0,
                                    Color32::from_black_alpha(120),
                                );
                                let status = if let Some(err) =
                                    loader.thumbnail_error(&config.map_name)
                                {
                                    format!("Thumbnail: {err}")
                                } else if loader.thumbnail_in_flight(&config.map_name) {
                                    strings.loading_maps.to_string()
                                } else {
                                    strings.no_preview.to_string()
                                };
                                crate::ui::theme::outlined_text(
                                    ui.painter(),
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    &status,
                                    egui::FontId::proportional(14.0),
                                    theme::text_secondary(),
                                    Color32::BLACK,
                                );
                            }

                            ui.painter().rect_stroke(
                                rect,
                                12.0_f32,
                                Stroke::new(1.5_f32, theme::menu_panel_border_glow()),
                                egui::StrokeKind::Inside,
                            );
                        };

                    let draw_map_picker =
                        |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                            setting_card(ui, &strings.map_selection, is_mobile, |ui| {
                                ui.style_mut().spacing.button_padding = egui::vec2(14.0, 8.0);
                                let selected_label = loader
                                    .map_catalog
                                    .as_ref()
                                    .and_then(|c| {
                                        sow_core::maps::catalog_lookup(c, &config.map_name)
                                    })
                                    .map(|e| e.display_name.as_str())
                                    .unwrap_or(config.map_name.as_str());
                                egui::ComboBox::from_id_salt("sp_map")
                                    .width(ui.available_width())
                                    .selected_text(RichText::new(selected_label).size(16.0))
                                    .show_ui(ui, |ui| {
                                        if let Some(catalog) = &loader.map_catalog {
                                            if catalog.is_empty() {
                                                ui.label(&strings.no_maps_found);
                                            } else {
                                                for map_entry in catalog {
                                                    let label = if loader.has_map(&map_entry.key)
                                                    {
                                                        format!(
                                                            "{}{}",
                                                            map_entry.display_name,
                                                            strings.map_offline_tag
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
                        };

                    let draw_diff =
                        |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                            setting_card(ui, &strings.bot_difficulty, is_mobile, |ui| {
                                ui.style_mut().spacing.button_padding = egui::vec2(14.0, 8.0);
                                egui::ComboBox::from_id_salt("sp_diff")
                                    .width(ui.available_width())
                                    .selected_text(
                                        RichText::new(format!("{:?}", config.bot_difficulty))
                                            .size(16.0),
                                    )
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
                        };

                    let draw_leader_picker =
                        |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                            setting_card(ui, "ACTIVE LEADER & CIVILIZATION", is_mobile, |ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 4.0;

                                    let leader = config.player_leader;
                                    let civ = config.player_civilization;
                                    let emoji = leader.menu_emoji();

                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(emoji).size(20.0));
                                        ui.add_space(4.0);
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} ({})",
                                                    leader.name(),
                                                    civ.name()
                                                ))
                                                .strong()
                                                .color(Color32::WHITE)
                                                .size(13.0),
                                            );
                                            ui.label(
                                                RichText::new(leader.perk_description())
                                                    .size(10.5)
                                                    .color(theme::accent_solo_cyan())
                                                    .strong(),
                                            );
                                        });
                                    });
                                });
                            });
                        };

                    let draw_bots =
                        |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                            setting_card(ui, &strings.tribes_count, is_mobile, |ui| {
                                draw_custom_slider(ui, &mut config.bot_count, 0..=1000);
                            });
                        };

                    let draw_nations =
                        |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                            setting_card(ui, &strings.nations_count, is_mobile, |ui| {
                                draw_custom_slider(ui, &mut config.nation_count, 0..=400);
                            });
                        };

                    let draw_spawn =
                        |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                            setting_card(ui, &strings.random_spawning, is_mobile, |ui| {
                                let btn_text = if config.random_spawn {
                                    "ON ✔"
                                } else {
                                    "OFF ❌"
                                };
                                let btn =
                                    egui::Button::new(RichText::new(btn_text).strong().size(14.0))
                                        .fill(if config.random_spawn {
                                            theme::accent_solo_cyan()
                                        } else {
                                            theme::menu_secondary_button()
                                        })
                                        .corner_radius(12.0)
                                        .min_size(egui::vec2(100.0, 36.0 * scale));

                                if ui.add(btn).clicked() {
                                    config.random_spawn = !config.random_spawn;
                                }
                            });
                        };

                    let draw_seed_picker =
                        |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                            setting_card(ui, "WORLD SEED", is_mobile, |ui| {
                                draw_custom_slider_u64(ui, &mut config.seed, 1..=9999);
                            });
                        };

                    let scroll_h = ui.available_height();
                    let preview_max_h = if use_two_columns {
                        (scroll_h * 0.38).max(60.0)
                    } else {
                        (scroll_h * 0.22).max(48.0)
                    };

                    if use_two_columns {
                        ui.horizontal(|ui| {
                            ui.set_width(ui.available_width());
                            let col_gap = 24.0;
                            let col_w = ((ui.available_width() - col_gap) / 2.0).max(0.0);

                            ui.vertical(|ui| {
                                ui.set_width(col_w);
                                ui.spacing_mut().item_spacing.y = item_gap;
                                draw_preview(ui, config, preview_max_h);
                                draw_map_picker(ui, config);
                                draw_diff(ui, config);
                                draw_seed_picker(ui, config);
                            });

                            ui.add_space(col_gap);

                            ui.vertical(|ui| {
                                ui.set_width(col_w);
                                ui.spacing_mut().item_spacing.y = item_gap;
                                draw_leader_picker(ui, config);
                                draw_bots(ui, config);
                                draw_nations(ui, config);
                                draw_spawn(ui, config);
                            });
                        });
                    } else {
                        draw_leader_picker(ui, config);
                        draw_preview(ui, config, preview_max_h);
                        draw_map_picker(ui, config);
                        draw_diff(ui, config);
                        draw_bots(ui, config);
                        draw_nations(ui, config);
                        draw_spawn(ui, config);
                        draw_seed_picker(ui, config);
                    }
                });

            ui.add_space(section_gap);

            let btn_h = 44.0 * scale;
            let (btn_w, _) = if is_mobile {
                (ui.available_width(), btn_h)
            } else {
                (220.0, 50.0 * scale)
            };

            ui.vertical_centered(|ui| {
                if !is_mobile {
                    ui.horizontal(|ui| {
                        let spacing = 24.0;
                        let total_w = btn_w * 2.0 + spacing;
                        let remaining = (ui.available_width() - total_w) / 2.0;
                        ui.add_space(remaining);

                        let cancel_btn = crate::widgets::ThemeButton::new(&strings.back)
                            .style(crate::widgets::ThemeButtonStyle::Tertiary)
                            .min_size(egui::vec2(btn_w, btn_h.max(50.0 * scale)))
                            .text_size(18.0);

                        if ui.add(cancel_btn).clicked() {
                            close = true;
                        }

                        ui.add_space(spacing);

                        let start_btn =
                            crate::widgets::ThemeButton::new(&strings.start_simulation)
                                .style(crate::widgets::ThemeButtonStyle::Primary)
                                .min_size(egui::vec2(btn_w, btn_h.max(50.0 * scale)))
                                .text_size(18.0);

                        if ui.add(start_btn).clicked() {
                            *action = Some(UiAction::StartSinglePlayer(Box::new(
                                *state.single_player_config.clone(),
                            )));
                            close = true;
                        }
                    });
                } else {
                    let start_btn =
                        crate::widgets::ThemeButton::new(&strings.start_simulation)
                            .style(crate::widgets::ThemeButtonStyle::Primary)
                            .min_size(egui::vec2(btn_w, btn_h))
                            .text_size(18.0);

                    if ui.add(start_btn).clicked() {
                        *action = Some(UiAction::StartSinglePlayer(Box::new(
                            *state.single_player_config.clone(),
                        )));
                        close = true;
                    }

                    ui.add_space(8.0);

                    let cancel_btn = crate::widgets::ThemeButton::new(&strings.back)
                        .style(crate::widgets::ThemeButtonStyle::Tertiary)
                        .min_size(egui::vec2(btn_w, btn_h))
                        .text_size(18.0);

                    if ui.add(cancel_btn).clicked() {
                        close = true;
                    }
                }
            });
        });

    if close {
        state.show_single_player_setup = false;
    }
}
