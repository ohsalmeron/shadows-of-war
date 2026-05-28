use egui::{Color32, Stroke};

pub(crate) fn calculate_cover_uv(rect_size: egui::Vec2, tex_size: egui::Vec2) -> egui::Rect {
    let rect_aspect = rect_size.x / rect_size.y;
    let tex_aspect = tex_size.x / tex_size.y;

    if tex_aspect > rect_aspect {
        // Texture is wider than destination rect. Crop horizontally.
        let u_width = rect_aspect / tex_aspect;
        let u_start = (1.0 - u_width) / 2.0;
        egui::Rect::from_min_max(egui::pos2(u_start, 0.0), egui::pos2(u_start + u_width, 1.0))
    } else {
        // Texture is taller than destination rect. Crop vertically.
        let v_height = tex_aspect / rect_aspect;
        let v_start = (1.0 - v_height) / 2.0;
        egui::Rect::from_min_max(
            egui::pos2(0.0, v_start),
            egui::pos2(1.0, v_start + v_height),
        )
    }
}

fn draw_skin_swatches(ui: &mut egui::Ui, swatch_size: f32) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("SKINS:")
                .strong()
                .color(crate::ui::theme::text_secondary())
                .size(if swatch_size >= 15.0 { 13.0 } else { 12.0 }),
        );
        let swatches = [
            Color32::from_rgb(100, 116, 139),
            Color32::from_rgb(234, 179, 8),
            Color32::from_rgb(30, 41, 59),
            Color32::from_rgb(6, 182, 212),
        ];
        let radius = swatch_size * 0.5;
        for color in swatches {
            let (s_rect, s_resp) =
                ui.allocate_exact_size(egui::vec2(swatch_size, swatch_size), egui::Sense::click());
            ui.painter()
                .circle_filled(s_rect.center(), radius, color);
            if s_resp.hovered() {
                ui.painter().circle_stroke(
                    s_rect.center(),
                    radius + 2.0,
                    Stroke::new(1.0_f32, Color32::WHITE),
                );
            }
        }
    });
}

fn draw_leader_info_card(
    ui: &mut egui::Ui,
    selected_leader: sow_core::player::Leader,
    selected_civilization: sow_core::player::Civilization,
    reign_dates: &str,
    is_mobile: bool,
    card_w: f32,
) {
    let card_frame = egui::Frame::NONE
        .fill(Color32::from_black_alpha(150))
        .stroke(Stroke::new(
            1.2_f32,
            crate::ui::theme::accent_solo_cyan(),
        ))
        .corner_radius(if is_mobile { 10 } else { 12 })
        .inner_margin(egui::Margin::symmetric(
            if is_mobile { 16 } else { 14 },
            if is_mobile { 10 } else { 10 },
        ));

    ui.allocate_ui(egui::vec2(card_w, 0.0), |ui| {
        card_frame.show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(selected_leader.name().to_uppercase())
                        .strong()
                        .color(Color32::WHITE)
                        .size(if is_mobile { 28.0 } else { 32.0 }),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{} • {}",
                        selected_civilization.name(),
                        reign_dates
                    ))
                    .color(crate::ui::theme::text_secondary())
                    .size(if is_mobile { 15.0 } else { 16.0 })
                    .strong(),
                );
                ui.add_space(4.0);

                ui.label(
                    egui::RichText::new("UNIQUE ABILITY:")
                        .strong()
                        .color(crate::ui::theme::text_secondary())
                        .size(if is_mobile { 12.0 } else { 13.0 }),
                );
                ui.label(
                    egui::RichText::new(selected_leader.perk_description())
                        .color(crate::ui::theme::accent_solo_cyan())
                        .size(if is_mobile { 17.0 } else { 19.0 })
                        .strong(),
                );
                ui.add_space(4.0);

                draw_skin_swatches(ui, if is_mobile { 14.0 } else { 16.0 });
            });
        });
    });
}

