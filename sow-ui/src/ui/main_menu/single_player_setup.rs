use crate::UiAction;
use egui::{Color32, CornerRadius, RichText, Stroke, Margin};
use crate::ui::theme;
use super::MainMenuState;

fn setting_card(ui: &mut egui::Ui, title: &str, _is_mobile: bool, content: impl FnOnce(&mut egui::Ui)) {
    let frame = egui::Frame::NONE
        .fill(theme::nickname_field_bg())
        .stroke(egui::Stroke::new(1.0_f32, theme::nickname_field_border()))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(16, 12));

    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical(|ui| {
            crate::ui::theme::outlined_label(ui, title, egui::FontId::proportional(16.0), theme::text_secondary());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                content(ui);
            });
        });
    });
}

pub fn draw_modal(
    ctx: &egui::Context,
    state: &mut MainMenuState,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    action: &mut Option<UiAction>,
    lang: sow_lang::Language,
) {
    let strings = &sow_lang::get(lang).main_menu;
    let mut close = false;

    egui::Area::new(egui::Id::new("single_player_setup_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let screen_rect = ctx.content_rect();
            let screen_w = screen_rect.width();
            let screen_h = screen_rect.height();
            let is_mobile = screen_w < 720.0;

            // 1. Fullscreen Background Image
            let background_tex = if is_mobile {
                asset_loader.splash_mobile.as_ref()
            } else {
                asset_loader.splash_desktop.as_ref()
            };

            if let Some(texture) = background_tex {
                let tex_aspect = texture.size()[0] as f32 / texture.size()[1] as f32;
                let screen_aspect = screen_w / screen_h;

                let (mut u0, mut v0, mut u1, mut v1) = (0.0, 0.0, 1.0, 1.0);

                if tex_aspect > screen_aspect {
                    let crop_w = screen_aspect / tex_aspect;
                    u0 = (1.0 - crop_w) / 2.0;
                    u1 = 1.0 - u0;
                } else {
                    let crop_h = tex_aspect / screen_aspect;
                    v0 = (1.0 - crop_h) / 2.0;
                    v1 = 1.0 - v0;
                }

                ui.painter().image(
                    texture.id(),
                    screen_rect,
                    egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1)),
                    Color32::WHITE,
                );
            }

            // 2. Dimming backdrop overlay
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                Color32::from_rgba_unmultiplied(8, 12, 24, 215),
            );

            // 3. Central Modal Panel
            let panel_w = if is_mobile {
                screen_w - 32.0
            } else {
                840.0f32.min(screen_w - 64.0)
            };
            let panel_h = 750.0f32.min(screen_h - 32.0);
            let modal_size = egui::vec2(panel_w, panel_h);
            let modal_rect = egui::Rect::from_center_size(screen_rect.center(), modal_size);

            ui.scope_builder(egui::UiBuilder::new().max_rect(modal_rect), |ui| {
                egui::Frame::NONE
                    .fill(theme::panel_bg())
                    .stroke(egui::Stroke::new(1.5_f32, theme::menu_panel_border_glow()))
                    .corner_radius(CornerRadius::same(20))
                    .inner_margin(if is_mobile { 16.0 } else { 28.0 })
                    .shadow(egui::Shadow {
                        blur: 32,
                        spread: 0,
                        color: theme::menu_panel_border_glow().linear_multiply(0.25),
                        offset: [0, 10],
                    })
                    .show(ui, |ui| {
                        ui.set_min_size(ui.available_size());

                        // Header Info
                        ui.vertical_centered(|ui| {
                            crate::ui::theme::outlined_label(
                                ui,
                                &strings.single_player_skirmish,
                                egui::FontId::proportional(if is_mobile { 24.0 } else { 32.0 }),
                                Color32::WHITE,
                            );
                            ui.label(
                                egui::RichText::new(&strings.config_simulation)
                                    .size(12.0)
                                    .color(theme::text_secondary())
                                    .strong(),
                            );
                        });

                        ui.add_space(20.0);

                        // Content Scroll Area
                        egui::ScrollArea::vertical()
                            .auto_shrink(false)
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(0.0, 16.0);
                                let config = &mut state.single_player_config;

                                let draw_map_picker = |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                                    setting_card(ui, &strings.map_selection, is_mobile, |ui| {
                                        ui.style_mut().spacing.button_padding = egui::vec2(16.0, 10.0);
                                        egui::ComboBox::from_id_salt("sp_map")
                                            .width(ui.available_width() - 8.0)
                                            .selected_text(RichText::new(&config.map_name).size(16.0))
                                            .show_ui(ui, |ui| {
                                                if let Some(catalog) = &asset_loader.map_catalog {
                                                    if catalog.is_empty() {
                                                        ui.label(&strings.no_maps_found);
                                                    } else {
                                                        for map_entry in catalog {
                                                            let display_name = &map_entry.name;
                                                            ui.selectable_value(&mut config.map_name, display_name.to_string(), display_name);
                                                        }
                                                    }
                                                } else {
                                                    ui.label(&strings.loading_maps);
                                                }
                                            });
                                    });
                                };

                                let draw_diff = |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                                    setting_card(ui, &strings.bot_difficulty, is_mobile, |ui| {
                                        ui.style_mut().spacing.button_padding = egui::vec2(16.0, 10.0);
                                        egui::ComboBox::from_id_salt("sp_diff")
                                            .width(ui.available_width() - 8.0)
                                            .selected_text(RichText::new(format!("{:?}", config.bot_difficulty)).size(16.0))
                                            .show_ui(ui, |ui| {
                                                ui.selectable_value(&mut config.bot_difficulty, sow_core::game_config::BotDifficulty::Vanilla, "Vanilla");
                                                ui.selectable_value(&mut config.bot_difficulty, sow_core::game_config::BotDifficulty::Terminator, "Terminator");
                                            });
                                    });
                                };

                                let draw_bots = |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                                    setting_card(ui, &strings.tribes_count, is_mobile, |ui| {
                                        ui.style_mut().spacing.slider_width = ui.available_width() - 80.0;
                                        ui.add(egui::Slider::new(&mut config.bot_count, 0..=1000));
                                    });
                                };

                                let draw_nations = |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                                    setting_card(ui, &strings.nations_count, is_mobile, |ui| {
                                        ui.style_mut().spacing.slider_width = ui.available_width() - 80.0;
                                        ui.add(egui::Slider::new(&mut config.nation_count, 0..=400));
                                    });
                                };
                                
                                let draw_spawn = |ui: &mut egui::Ui, config: &mut sow_core::game_config::GameConfig| {
                                    setting_card(ui, &strings.random_spawning, is_mobile, |ui| {
                                        let btn_text = if config.random_spawn { "ON ✔" } else { "OFF ❌" };
                                        let btn = egui::Button::new(RichText::new(btn_text).strong().size(14.0))
                                            .fill(if config.random_spawn { theme::accent_solo_cyan() } else { theme::menu_secondary_button() })
                                            .corner_radius(12.0)
                                            .min_size(egui::vec2(100.0, 36.0));
                                        
                                        if ui.add(btn).clicked() {
                                            config.random_spawn = !config.random_spawn;
                                        }
                                    });
                                };
                                
                                if is_mobile {
                                    // Single Column layout
                                    draw_map_picker(ui, config);
                                    draw_diff(ui, config);
                                    draw_bots(ui, config);
                                    draw_nations(ui, config);
                                    draw_spawn(ui, config);
                                } else {
                                    // Two columns on Desktop
                                    ui.columns(2, |cols| {
                                        cols[0].spacing_mut().item_spacing = egui::vec2(0.0, 16.0);
                                        cols[1].spacing_mut().item_spacing = egui::vec2(0.0, 16.0);

                                        // Left Column: Map Visual briefing, selector, diff, and spawn
                                        // Draw Map visual preview
                                        let thumbnail = asset_loader.thumbnail(&config.map_name);
                                        let aspect = 1.77_f32;
                                        let w = cols[0].available_width();
                                        let h = w / aspect;
                                        let rect = cols[0].allocate_exact_size(egui::vec2(w, h), egui::Sense::hover()).0;

                                        if let Some(tex) = thumbnail {
                                            let image = egui::Image::new(tex)
                                                .fit_to_exact_size(rect.size())
                                                .corner_radius(12);
                                            cols[0].put(rect, image);
                                        } else {
                                            cols[0].painter().rect_filled(rect, 12.0, Color32::from_black_alpha(120));
                                            crate::ui::theme::outlined_text(
                                                cols[0].painter(),
                                                rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                &strings.no_preview,
                                                egui::FontId::proportional(16.0),
                                                theme::text_secondary(),
                                                Color32::BLACK,
                                            );
                                        }

                                        cols[0].painter().rect_stroke(
                                            rect,
                                            12.0_f32,
                                            Stroke::new(1.5_f32, theme::menu_panel_border_glow()),
                                            egui::StrokeKind::Inside,
                                        );

                                        cols[0].add_space(4.0);
                                        draw_map_picker(&mut cols[0], config);
                                        draw_diff(&mut cols[0], config);
                                        draw_spawn(&mut cols[0], config);

                                        // Right Column: Sliders
                                        draw_bots(&mut cols[1], config);
                                        draw_nations(&mut cols[1], config);
                                    });
                                }

                                ui.add_space(20.0);

                                // Campaign action buttons at bottom
                                ui.vertical_centered(|ui| {
                                    let (btn_w, btn_h) = if is_mobile {
                                        (panel_w - 32.0, 44.0)
                                    } else {
                                        (220.0, 50.0)
                                    };

                                    if !is_mobile {
                                        ui.horizontal(|ui| {
                                            let spacing = 24.0;
                                            let total_w = btn_w * 2.0 + spacing;
                                            let remaining = (ui.available_width() - total_w) / 2.0;
                                            ui.add_space(remaining);
                                            
                                            let cancel_btn = crate::widgets::NeonButton::new(&strings.back)
                                                .style(crate::widgets::NeonButtonStyle::Outline)
                                                .min_size(egui::vec2(btn_w, btn_h))
                                                .text_size(18.0);
                                            
                                            if ui.add(cancel_btn).clicked() {
                                                close = true;
                                            }

                                            ui.add_space(spacing);

                                            let start_btn = crate::widgets::NeonButton::new(&strings.start_simulation)
                                                .style(crate::widgets::NeonButtonStyle::Primary)
                                                .min_size(egui::vec2(btn_w, btn_h))
                                                .text_size(18.0);
                                            
                                            if ui.add(start_btn).clicked() {
                                                *action = Some(UiAction::StartSinglePlayer(Box::new(*state.single_player_config.clone())));
                                                close = true;
                                            }
                                        });
                                    } else {
                                        let start_btn = crate::widgets::NeonButton::new(&strings.start_simulation)
                                            .style(crate::widgets::NeonButtonStyle::Primary)
                                            .min_size(egui::vec2(btn_w, btn_h))
                                            .text_size(18.0);
                                        
                                        if ui.add(start_btn).clicked() {
                                            *action = Some(UiAction::StartSinglePlayer(Box::new(*state.single_player_config.clone())));
                                            close = true;
                                        }

                                        ui.add_space(12.0);

                                        let cancel_btn = crate::widgets::NeonButton::new(&strings.back)
                                            .style(crate::widgets::NeonButtonStyle::Outline)
                                            .min_size(egui::vec2(btn_w, btn_h))
                                            .text_size(18.0);
                                        
                                        if ui.add(cancel_btn).clicked() {
                                            close = true;
                                        }
                                    }
                                });
                                
                                ui.add_space(16.0);
                            });
                     });
            });
        });

    if close {
        state.show_single_player_setup = false;
    }
}
