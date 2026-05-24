use egui::{Color32, Stroke};

fn calculate_cover_uv(rect_size: egui::Vec2, tex_size: egui::Vec2) -> egui::Rect {
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

fn draw_vertical_gradient(
    painter: &egui::Painter,
    rect: egui::Rect,
    top_color: egui::Color32,
    bottom_color: egui::Color32,
) {
    let mut mesh = egui::Mesh::default();
    mesh.vertices.reserve(4);
    mesh.indices.reserve(6);

    let idx = mesh.vertices.len() as u32;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_top(),
        uv: egui::epaint::WHITE_UV,
        color: top_color,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_top(),
        uv: egui::epaint::WHITE_UV,
        color: top_color,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_bottom(),
        uv: egui::epaint::WHITE_UV,
        color: bottom_color,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_bottom(),
        uv: egui::epaint::WHITE_UV,
        color: bottom_color,
    });

    mesh.indices.push(idx);
    mesh.indices.push(idx + 1);
    mesh.indices.push(idx + 2);
    mesh.indices.push(idx);
    mesh.indices.push(idx + 2);
    mesh.indices.push(idx + 3);

    painter.add(egui::Shape::mesh(mesh));
}

pub fn draw_leader_picker_modal(
    ctx: &egui::Context,
    selected_leader: &mut sow_core::player::Leader,
    selected_civilization: &mut sow_core::player::Civilization,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
) -> bool {
    let mut close = false;

    egui::Area::new(egui::Id::new("leader_picker_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let screen_rect = ctx.content_rect();
            let is_mobile = screen_rect.width() < 720.0;

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
                        egui::pos2(content_rect.max.x - 380.0, content_rect.min.y + 60.0),
                        egui::pos2(content_rect.max.x + 20.0, content_rect.max.y - 120.0),
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

            // Transparent overlay layer (15% dark)
            ui.painter()
                .rect_filled(screen_rect, 0.0, Color32::from_black_alpha(38));

            // Bottom-fading vertical dark gradient (50% bottom, fading to 0% at the top)
            draw_vertical_gradient(
                ui.painter(),
                screen_rect,
                Color32::from_black_alpha(0),
                Color32::from_black_alpha(128),
            );

            ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                ui.vertical(|ui| {
                    // 1. Centered Premium Header
                    ui.vertical_centered(|ui| {
                        crate::ui::theme::outlined_label(
                            ui,
                            "CHOOSE YOUR LEADER",
                            egui::FontId::proportional(if is_mobile { 24.0 } else { 32.0 }),
                            Color32::WHITE,
                        );
                    });

                    ui.add_space(if is_mobile { 12.0 } else { 24.0 });

                    let card_w = if is_mobile {
                        ui.available_width()
                    } else {
                        340.0
                    };

                    if !is_mobile {
                        // --- DESKTOP SPECIFIC LAYOUT: Floating panel on the right side ---
                        ui.horizontal(|ui| {
                            let remaining_space = ui.available_width() - card_w;
                            ui.add_space(remaining_space);

                            let card_frame = egui::Frame::NONE
                                .fill(Color32::from_black_alpha(150))
                                .stroke(Stroke::new(1.2_f32, crate::ui::theme::accent_solo_cyan()))
                                .corner_radius(12)
                                .inner_margin(egui::Margin::symmetric(24, 20));

                            ui.allocate_ui(egui::vec2(card_w, 0.0), |ui| {
                                card_frame.show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        ui.label(
                                            egui::RichText::new(
                                                selected_leader.name().to_uppercase(),
                                            )
                                            .strong()
                                            .color(Color32::WHITE)
                                            .size(24.0),
                                        );
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{} • {}",
                                                selected_civilization.name(),
                                                reign_dates
                                            ))
                                            .color(crate::ui::theme::text_secondary())
                                            .size(12.0)
                                            .strong(),
                                        );
                                        ui.add_space(10.0);

                                        ui.label(
                                            egui::RichText::new("UNIQUE ABILITY:")
                                                .strong()
                                                .color(crate::ui::theme::text_secondary())
                                                .size(10.5),
                                        );
                                        ui.label(
                                            egui::RichText::new(selected_leader.perk_description())
                                                .color(crate::ui::theme::accent_solo_cyan())
                                                .size(13.5)
                                                .strong(),
                                        );
                                        ui.add_space(10.0);

                                        // Skins
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new("SKINS:")
                                                    .strong()
                                                    .color(crate::ui::theme::text_secondary())
                                                    .size(10.5),
                                            );
                                            let swatches = [
                                                Color32::from_rgb(100, 116, 139),
                                                Color32::from_rgb(234, 179, 8),
                                                Color32::from_rgb(30, 41, 59),
                                                Color32::from_rgb(6, 182, 212),
                                            ];
                                            for color in swatches {
                                                let (s_rect, s_resp) = ui.allocate_exact_size(
                                                    egui::vec2(16.0, 16.0),
                                                    egui::Sense::click(),
                                                );
                                                ui.painter().circle_filled(
                                                    s_rect.center(),
                                                    8.0,
                                                    color,
                                                );
                                                if s_resp.hovered() {
                                                    ui.painter().circle_stroke(
                                                        s_rect.center(),
                                                        10.0,
                                                        Stroke::new(1.0_f32, Color32::WHITE),
                                                    );
                                                }
                                            }
                                        });
                                    });
                                });
                            });
                        });

                        // Push carousel to the bottom
                        let space = (ui.available_height() - 150.0).max(10.0);
                        ui.add_space(space);
                    } else {
                        // --- MOBILE SPECIFIC LAYOUT: Push all elements down so they stack at the bottom ---
                        let space = (ui.available_height() - 340.0).max(10.0);
                        ui.add_space(space);
                    }

                    // --- BOTTOM STACKED ELEMENTS ---
                    if is_mobile {
                        // Floating Description & Skins Panel stacked above avatars
                        let card_frame = egui::Frame::NONE
                            .fill(Color32::from_black_alpha(150))
                            .stroke(Stroke::new(1.2_f32, crate::ui::theme::accent_solo_cyan()))
                            .corner_radius(12)
                            .inner_margin(egui::Margin::symmetric(20, 14));

                        card_frame.show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(selected_leader.name().to_uppercase())
                                        .strong()
                                        .color(Color32::WHITE)
                                        .size(20.0),
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} • {}",
                                        selected_civilization.name(),
                                        reign_dates
                                    ))
                                    .color(crate::ui::theme::text_secondary())
                                    .size(11.0)
                                    .strong(),
                                );
                                ui.add_space(6.0);

                                ui.label(
                                    egui::RichText::new("UNIQUE ABILITY:")
                                        .strong()
                                        .color(crate::ui::theme::text_secondary())
                                        .size(10.0),
                                );
                                ui.label(
                                    egui::RichText::new(selected_leader.perk_description())
                                        .color(crate::ui::theme::accent_solo_cyan())
                                        .size(12.0)
                                        .strong(),
                                );
                                ui.add_space(6.0);

                                // Skins
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("SKINS:")
                                            .strong()
                                            .color(crate::ui::theme::text_secondary())
                                            .size(10.0),
                                    );
                                    let swatches = [
                                        Color32::from_rgb(100, 116, 139),
                                        Color32::from_rgb(234, 179, 8),
                                        Color32::from_rgb(30, 41, 59),
                                        Color32::from_rgb(6, 182, 212),
                                    ];
                                    for color in swatches {
                                        let (s_rect, s_resp) = ui.allocate_exact_size(
                                            egui::vec2(14.0, 14.0),
                                            egui::Sense::click(),
                                        );
                                        ui.painter().circle_filled(s_rect.center(), 7.0, color);
                                        if s_resp.hovered() {
                                            ui.painter().circle_stroke(
                                                s_rect.center(),
                                                9.0,
                                                Stroke::new(1.0_f32, Color32::WHITE),
                                            );
                                        }
                                    }
                                });
                            });
                        });
                        ui.add_space(10.0);
                    }

                    // Horizontal Scrollable Carousel for Avatar Selection (Centered at bottom)
                    let avatar_size = if is_mobile { 64.0 } else { 72.0 };
                    let scroll_area_h = avatar_size; // No extra height needed since scrollbar is hidden!

                    let panel_w = if is_mobile {
                        ui.available_width()
                    } else {
                        540.0
                    };

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
                                        egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                    )
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 12.0;

                                            // Spacer to center the avatars inside the panel if they fit
                                            let inner_w =
                                                panel_w - (if is_mobile { 16.0 } else { 32.0 });
                                            let total_carousel_w =
                                                (avatar_size + 12.0) * 6.0 - 12.0;
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
                                                };

                                                let bg = if is_selected {
                                                    crate::ui::theme::accent_solo_cyan()
                                                        .linear_multiply(0.2)
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
                                                    ui.ctx().set_cursor_icon(
                                                        egui::CursorIcon::PointingHand,
                                                    );
                                                }

                                                ui.painter().rect(
                                                    s_rect,
                                                    8,
                                                    bg,
                                                    Stroke::NONE,
                                                    egui::StrokeKind::Inside,
                                                );

                                                if let Some(tex) = asset_loader.avatars.get(&leader)
                                                {
                                                    let image = egui::Image::new(tex)
                                                        .fit_to_exact_size(s_rect.size())
                                                        .corner_radius(egui::CornerRadius::same(8));
                                                    ui.put(s_rect, image);
                                                }

                                                // Draw premium light border frame on top of the image so it is clearly visible
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

                    ui.add_space(12.0);

                    // Centered Floating Confirm Button
                    ui.vertical_centered(|ui| {
                        let btn = crate::widgets::ThemeButton::new("CONFIRM")
                            .style(crate::widgets::ThemeButtonStyle::Primary)
                            .min_size(egui::vec2(200.0, 44.0))
                            .text_size(16.0);
                        if ui.add(btn).clicked() {
                            close = true;
                        }
                    });
                });
            });
        });

    close
}