fn draw_leader_carousel(
    ui: &mut egui::Ui,
    selected_leader: &mut sow_core::player::Leader,
    selected_civilization: &mut sow_core::player::Civilization,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    avatar_size: f32,
    scroll_area_h: f32,
    panel_w: f32,
    is_mobile: bool,
) {
    let leader_count = sow_core::player::Leader::ALL.len() as f32;
    let total_carousel_w = (avatar_size + 12.0) * leader_count - 12.0;

    ui.vertical_centered(|ui| {
        ui.allocate_ui(egui::vec2(panel_w, scroll_area_h + 24.0), |ui| {
            let carousel_frame = egui::Frame::NONE
                .fill(Color32::from_black_alpha(150))
                .stroke(Stroke::new(
                    1.2_f32,
                    crate::ui::theme::accent_solo_cyan().linear_multiply(0.4),
                ))
                .corner_radius(if is_mobile { 10 } else { 12 })
                .inner_margin(egui::Margin::symmetric(
                    if is_mobile { 8 } else { 16 },
                    12,
                ));

            carousel_frame.show(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;

                            let inner_w = panel_w - (if is_mobile { 16.0 } else { 32.0 });
                            if inner_w > total_carousel_w {
                                let space = (inner_w - total_carousel_w) / 2.0;
                                ui.add_space(space);
                            } else {
                                ui.add_space(12.0);
                            }

                            for &leader in sow_core::player::Leader::ALL.iter() {
                                let is_selected = *selected_leader == leader;
                                let civ = match leader {
                                    sow_core::player::Leader::Caesar => {
                                        sow_core::player::Civilization::Rome
                                    }
                                    sow_core::player::Leader::Cleopatra => {
                                        sow_core::player::Civilization::Egypt
                                    }
                                    sow_core::player::Leader::Ragnar => {
                                        sow_core::player::Civilization::Vikings
                                    }
                                    sow_core::player::Leader::SunTzu => {
                                        sow_core::player::Civilization::China
                                    }
                                    sow_core::player::Leader::Alexander => {
                                        sow_core::player::Civilization::Macedon
                                    }
                                    sow_core::player::Leader::GenghisKhan => {
                                        sow_core::player::Civilization::Mongols
                                    }
                                    sow_core::player::Leader::RichardTheLionheart => {
                                        sow_core::player::Civilization::Angevin
                                    }
                                    sow_core::player::Leader::Vercingetorix => {
                                        sow_core::player::Civilization::Gallic
                                    }
                                    sow_core::player::Leader::Boudica => {
                                        sow_core::player::Civilization::Iceni
                                    }
                                    sow_core::player::Leader::LadySixSky => {
                                        sow_core::player::Civilization::Maya
                                    }
                                    sow_core::player::Leader::Leonidas => {
                                        sow_core::player::Civilization::Sparta
                                    }
                                };

                                let bg = if is_selected {
                                    crate::ui::theme::accent_solo_cyan().linear_multiply(0.2)
                                } else {
                                    Color32::from_black_alpha(140)
                                };
                                let border = if is_selected {
                                    crate::ui::theme::accent_solo_cyan()
                                } else {
                                    crate::ui::theme::nickname_field_border()
                                };

                                let (s_rect, s_resp) = ui.allocate_exact_size(
                                    egui::vec2(avatar_size, avatar_size),
                                    egui::Sense::click(),
                                );
                                if s_resp.hovered() {
                                    ui.ctx()
                                        .set_cursor_icon(egui::CursorIcon::PointingHand);
                                }

                                ui.painter().rect(
                                    s_rect,
                                    8,
                                    bg,
                                    Stroke::NONE,
                                    egui::StrokeKind::Inside,
                                );

                                if let Some(tex) = asset_loader.avatars.get(&leader) {
                                    let image = egui::Image::new(tex)
                                        .fit_to_exact_size(s_rect.size())
                                        .corner_radius(egui::CornerRadius::same(8));
                                    ui.put(s_rect, image);
                                }

                                ui.painter().rect(
                                    s_rect,
                                    8,
                                    Color32::TRANSPARENT,
                                    Stroke::new(
                                        if is_selected { 2.0_f32 } else { 0.5_f32 },
                                        border,
                                    ),
                                    egui::StrokeKind::Inside,
                                );

                                if s_resp.clicked() {
                                    *selected_leader = leader;
                                    *selected_civilization = civ;
                                }
                            }
                            if inner_w <= total_carousel_w {
                                ui.add_space(12.0);
                            }
                        });
                    });
            });
        });
    });
}

