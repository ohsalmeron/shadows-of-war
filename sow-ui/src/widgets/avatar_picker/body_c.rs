
fn draw_leader_hero_text(
    ui: &mut egui::Ui,
    selected_leader: sow_data::Leader,
    selected_civilization: sow_data::Civilization,
    reign_dates: &str,
    text_w: f32,
    metrics: &LeaderPickerMetrics,
    center: bool,
) {
    let layout = if center {
        egui::Layout::top_down(egui::Align::Center)
    } else {
        egui::Layout::top_down(egui::Align::LEFT)
    };
    ui.with_layout(layout, |ui| {
        ui.set_width(text_w);
        ui.spacing_mut().item_spacing.y = metrics.hero_text_spacing;
        crate::ui::theme::leader_name_label(ui, selected_leader.name(), metrics.name_font);
        crate::ui::theme::leader_caps_line(
            ui,
            &format!("{} • {}", selected_civilization.name(), reign_dates),
            metrics.caps_line_font,
        );
        crate::ui::theme::leader_caps_paragraph(
            ui,
            selected_leader.perk_description(),
            metrics.caps_para_font,
            text_w,
        );
    });
}

fn paint_gradient_rect(
    painter: &egui::Painter,
    rect: egui::Rect,
    left_top: Color32,
    right_top: Color32,
    right_bottom: Color32,
    left_bottom: Color32,
) {
    if !rect.is_positive() {
        return;
    }
    let mut mesh = egui::Mesh::default();
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_top(),
        uv: egui::Pos2::ZERO,
        color: left_top,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_top(),
        uv: egui::Pos2::ZERO,
        color: right_top,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_bottom(),
        uv: egui::Pos2::ZERO,
        color: right_bottom,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_bottom(),
        uv: egui::Pos2::ZERO,
        color: left_bottom,
    });
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}



pub(crate) fn draw_leader_picker_overlay_gradient(
    painter: &egui::Painter,
    screen_rect: Rect,
    is_mobile: bool,
) {
    const PANEL_FRAC: f32 = 0.45;
    let dark = Color32::from_rgba_unmultiplied(0, 0, 0, 190);

    if is_mobile {
        let panel_h = screen_rect.height() * PANEL_FRAC;
        let gradient_rect = Rect::from_min_max(
            egui::pos2(screen_rect.min.x, screen_rect.max.y - panel_h),
            screen_rect.max,
        );
        paint_gradient_rect(
            painter,
            gradient_rect,
            Color32::TRANSPARENT,
            Color32::TRANSPARENT,
            dark,
            dark,
        );
    } else {
        let panel_w = screen_rect.width() * PANEL_FRAC;
        let gradient_rect = Rect::from_min_max(
            screen_rect.min,
            egui::pos2(screen_rect.min.x + panel_w, screen_rect.max.y),
        );
        paint_gradient_rect(
            painter,
            gradient_rect,
            dark,
            Color32::TRANSPARENT,
            Color32::TRANSPARENT,
            dark,
        );
    }
}

pub(crate) fn leader_civilization(leader: sow_data::Leader) -> sow_data::Civilization {
    match leader {
        sow_data::Leader::Caesar => sow_data::Civilization::Rome,
        sow_data::Leader::Cleopatra => sow_data::Civilization::Egypt,
        sow_data::Leader::Ragnar => sow_data::Civilization::Vikings,
        sow_data::Leader::SunTzu => sow_data::Civilization::China,
        sow_data::Leader::Alexander => sow_data::Civilization::Macedon,
        sow_data::Leader::GenghisKhan => sow_data::Civilization::Mongols,
        sow_data::Leader::RichardTheLionheart => sow_data::Civilization::Angevin,
        sow_data::Leader::Vercingetorix => sow_data::Civilization::Gallic,
        sow_data::Leader::Boudica => sow_data::Civilization::Iceni,
        sow_data::Leader::LadySixSky => sow_data::Civilization::Maya,
        sow_data::Leader::Leonidas => sow_data::Civilization::Sparta,
        sow_data::Leader::Napoleon => sow_data::Civilization::France,
    }
}

