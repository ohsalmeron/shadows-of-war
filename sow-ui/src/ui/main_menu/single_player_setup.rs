use super::MainMenuState;
use crate::ui::theme::{screen_bg, text_secondary};
use crate::UiAction;
use egui::{Color32, CornerRadius, Frame, Margin, RichText, ScrollArea, Stroke};

fn screen_panel_frame() -> Frame {
    Frame::new()
        .fill(screen_bg())
        .inner_margin(Margin::symmetric(16, 10))
}

fn setting_card(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    let frame = egui::Frame::NONE
        .fill(crate::ui::theme::nickname_field_bg())
        .stroke(Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border()))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 8));

    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical(|ui| {
            ui.label(RichText::new(title).small().color(text_secondary()));
            ui.add_space(4.0);
            content(ui);
        });
    });
}

fn draw_custom_slider(ui: &mut egui::Ui, value: &mut u32, range: std::ops::RangeInclusive<u32>) {
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

fn draw_custom_slider_u64(ui: &mut egui::Ui, value: &mut u64, range: std::ops::RangeInclusive<u64>) {
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
) {
    state.single_player_config.player_leader = state.selected_leader;
    state.single_player_config.player_civilization = state.selected_civilization;
    let strings = &sow_i18n::get(lang).main_menu;
    let mut close = false;

    egui::Panel::top("solo_header")
        .frame(screen_panel_frame())
        .show_inside(root_ui, |ui| {
            ui.vertical(|ui| {
                ui.heading(&strings.single_player_skirmish);
                ui.label(
                    RichText::new(&strings.config_simulation)
                        .small()
                        .color(text_secondary()),
                );
            });
        });

    egui::Panel::bottom("solo_footer")
        .frame(screen_panel_frame())
        .show_inside(root_ui, |ui| {
            let narrow = ui.available_width() < 480.0;
            if narrow {
                let start_btn = crate::widgets::ThemeButton::new(&strings.start_simulation)
                    .style(crate::widgets::ThemeButtonStyle::Primary)
                    .min_size(egui::vec2(ui.available_width(), 36.0));
                if ui.add(start_btn).clicked() {
                    *action = Some(UiAction::StartSinglePlayer(Box::new(
                        *state.single_player_config.clone(),
                    )));
                    close = true;
                }
                ui.add_space(6.0);
                let cancel_btn = crate::widgets::ThemeButton::new(&strings.back)
                    .style(crate::widgets::ThemeButtonStyle::Tertiary)
                    .min_size(egui::vec2(ui.available_width(), 36.0));
                if ui.add(cancel_btn).clicked() {
                    close = true;
                }
            } else {
                ui.columns(2, |cols| {
                    let cancel_btn = crate::widgets::ThemeButton::new(&strings.back)
                        .style(crate::widgets::ThemeButtonStyle::Tertiary)
                        .min_size(egui::vec2(cols[0].available_width(), 36.0));
                    if cols[0].add(cancel_btn).clicked() {
                        close = true;
                    }
                    let start_btn = crate::widgets::ThemeButton::new(&strings.start_simulation)
                        .style(crate::widgets::ThemeButtonStyle::Primary)
                        .min_size(egui::vec2(cols[1].available_width(), 36.0));
                    if cols[1].add(start_btn).clicked() {
                        *action = Some(UiAction::StartSinglePlayer(Box::new(
                            *state.single_player_config.clone(),
                        )));
                        close = true;
                    }
                });
            }
        });

    egui::CentralPanel::default()
        .frame(Frame::new().fill(screen_bg()))
        .show_inside(root_ui, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(catalog) = &asset_loader.map_catalog {
                        state.apply_map_catalog(catalog);
                    }
                    let map_name = state.single_player_config.map_name.clone();
                    asset_loader.request_thumbnail(&map_name);
                    let loader = &*asset_loader;
                    let config = &mut state.single_player_config;
                    let item_gap = 10.0;

                    let draw_preview = |ui: &mut egui::Ui,
                                        config: &sow_core::game_config::GameConfig| {
                        let thumbnail = loader.thumbnail(&config.map_name);
                        let aspect = 1.77_f32;
                        let w = ui.available_width();
                        let h = (w / aspect).min(200.0).max(48.0);
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
                            ui.painter()
                                .rect_filled(rect, 8.0, Color32::from_black_alpha(120));
                            let status = if let Some(err) = loader.thumbnail_error(&config.map_name)
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
                                egui::FontId::proportional(13.0),
                                text_secondary(),
                                Color32::BLACK,
                            );
                        }

                        ui.painter().rect_stroke(
                            rect,
                            8.0_f32,
                            Stroke::new(1.0_f32, crate::ui::theme::menu_panel_border_glow()),
                            egui::StrokeKind::Inside,
                        );
                    };

                    let draw_map_picker =
                        |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                            setting_card(ui, &strings.map_selection, |ui| {
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
                                    .selected_text(selected_label)
                                    .show_ui(ui, |ui| {
                                        if let Some(catalog) = &loader.map_catalog {
                                            if catalog.is_empty() {
                                                ui.label(&strings.no_maps_found);
                                            } else {
                                                for map_entry in catalog {
                                                    let label = if loader.has_map(&map_entry.key) {
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
                        };

                    let draw_leader_picker =
                        |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                            setting_card(ui, "ACTIVE LEADER & CIVILIZATION", |ui| {
                                let leader = config.player_leader;
                                let civ = config.player_civilization;
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(leader.menu_emoji()).size(18.0));
                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "{} ({})",
                                                leader.name(),
                                                civ.name()
                                            ))
                                            .strong(),
                                        );
                                        ui.label(
                                            RichText::new(leader.perk_description())
                                                .small()
                                                .color(crate::ui::theme::accent_solo_cyan()),
                                        );
                                    });
                                });
                            });
                        };

                    let draw_bots =
                        |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                            setting_card(ui, &strings.tribes_count, |ui| {
                                draw_custom_slider(ui, &mut config.bot_count, 0..=1000);
                            });
                        };

                    let draw_nations =
                        |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                            setting_card(ui, &strings.nations_count, |ui| {
                                draw_custom_slider(ui, &mut config.nation_count, 0..=400);
                            });
                        };

                    let draw_spawn =
                        |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                            setting_card(ui, &strings.random_spawning, |ui| {
                                let btn_text = if config.random_spawn {
                                    "ON"
                                } else {
                                    "OFF"
                                };
                                let btn = egui::Button::new(btn_text).fill(if config.random_spawn {
                                    crate::ui::theme::accent_solo_cyan()
                                } else {
                                    crate::ui::theme::menu_secondary_button()
                                });
                                if ui.add(btn).clicked() {
                                    config.random_spawn = !config.random_spawn;
                                }
                            });
                        };

                    let draw_seed_picker =
                        |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                            setting_card(ui, "WORLD SEED", |ui| {
                                draw_custom_slider_u64(ui, &mut config.seed, 1..=9999);
                            });
                        };

                    let two_col = ui.available_width() >= 720.0;
                    if two_col {
                        ui.columns(2, |cols| {
                            cols[0].spacing_mut().item_spacing.y = item_gap;
                            draw_preview(&mut cols[0], config);
                            draw_map_picker(&mut cols[0], config);
                            draw_diff(&mut cols[0], config);
                            draw_seed_picker(&mut cols[0], config);

                            cols[1].spacing_mut().item_spacing.y = item_gap;
                            draw_leader_picker(&mut cols[1], config);
                            draw_bots(&mut cols[1], config);
                            draw_nations(&mut cols[1], config);
                            draw_spawn(&mut cols[1], config);
                        });
                    } else {
                        draw_leader_picker(ui, config);
                        draw_preview(ui, config);
                        draw_map_picker(ui, config);
                        draw_diff(ui, config);
                        draw_bots(ui, config);
                        draw_nations(ui, config);
                        draw_spawn(ui, config);
                        draw_seed_picker(ui, config);
                    }
                });
        });

    if close {
        state.show_single_player_setup = false;
    }
}