pub fn draw_leader_picker_modal(
    ctx: &egui::Context,
    selected_leader: &mut sow_core::player::Leader,
    selected_civilization: &mut sow_core::player::Civilization,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
) -> bool {
    let mut close = false;

    egui::Area::new(egui::Id::new("leader_picker_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let screen_rect = ctx.content_rect();
            let is_mobile = screen_rect.width() < 900.0 || screen_rect.height() < 600.0;

            for &leader in &sow_core::player::Leader::ALL {
                asset_loader.request_leader_portrait(leader, is_mobile);
            }
            asset_loader.request_leader_portrait(*selected_leader, is_mobile);

            let content_rect = if is_mobile {
                let mut rect = screen_rect;
                rect.min.x += 24.0; // Generous left margin
                rect.max.x -= 24.0; // Generous right margin
                rect.min.y += 56.0; // Top margin to clear safe areas and notch
                rect.max.y -= 36.0; // Bottom margin to clear home indicator and safe areas
                rect
            } else {
                screen_rect.shrink(40.0)
            };

            let is_inside_active_ui = if let Some(click_pos) = ctx.input(|i| {
                i.pointer
                    .press_origin()
                    .or_else(|| i.pointer.interact_pos())
            }) {
                if is_mobile {
                    content_rect.contains(click_pos)
                } else {
                    let header_rect = egui::Rect::from_min_max(
                        egui::pos2(content_rect.min.x, content_rect.min.y),
                        egui::pos2(content_rect.max.x, content_rect.min.y + 80.0),
                    );
                    let card_rect = egui::Rect::from_min_max(
                        egui::pos2(content_rect.min.x - 20.0, content_rect.min.y + 60.0),
                        egui::pos2(content_rect.min.x + 380.0, content_rect.max.y - 120.0),
                    );
                    let bottom_rect = egui::Rect::from_min_max(
                        egui::pos2(content_rect.min.x, content_rect.max.y - 180.0),
                        egui::pos2(content_rect.max.x, content_rect.max.y + 20.0),
                    );
                    header_rect.contains(click_pos)
                        || card_rect.contains(click_pos)
                        || bottom_rect.contains(click_pos)
                }
            } else {
                false
            };

            // Click backdrop to close
            let backdrop_response = ui.allocate_rect(screen_rect, egui::Sense::click());
            ui.painter()
                .rect_filled(screen_rect, 0.0, Color32::from_black_alpha(210));
            if backdrop_response.clicked() && !is_inside_active_ui {
                close = true;
            }

            let reign_dates = match *selected_leader {
                sow_core::player::Leader::Caesar => "Reigned 49 – 44 BC",
                sow_core::player::Leader::Cleopatra => "Reigned 51 – 30 BC",
                sow_core::player::Leader::Ragnar => "Reigned 800 – 845 AD",
                sow_core::player::Leader::SunTzu => "Reigned 544 – 496 BC",
                sow_core::player::Leader::Alexander => "Reigned 336 – 323 BC",
                sow_core::player::Leader::GenghisKhan => "Reigned 1206 – 1227 AD",
                sow_core::player::Leader::RichardTheLionheart => "Reigned 1189 – 1199 AD",
                sow_core::player::Leader::Vercingetorix => "Reigned 82 – 46 BC",
                sow_core::player::Leader::Boudica => "Reigned 60 – 61 AD",
                sow_core::player::Leader::LadySixSky => "Reigned 612 – 693 AD",
                sow_core::player::Leader::Leonidas => "Reigned 489 – 480 BC",
            };

            // --- FULLSCREEN IMMERSIVE BACKDROP PORTRAIT (COVER ASPECT RATIO, NO STRETCH) ---
            if is_mobile {
                if let Some(tex) = asset_loader.leader_mobile_images.get(selected_leader) {
                    let uv = calculate_cover_uv(screen_rect.size(), tex.size_vec2());
                    ui.painter()
                        .image(tex.id(), screen_rect, uv, Color32::WHITE);
                }
            } else {
                if let Some(tex) = asset_loader.leader_desktop_images.get(selected_leader) {
                    let uv = calculate_cover_uv(screen_rect.size(), tex.size_vec2());
                    ui.painter()
                        .image(tex.id(), screen_rect, uv, Color32::WHITE);
                }
            }

            const CONFIRM_BTN_H: f32 = 44.0;
            const CONFIRM_BTN_W: f32 = 200.0;
            const CONFIRM_GAP: f32 = 12.0;
            const MOBILE_CARD_GAP: f32 = 8.0;
            const MOBILE_CARD_EST_H: f32 = 168.0;
            let avatar_size = if is_mobile { 64.0 } else { 72.0 };
            let scroll_area_h = avatar_size + 4.0;
            let carousel_block_h = scroll_area_h + 24.0;
            let card_w = if is_mobile {
                content_rect.width()
            } else {
                320.0
            };
            let mobile_bottom_stack_h =
                MOBILE_CARD_EST_H + MOBILE_CARD_GAP + carousel_block_h;
            let leader_count = sow_core::player::Leader::ALL.len() as f32;
            let total_carousel_w = (avatar_size + 12.0) * leader_count - 12.0;
            let horizontal_margin = if is_mobile { 16.0 } else { 32.0 };
            let min_panel_w = total_carousel_w + horizontal_margin;
            let desktop_panel_w = min_panel_w.max(540.0).min(content_rect.width());
            let mobile_panel_w = content_rect.width();
            let main_rect = egui::Rect::from_min_max(
                content_rect.min,
                egui::pos2(
                    content_rect.max.x,
                    content_rect.max.y - CONFIRM_BTN_H - CONFIRM_GAP,
                ),
            );

            ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                ui.scope_builder(egui::UiBuilder::new().max_rect(main_rect), |ui| {
                    ui.vertical_centered(|ui| {
                        crate::ui::theme::outlined_label(
                            ui,
                            "CHOOSE YOUR LEADER",
                            egui::FontId::proportional(if is_mobile { 24.0 } else { 32.0 }),
                            Color32::WHITE,
                        );
                    });
                    ui.add_space(if is_mobile { 12.0 } else { 24.0 });

                    if is_mobile {
                        let stack_rect = egui::Rect::from_min_max(
                            egui::pos2(
                                content_rect.min.x,
                                main_rect.max.y - mobile_bottom_stack_h,
                            ),
                            main_rect.max,
                        );
                        ui.scope_builder(egui::UiBuilder::new().max_rect(stack_rect), |ui| {
                            ui.with_layout(
                                egui::Layout::bottom_up(egui::Align::Center),
                                |ui| {
                                    ui.set_width(mobile_panel_w);
                                    draw_leader_carousel(
                                        ui,
                                        selected_leader,
                                        selected_civilization,
                                        asset_loader,
                                        avatar_size,
                                        scroll_area_h,
                                        mobile_panel_w,
                                        true,
                                    );
                                    ui.add_space(MOBILE_CARD_GAP);
                                    draw_leader_info_card(
                                        ui,
                                        *selected_leader,
                                        *selected_civilization,
                                        reign_dates,
                                        true,
                                        card_w,
                                    );
                                },
                            );
                        });
                    } else {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                draw_leader_info_card(
                                    ui,
                                    *selected_leader,
                                    *selected_civilization,
                                    reign_dates,
                                    false,
                                    card_w,
                                );
                            });

                            let space =
                                (ui.available_height() - carousel_block_h).max(10.0);
                            ui.add_space(space);

                            draw_leader_carousel(
                                ui,
                                selected_leader,
                                selected_civilization,
                                asset_loader,
                                avatar_size,
                                scroll_area_h,
                                desktop_panel_w,
                                false,
                            );
                        });
                    }
                });

                // Confirm button pinned to the bottom — never pushed by content above
                let confirm_rect = egui::Rect::from_min_max(
                    egui::pos2(
                        content_rect.center().x - CONFIRM_BTN_W * 0.5,
                        content_rect.max.y - CONFIRM_BTN_H,
                    ),
                    egui::pos2(
                        content_rect.center().x + CONFIRM_BTN_W * 0.5,
                        content_rect.max.y,
                    ),
                );
                let confirm_response = ui.put(
                    confirm_rect,
                    crate::widgets::ThemeButton::new("CONFIRM")
                        .style(crate::widgets::ThemeButtonStyle::Primary)
                        .min_size(egui::vec2(CONFIRM_BTN_W, CONFIRM_BTN_H))
                        .text_size(16.0),
                );
                if confirm_response.clicked() {
                    close = true;
                }
            });
        });

    close
}