fn leader_filler_color32(leader: sow_data::Leader) -> Color32 {
    let [r, g, b] = leader.filler_rgb();
    Color32::from_rgb(
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    )
}

fn draw_leader_avatar_button(
    ui: &mut egui::Ui,
    leader: sow_data::Leader,
    selected_leader: sow_data::Leader,
    avatar_size: f32,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
) -> bool {
    let is_selected = selected_leader == leader;
    let (s_rect, s_resp) =
        ui.allocate_exact_size(egui::vec2(avatar_size, avatar_size), egui::Sense::click());
    if s_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let leader_color = leader_filler_color32(leader);

    let anim_id = egui::Id::new(("leader_picker_select", leader));
    let scale = crate::ui::animation::selection_grow_scale(
        ui.ctx(),
        anim_id,
        is_selected,
        LEADER_SELECT_GROW,
        LEADER_SELECT_ANIM_SECS,
    );
    let paint_size = avatar_size * scale;
    let center = s_rect.center();
    let radius = paint_size * 0.5;
    let inner_r = radius - 2.0;

    ui.painter().circle_filled(center, inner_r, leader_color);

    if let Some(tex) = asset_loader.avatars.get(&leader) {
        let image = egui::Image::new(tex)
            .fit_to_exact_size(egui::vec2(inner_r * 2.0, inner_r * 2.0))
            .corner_radius(egui::CornerRadius::same((inner_r as u8).max(1)));
        let image_rect =
            egui::Rect::from_center_size(center, egui::vec2(inner_r * 2.0, inner_r * 2.0));
        ui.put(image_rect, image);
    }

    if is_selected {
        let glow = leader_color;
        ui.painter()
            .circle_stroke(center, radius + 1.0, Stroke::new(2.5_f32, glow));
        ui.painter().circle_stroke(
            center,
            radius + 4.0,
            Stroke::new(1.0_f32, glow.linear_multiply(0.45)),
        );
    } else if s_resp.hovered() {
        ui.painter().circle_stroke(
            center,
            radius + 1.0,
            Stroke::new(1.5_f32, leader_color.linear_multiply(0.85)),
        );
    }

    s_resp.clicked()
}

fn draw_leader_rail(
    ui: &mut egui::Ui,
    selected_leader: &mut sow_data::Leader,
    selected_civilization: &mut sow_data::Civilization,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    metrics: &LeaderPickerMetrics,
    rail_h: f32,
) {
    let avatar_size = metrics.avatar_size;
    ui.allocate_ui(egui::vec2(ui.max_rect().width(), rail_h), |ui| {
        let area = ui.max_rect();
        // Avatars are pinned on the right; scrollbar sits to their left.
        let avatar_rect = Rect::from_min_size(
            egui::pos2(area.max.x - avatar_size, area.min.y),
            egui::vec2(avatar_size, rail_h),
        );
        let bar_lane_rect = Rect::from_min_max(
            egui::pos2(
                avatar_rect.min.x - metrics.rail_scrollbar_gap - metrics.rail_bar_lane,
                area.min.y,
            ),
            egui::pos2(avatar_rect.min.x - metrics.rail_scrollbar_gap, area.max.y),
        );
        let scroll_bar_lane =
            bar_lane_rect.shrink2(egui::vec2(0.0, metrics.rail_scroll_track_top_pad));

        let scroll_output = ui
            .scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .scroll_source(egui::scroll_area::ScrollSource {
                        scroll_bar: false,
                        drag: true,
                        mouse_wheel: true,
                    })
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .id_salt("leader_desktop_rail")
                    .show(ui, |ui| {
                        ui.set_width(area.width());
                        ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
                            ui.set_width(avatar_size);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = metrics.avatar_list_spacing;
                                ui.add_space(metrics.avatar_list_pad);
                                for &leader in sow_data::Leader::ALL.iter() {
                                    if draw_leader_avatar_button(
                                        ui,
                                        leader,
                                        *selected_leader,
                                        avatar_size,
                                        asset_loader,
                                    ) {
                                        *selected_leader = leader;
                                        *selected_civilization = leader_civilization(leader);
                                    }
                                }
                                ui.add_space(metrics.avatar_list_pad);
                            });
                        });
                    })
            })
            .inner;

        store_picker_scroll_id(ui.ctx(), DESKTOP_RAIL_SCROLL_KEY, scroll_output.id);

        draw_left_vertical_scrollbar(
            ui,
            scroll_bar_lane,
            area,
            scroll_output.inner_rect.height(),
            scroll_output.content_size.y,
            scroll_output.id,
        );
    });
}

fn draw_leader_carousel(
    ui: &mut egui::Ui,
    selected_leader: &mut sow_data::Leader,
    selected_civilization: &mut sow_data::Civilization,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    metrics: &LeaderPickerMetrics,
    scroll_area_h: f32,
    panel_w: f32,
) {
    let avatar_size = metrics.avatar_size;
    let leader_count = sow_data::Leader::ALL.len() as f32;
    let spacing = metrics.carousel_spacing;
    let total_carousel_w = (avatar_size + spacing) * leader_count - spacing;

    ui.allocate_ui(egui::vec2(panel_w, scroll_area_h), |ui| {
        // Content drag steals taps on avatars; use the scroll bar + wheel instead.
        let scroll_output = egui::ScrollArea::horizontal()
            .id_salt("leader_mobile_carousel")
            .scroll_source(egui::scroll_area::ScrollSource {
                scroll_bar: true,
                drag: false,
                mouse_wheel: true,
            })
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = spacing;

                    for &leader in sow_data::Leader::ALL.iter() {
                        if draw_leader_avatar_button(
                            ui,
                            leader,
                            *selected_leader,
                            avatar_size,
                            asset_loader,
                        ) {
                            *selected_leader = leader;
                            *selected_civilization = leader_civilization(leader);
                        }
                    }
                    if panel_w > total_carousel_w {
                        ui.add_space(spacing);
                    }
                });
            });
        store_picker_scroll_id(ui.ctx(), MOBILE_CAROUSEL_SCROLL_KEY, scroll_output.id);
    });
}

fn draw_leader_picker_back_button(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    rect: Rect,
) -> egui::Response {
    let btn = crate::widgets::ThemeButton::new("◀")
        .style(crate::widgets::ThemeButtonStyle::Tertiary)
        .custom_fill(crate::ui::theme::palette::button_inactive())
        .min_size(rect.size())
        .text_size(16.0);
    ui.put(rect, btn)
}

fn draw_leader_picker_top_column(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    metrics: &LeaderPickerMetrics,
    _content_max_x: f32,
) -> (bool, f32) {
    let back_rect = crate::ui::main_menu::profile::main_menu_avatar_button_rect(ctx);
    let back_response = draw_leader_picker_back_button(ui, ctx, back_rect);

    let font_id = egui::FontId::proportional(metrics.title_font);
    let galley = ui.painter().layout_no_wrap("CHOOSE YOUR LEADER".to_owned(), font_id, Color32::WHITE);
    let text_size = galley.size();

    let title_top = back_rect.center().y - text_size.y * 0.5;
    let title_rect = Rect::from_min_size(
        egui::pos2(back_rect.max.x + metrics.title_gap, title_top),
        text_size,
    );

    if ui.is_rect_visible(title_rect) {
        crate::ui::theme::paint_premium_glow_galley(
            ui.painter(),
            title_rect.left_top(),
            galley,
            Color32::WHITE,
            Color32::BLACK,
        );
    }
    ui.allocate_rect(title_rect, egui::Sense::hover());

    let column_bottom = back_rect.max.y.max(title_rect.max.y);
    (back_response.clicked(), column_bottom)
}

