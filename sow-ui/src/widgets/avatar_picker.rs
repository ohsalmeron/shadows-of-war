use egui::{emath::remap, lerp, Color32, Rangef, Rect, Sense, Stroke};
use std::sync::Once;

const LEADER_SELECT_GROW: f32 = 0.1;
const LEADER_SELECT_ANIM_SECS: f32 = 0.25;

const RAIL_SCROLLBAR_GAP: f32 = 12.0;
const RAIL_BAR_LANE: f32 = 10.0;
const RAIL_HEADER_GAP: f32 = 12.0;
const RAIL_SCROLL_TRACK_TOP_PAD: f32 = 10.0;

pub(crate) fn leader_rail_width(avatar_size: f32) -> f32 {
    RAIL_BAR_LANE + RAIL_SCROLLBAR_GAP + avatar_size
}

pub(crate) fn leader_rail_scroll_extent(avatar_size: f32) -> f32 {
    leader_rail_width(avatar_size) - avatar_size
}

fn draw_left_vertical_scrollbar(
    ui: &mut egui::Ui,
    bar_lane_rect: Rect,
    scroll_outer_rect: Rect,
    viewport_h: f32,
    content_h: f32,
    scroll_id: egui::Id,
) {
    let max_offset = (content_h - viewport_h).max(0.0);
    if max_offset <= 0.0 {
        return;
    }

    let scroll_style = ui.spacing().scroll;
    let mut state = egui::scroll_area::State::load(ui.ctx(), scroll_id).unwrap_or_default();

    let is_hovering_outer_rect = ui.rect_contains_pointer(scroll_outer_rect)
        || ui.rect_contains_pointer(bar_lane_rect);

    let outer_margin = scroll_style.bar_outer_margin;
    let full_width = scroll_style.bar_width;
    let max_bar_rect = Rect::from_min_max(
        egui::pos2(
            bar_lane_rect.min.x + outer_margin,
            bar_lane_rect.min.y + outer_margin,
        ),
        egui::pos2(
            bar_lane_rect.max.x - outer_margin,
            bar_lane_rect.max.y - outer_margin,
        ),
    );

    let response = ui.interact(
        max_bar_rect,
        scroll_id.with("left_v_bar"),
        Sense::click_and_drag(),
    );

    let is_hovering_bar_area = response.hovered() || response.dragged();
    let is_hovering_bar_area_t = ui.ctx().animate_bool_responsive(
        scroll_id.with((1_usize, "bar_hover")),
        is_hovering_bar_area,
    );

    let width = lerp(
        scroll_style.floating_width..=full_width,
        is_hovering_bar_area_t,
    );
    let min_cross = bar_lane_rect.min.x + outer_margin;
    let cross = Rangef::new(min_cross, min_cross + width);

    let scroll_track = Rect::from_min_max(
        egui::pos2(
            bar_lane_rect.min.x + outer_margin,
            bar_lane_rect.min.y + outer_margin,
        ),
        egui::pos2(
            bar_lane_rect.max.x - outer_margin,
            bar_lane_rect.max.y - outer_margin,
        ),
    );

    let handle_len =
        (viewport_h / content_h * scroll_track.height()).max(scroll_style.handle_min_length);
    let handle_travel = (scroll_track.height() - handle_len).max(0.0);
    let handle_top = scroll_track.top()
        + if handle_travel > 0.0 {
            (state.offset.y / max_offset) * handle_travel
        } else {
            0.0
        };
    let outer_scroll_bar_rect = Rect::from_x_y_ranges(cross, scroll_track.y_range());
    let handle_rect = Rect::from_min_max(
        egui::pos2(cross.min, handle_top),
        egui::pos2(cross.max, handle_top + handle_len),
    );

    let handle_opacity = if response.hovered() || response.dragged() {
        scroll_style.interact_handle_opacity
    } else {
        let is_hovering_outer_rect_t = ui.ctx().animate_bool_responsive(
            scroll_id.with((1_usize, "is_hovering_outer_rect")),
            is_hovering_outer_rect,
        );
        lerp(
            scroll_style.dormant_handle_opacity..=scroll_style.active_handle_opacity,
            is_hovering_outer_rect_t,
        )
    };

    let background_opacity = if response.hovered() || response.dragged() {
        scroll_style.interact_background_opacity
    } else if is_hovering_outer_rect {
        scroll_style.active_background_opacity
    } else {
        scroll_style.dormant_background_opacity
    };

    if handle_response_drag_or_track_click(
        ui,
        &mut state,
        scroll_id,
        &response,
        handle_rect,
        scroll_track,
        handle_len,
        handle_travel,
        max_offset,
    ) {
        ui.ctx().request_repaint();
    }

    if handle_opacity <= 0.001 && background_opacity <= 0.001 {
        return;
    }

    let visuals = &ui.visuals().widgets.inactive;
    let accent = crate::ui::theme::accent_solo_cyan();
    let track_color = accent.linear_multiply(0.28);

    if background_opacity > 0.0 {
        ui.painter().rect_filled(
            outer_scroll_bar_rect,
            visuals.corner_radius,
            track_color.gamma_multiply(background_opacity),
        );
    }

    if handle_opacity > 0.0 {
        ui.painter().rect_filled(
            handle_rect,
            visuals.corner_radius,
            accent.gamma_multiply(handle_opacity),
        );
    }
}

fn handle_response_drag_or_track_click(
    ui: &mut egui::Ui,
    state: &mut egui::scroll_area::State,
    scroll_id: egui::Id,
    bar_response: &egui::Response,
    handle_rect: Rect,
    scroll_track: Rect,
    handle_len: f32,
    handle_travel: f32,
    max_offset: f32,
) -> bool {
    let mut changed = false;

    if bar_response.dragged() {
        if let Some(pos) = bar_response.interact_pointer_pos() {
            let drag_offset_id = scroll_id.with("left_v_drag_offset");
            if bar_response.drag_started() {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(drag_offset_id, pos.y - handle_rect.top());
                });
            }
            let drag_offset = ui
                .ctx()
                .data(|d| d.get_temp(drag_offset_id))
                .unwrap_or(handle_len * 0.5);
            let new_top = (pos.y - drag_offset)
                .clamp(scroll_track.top(), scroll_track.top() + handle_travel);
            state.offset.y = if handle_travel > 0.0 {
                remap(
                    new_top,
                    scroll_track.top()..=scroll_track.top() + handle_travel,
                    0.0..=max_offset,
                )
            } else {
                0.0
            };
            state.store(ui.ctx(), scroll_id);
            changed = true;
        }
    } else if bar_response.clicked() {
        if let Some(pos) = bar_response.interact_pointer_pos() {
            if !handle_rect.contains(pos) {
                let new_top = (pos.y - handle_len * 0.5)
                    .clamp(scroll_track.top(), scroll_track.top() + handle_travel);
                state.offset.y = if handle_travel > 0.0 {
                    remap(
                        new_top,
                        scroll_track.top()..=scroll_track.top() + handle_travel,
                        0.0..=max_offset,
                    )
                } else {
                    0.0
                };
                state.store(ui.ctx(), scroll_id);
                changed = true;
            }
        }
    }

    changed
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverAnchor {
    Center,
    TopRight,
    TopCenter,
    BottomCenter,
    /// Full texture height; crop horizontally from center (desktop leader portraits).
    CenterFullHeight,
    /// Full texture height; slight right bias so hero portrait stays visible.
    CenterFullHeightHeroRight,
}

pub(crate) fn calculate_cover_uv(
    rect_size: egui::Vec2,
    tex_size: egui::Vec2,
    anchor: CoverAnchor,
) -> egui::Rect {
    let rect_aspect = rect_size.x / rect_size.y;
    let tex_aspect = tex_size.x / tex_size.y;

    if tex_aspect > rect_aspect {
        // Texture is wider than destination rect. Crop horizontally.
        let u_width = rect_aspect / tex_aspect;
        let u_start = match anchor {
            CoverAnchor::Center
            | CoverAnchor::BottomCenter
            | CoverAnchor::TopCenter
            | CoverAnchor::CenterFullHeight => (1.0 - u_width) / 2.0,
            CoverAnchor::CenterFullHeightHeroRight => {
                ((1.0 - u_width) / 2.0 + 0.08).clamp(0.0, 1.0 - u_width)
            }
            CoverAnchor::TopRight => 1.0 - u_width,
        };
        egui::Rect::from_min_max(egui::pos2(u_start, 0.0), egui::pos2(u_start + u_width, 1.0))
    } else {
        // Texture is taller than destination rect.
        match anchor {
            CoverAnchor::CenterFullHeight | CoverAnchor::CenterFullHeightHeroRight => {
                // Height-fit: show the full portrait height, crop sides from center.
                let u_width = rect_aspect / tex_aspect;
                if u_width <= 1.0 {
                    let u_start = match anchor {
                        CoverAnchor::CenterFullHeightHeroRight => {
                            ((1.0 - u_width) / 2.0 + 0.08).clamp(0.0, 1.0 - u_width)
                        }
                        _ => (1.0 - u_width) / 2.0,
                    };
                    egui::Rect::from_min_max(
                        egui::pos2(u_start, 0.0),
                        egui::pos2(u_start + u_width, 1.0),
                    )
                } else {
                    // Rare: still need vertical crop while filling viewport height.
                    let v_height = tex_aspect / rect_aspect;
                    let v_start = (1.0 - v_height) / 2.0;
                    egui::Rect::from_min_max(
                        egui::pos2(0.0, v_start),
                        egui::pos2(1.0, v_start + v_height),
                    )
                }
            }
            _ => {
                let v_height = tex_aspect / rect_aspect;
                let v_start = match anchor {
                    CoverAnchor::Center
                    | CoverAnchor::CenterFullHeight
                    | CoverAnchor::CenterFullHeightHeroRight => (1.0 - v_height) / 2.0,
                    CoverAnchor::TopRight | CoverAnchor::TopCenter => 0.0,
                    CoverAnchor::BottomCenter => 1.0 - v_height,
                };
                egui::Rect::from_min_max(
                    egui::pos2(0.0, v_start),
                    egui::pos2(1.0, v_start + v_height),
                )
            }
        }
    }
}

pub(crate) fn leader_background_cover_uv(
    rect_size: egui::Vec2,
    tex_size: egui::Vec2,
    mobile: bool,
) -> egui::Rect {
    calculate_cover_uv(
        rect_size,
        tex_size,
        if mobile {
            CoverAnchor::TopCenter
        } else {
            CoverAnchor::CenterFullHeightHeroRight
        },
    )
}

fn draw_leader_hero_text(
    ui: &mut egui::Ui,
    selected_leader: sow_core::player::Leader,
    selected_civilization: sow_core::player::Civilization,
    reign_dates: &str,
    text_w: f32,
    is_mobile: bool,
    center: bool,
) {
    let layout = if center {
        egui::Layout::top_down(egui::Align::Center)
    } else {
        egui::Layout::top_down(egui::Align::LEFT)
    };
    ui.with_layout(layout, |ui| {
        ui.set_width(text_w);
        ui.spacing_mut().item_spacing.y = 8.0;
        crate::ui::theme::leader_name_label(
            ui,
            selected_leader.name(),
            if is_mobile { 32.0 } else { 48.0 },
        );
        crate::ui::theme::leader_caps_line(
            ui,
            &format!(
                "{} • {}",
                selected_civilization.name(),
                reign_dates
            ),
            if is_mobile { 15.0 } else { 18.0 },
        );
        crate::ui::theme::leader_caps_paragraph(
            ui,
            selected_leader.perk_description(),
            if is_mobile { 15.0 } else { 16.0 },
            text_w,
        );
    });
}

fn paint_horizontal_gradient_rect(
    painter: &egui::Painter,
    rect: egui::Rect,
    left: Color32,
    right: Color32,
) {
    if !rect.is_positive() {
        return;
    }
    let mut mesh = egui::Mesh::default();
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_top(),
        uv: egui::Pos2::ZERO,
        color: left,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_top(),
        uv: egui::Pos2::ZERO,
        color: right,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_bottom(),
        uv: egui::Pos2::ZERO,
        color: right,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_bottom(),
        uv: egui::Pos2::ZERO,
        color: left,
    });
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

fn paint_vertical_gradient_rect(
    painter: &egui::Painter,
    rect: egui::Rect,
    top: Color32,
    bottom: Color32,
) {
    if !rect.is_positive() {
        return;
    }
    let mut mesh = egui::Mesh::default();
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_top(),
        uv: egui::Pos2::ZERO,
        color: top,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_top(),
        uv: egui::Pos2::ZERO,
        color: top,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_bottom(),
        uv: egui::Pos2::ZERO,
        color: bottom,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_bottom(),
        uv: egui::Pos2::ZERO,
        color: bottom,
    });
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

fn leader_picker_column_bottom(ctx: &egui::Context, is_mobile: bool, content_max_x: f32) -> f32 {
    let back_rect = crate::ui::main_menu::profile::main_menu_avatar_button_rect(ctx);
    let title_gap = 12.0;
    let title_font = if is_mobile { 20.0 } else { 28.0 };
    let title_line_h = title_font + 10.0;
    let title_top = back_rect.center().y - title_line_h * 0.5;
    let title_rect = Rect::from_min_max(
        egui::pos2(back_rect.max.x + title_gap, title_top),
        egui::pos2(content_max_x, title_top + title_line_h),
    );
    back_rect.max.y.max(title_rect.max.y)
}

fn draw_leader_picker_overlay_gradient(
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
        paint_vertical_gradient_rect(painter, gradient_rect, Color32::TRANSPARENT, dark);
    } else {
        let panel_w = screen_rect.width() * PANEL_FRAC;
        let gradient_rect = Rect::from_min_max(
            screen_rect.min,
            egui::pos2(screen_rect.min.x + panel_w, screen_rect.max.y),
        );
        paint_horizontal_gradient_rect(painter, gradient_rect, dark, Color32::TRANSPARENT);
    }
}

fn leader_civilization(leader: sow_core::player::Leader) -> sow_core::player::Civilization {
    match leader {
        sow_core::player::Leader::Caesar => sow_core::player::Civilization::Rome,
        sow_core::player::Leader::Cleopatra => sow_core::player::Civilization::Egypt,
        sow_core::player::Leader::Ragnar => sow_core::player::Civilization::Vikings,
        sow_core::player::Leader::SunTzu => sow_core::player::Civilization::China,
        sow_core::player::Leader::Alexander => sow_core::player::Civilization::Macedon,
        sow_core::player::Leader::GenghisKhan => sow_core::player::Civilization::Mongols,
        sow_core::player::Leader::RichardTheLionheart => sow_core::player::Civilization::Angevin,
        sow_core::player::Leader::Vercingetorix => sow_core::player::Civilization::Gallic,
        sow_core::player::Leader::Boudica => sow_core::player::Civilization::Iceni,
        sow_core::player::Leader::LadySixSky => sow_core::player::Civilization::Maya,
        sow_core::player::Leader::Leonidas => sow_core::player::Civilization::Sparta,
        sow_core::player::Leader::Napoleon => sow_core::player::Civilization::France,
    }
}

fn leader_filler_color32(leader: sow_core::player::Leader) -> Color32 {
    let [r, g, b] = leader.filler_rgb();
    Color32::from_rgb(
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    )
}

fn draw_leader_avatar_button(
    ui: &mut egui::Ui,
    leader: sow_core::player::Leader,
    selected_leader: sow_core::player::Leader,
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
    selected_leader: &mut sow_core::player::Leader,
    selected_civilization: &mut sow_core::player::Civilization,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    avatar_size: f32,
    rail_h: f32,
    scroll_track_top_pad: f32,
) {
    ui.allocate_ui(egui::vec2(ui.max_rect().width(), rail_h), |ui| {
        let area = ui.max_rect();
        // Avatars are pinned on the right; scrollbar sits to their left.
        let avatar_rect = Rect::from_min_size(
            egui::pos2(area.max.x - avatar_size, area.min.y),
            egui::vec2(avatar_size, rail_h),
        );
        let bar_lane_rect = Rect::from_min_max(
            egui::pos2(
                avatar_rect.min.x - RAIL_SCROLLBAR_GAP - RAIL_BAR_LANE,
                area.min.y,
            ),
            egui::pos2(avatar_rect.min.x - RAIL_SCROLLBAR_GAP, area.max.y),
        );
        let scroll_bar_lane = bar_lane_rect.shrink2(egui::vec2(0.0, scroll_track_top_pad));

        let scroll_output = ui
            .scope_builder(egui::UiBuilder::new().max_rect(avatar_rect), |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .scroll_source(egui::scroll_area::ScrollSource::MOUSE_WHEEL)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .id_salt("leader_desktop_rail")
                    .show(ui, |ui| {
                        ui.set_width(avatar_size);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 10.0;
                            ui.add_space(4.0);
                            for &leader in sow_core::player::Leader::ALL.iter() {
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
                            ui.add_space(4.0);
                        });
                    })
            })
            .inner;

        draw_left_vertical_scrollbar(
            ui,
            scroll_bar_lane,
            avatar_rect,
            scroll_output.inner_rect.height(),
            scroll_output.content_size.y,
            scroll_output.id,
        );
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
) {
    let leader_count = sow_core::player::Leader::ALL.len() as f32;
    let total_carousel_w = (avatar_size + 12.0) * leader_count - 12.0;

    ui.allocate_ui(egui::vec2(panel_w, scroll_area_h), |ui| {
            // Content drag steals taps on avatars; use the scroll bar + wheel instead.
            egui::ScrollArea::horizontal()
                .id_salt("leader_mobile_carousel")
                .scroll_source(egui::scroll_area::ScrollSource {
                    scroll_bar: true,
                    drag: false,
                    mouse_wheel: true,
                })
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;

                        for &leader in sow_core::player::Leader::ALL.iter() {
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
                            ui.add_space(12.0);
                        }
                    });
                });
        });
}

fn draw_leader_picker_back_button(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    rect: Rect,
) -> egui::Response {
    static REGISTER_BACK_ONCE: Once = Once::new();
    REGISTER_BACK_ONCE.call_once(|| {
        ctx.include_bytes(
            "bytes://back.svg",
            include_bytes!("../../../sow-client/assets/back.svg").as_slice(),
        );
    });

    let response = ui.put(
        rect,
        egui::Button::new("")
            .min_size(rect.size())
            .corner_radius(egui::CornerRadius::same(6))
            .fill(crate::ui::theme::palette::button_inactive()),
    );

    if ui.is_rect_visible(rect) {
        let icon_size = rect.width() * 0.5;
        if let Ok(egui::load::TexturePoll::Ready { texture }) = ctx.try_load_texture(
            "bytes://back.svg",
            egui::TextureOptions::default(),
            egui::load::SizeHint::Size {
                width: (icon_size * 2.0).round() as u32,
                height: (icon_size * 2.0).round() as u32,
                maintain_aspect_ratio: true,
            },
        ) {
            let icon_rect = Rect::from_center_size(response.rect.center(), egui::vec2(icon_size, icon_size));
            ui.put(
                icon_rect,
                egui::Image::new((texture.id, egui::vec2(icon_size, icon_size)))
                    .tint(Color32::WHITE),
            );
        }
    }

    response
}

fn draw_leader_picker_top_column(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    is_mobile: bool,
    content_max_x: f32,
) -> (bool, f32) {
    let back_rect = crate::ui::main_menu::profile::main_menu_avatar_button_rect(ctx);
    let title_gap = 12.0;
    let title_font = if is_mobile { 20.0 } else { 28.0 };
    let title_line_h = title_font + 10.0;
    let title_top = back_rect.center().y - title_line_h * 0.5;
    let title_rect = Rect::from_min_max(
        egui::pos2(back_rect.max.x + title_gap, title_top),
        egui::pos2(content_max_x, title_top + title_line_h),
    );

    let back_response = draw_leader_picker_back_button(ui, ctx, back_rect);

    ui.scope_builder(egui::UiBuilder::new().max_rect(title_rect), |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
            crate::ui::theme::outlined_label(
                ui,
                "CHOOSE YOUR LEADER",
                egui::FontId::proportional(title_font),
                Color32::WHITE,
            );
        });
    });

    let column_bottom = leader_picker_column_bottom(ctx, is_mobile, content_max_x);
    (back_response.clicked(), column_bottom)
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
            let is_mobile = crate::ui::theme::compact_viewport(ctx);

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
                content_rect.contains(click_pos)
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
                sow_core::player::Leader::Napoleon => "Reigned 1804 – 1814 AD",
            };

            // --- FULLSCREEN IMMERSIVE BACKDROP PORTRAIT (COVER ASPECT RATIO, NO STRETCH) ---
            if is_mobile {
                if let Some(tex) = asset_loader.leader_mobile_images.get(selected_leader) {
                    let uv = leader_background_cover_uv(screen_rect.size(), tex.size_vec2(), is_mobile);
                    ui.painter()
                        .image(tex.id(), screen_rect, uv, Color32::WHITE);
                }
            } else {
                if let Some(tex) = asset_loader.leader_desktop_images.get(selected_leader) {
                    let uv = leader_background_cover_uv(screen_rect.size(), tex.size_vec2(), is_mobile);
                    ui.painter()
                        .image(tex.id(), screen_rect, uv, Color32::WHITE);
                }
            }

            const CONFIRM_BTN_H: f32 = 44.0;
            const CONFIRM_BTN_W_MOBILE: f32 = 200.0;
            const CONFIRM_GAP: f32 = 12.0;
            const MOBILE_STACK_GAP: f32 = 12.0;
            const MOBILE_TEXT_STACK_H: f32 = 120.0;
            const RAIL_TEXT_GAP: f32 = 24.0;
            const DESKTOP_TEXT_W: f32 = 420.0;
            const DESKTOP_NARROW_HERO_H: f32 = 150.0;
            const DESKTOP_NARROW_BREAKPOINT: f32 = 1024.0;

            draw_leader_picker_overlay_gradient(ui.painter(), screen_rect, is_mobile);

            let avatar_size = if is_mobile { 64.0 } else { 54.0 };
            let scroll_area_h = avatar_size + 4.0;
            let carousel_block_h = scroll_area_h;
            let card_w = if is_mobile {
                content_rect.width()
            } else {
                DESKTOP_TEXT_W
            };
            let confirm_top = content_rect.max.y - CONFIRM_BTN_H - CONFIRM_GAP;
            let confirm_w = if is_mobile {
                CONFIRM_BTN_W_MOBILE
            } else {
                140.0
            };
            let confirm_x = if is_mobile {
                content_rect.center().x - confirm_w * 0.5
            } else {
                content_rect.max.x - confirm_w
            };

            let mut desktop_rail: Option<(egui::Rect, f32)> = None;

            ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                if is_mobile {
                    let (back, column_bottom) = draw_leader_picker_top_column(
                        ui,
                        ctx,
                        true,
                        content_rect.max.x,
                    );
                    if back {
                        close = true;
                    }

                    // Bottom → top: CONFIRM, avatar carousel, hero text, title (portrait fills middle).
                    let confirm_bottom = content_rect.max.y;
                    let confirm_top_y = confirm_bottom - CONFIRM_BTN_H;
                    let carousel_bottom = confirm_top_y - CONFIRM_GAP;
                    let carousel_top = carousel_bottom - carousel_block_h;
                    let text_bottom = carousel_top - MOBILE_STACK_GAP;
                    let text_top =
                        (text_bottom - MOBILE_TEXT_STACK_H).max(column_bottom + MOBILE_STACK_GAP);

                    let carousel_rect = egui::Rect::from_min_max(
                        egui::pos2(content_rect.min.x, carousel_top),
                        egui::pos2(content_rect.max.x, carousel_bottom),
                    );

                    if text_bottom > text_top {
                        let text_rect = egui::Rect::from_min_max(
                            egui::pos2(content_rect.min.x, text_top),
                            egui::pos2(content_rect.max.x, text_bottom),
                        );
                        ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
                            ui.with_layout(
                                egui::Layout::bottom_up(egui::Align::Center),
                                |ui| {
                                    ui.set_width(card_w);
                                    draw_leader_hero_text(
                                        ui,
                                        *selected_leader,
                                        *selected_civilization,
                                        reign_dates,
                                        card_w,
                                        true,
                                        true,
                                    );
                                },
                            );
                        });
                    }

                    ui.scope_builder(egui::UiBuilder::new().max_rect(carousel_rect), |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            ui.set_width(content_rect.width());
                            draw_leader_carousel(
                                ui,
                                selected_leader,
                                selected_civilization,
                                asset_loader,
                                avatar_size,
                                scroll_area_h,
                                content_rect.width(),
                            );
                        });
                    });
                } else {
                    let is_desktop_narrow = screen_rect.width() < DESKTOP_NARROW_BREAKPOINT;
                    let back_rect =
                        crate::ui::main_menu::profile::main_menu_avatar_button_rect(ctx);
                    let (back, column_bottom) = draw_leader_picker_top_column(
                        ui,
                        ctx,
                        false,
                        content_rect.max.x,
                    );
                    if back {
                        close = true;
                    }

                    let avatar_lane_x = back_rect.min.x;
                    let pick_top = column_bottom.max(back_rect.max.y) + RAIL_HEADER_GAP;
                    let pick_bottom = if is_desktop_narrow {
                        confirm_top - CONFIRM_GAP - DESKTOP_NARROW_HERO_H - MOBILE_STACK_GAP
                    } else {
                        confirm_top - CONFIRM_GAP
                    };
                    let pick_h = (pick_bottom - pick_top).max(avatar_size + 48.0);
                    let rail_rect = egui::Rect::from_min_max(
                        egui::pos2(avatar_lane_x - leader_rail_scroll_extent(avatar_size), pick_top),
                        egui::pos2(avatar_lane_x + avatar_size, pick_bottom),
                    );
                    desktop_rail = Some((rail_rect, pick_h));

                    if is_desktop_narrow {
                        let hero_left = avatar_lane_x + avatar_size + RAIL_TEXT_GAP;
                        let hero_right = (hero_left + card_w).min(content_rect.max.x);
                        let hero_rect = egui::Rect::from_min_max(
                            egui::pos2(hero_left, confirm_top - CONFIRM_GAP - DESKTOP_NARROW_HERO_H),
                            egui::pos2(hero_right, confirm_top - CONFIRM_GAP),
                        );
                        if hero_rect.height() > 0.0 && hero_rect.width() > 0.0 {
                            ui.scope_builder(egui::UiBuilder::new().max_rect(hero_rect), |ui| {
                                ui.with_layout(
                                    egui::Layout::bottom_up(egui::Align::LEFT),
                                    |ui| {
                                        ui.set_width(hero_rect.width());
                                        draw_leader_hero_text(
                                            ui,
                                            *selected_leader,
                                            *selected_civilization,
                                            reign_dates,
                                            hero_rect.width(),
                                            false,
                                            false,
                                        );
                                    },
                                );
                            });
                        }
                    } else {
                        let text_rect = egui::Rect::from_min_max(
                            egui::pos2(avatar_lane_x + avatar_size + RAIL_TEXT_GAP, pick_top),
                            egui::pos2(
                                (avatar_lane_x + avatar_size + RAIL_TEXT_GAP + card_w)
                                    .min(content_rect.max.x),
                                pick_top + pick_h,
                            ),
                        );

                        ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
                            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                                ui.set_width(text_rect.width());
                                draw_leader_hero_text(
                                    ui,
                                    *selected_leader,
                                    *selected_civilization,
                                    reign_dates,
                                    text_rect.width(),
                                    false,
                                    false,
                                );
                            });
                        });
                    }
                }

                let confirm_rect = egui::Rect::from_min_max(
                    egui::pos2(confirm_x, content_rect.max.y - CONFIRM_BTN_H),
                    egui::pos2(confirm_x + confirm_w, content_rect.max.y),
                );
                let confirm_button = if is_mobile {
                    crate::widgets::ThemeButton::new("CONFIRM")
                        .style(crate::widgets::ThemeButtonStyle::Primary)
                        .min_size(egui::vec2(confirm_w, CONFIRM_BTN_H))
                        .text_size(16.0)
                } else {
                    crate::widgets::ThemeButton::new("CONFIRM")
                        .custom_fill(Color32::from_black_alpha(175))
                        .min_size(egui::vec2(confirm_w, CONFIRM_BTN_H))
                        .text_size(16.0)
                };
                let confirm_response = ui.put(confirm_rect, confirm_button);
                if confirm_response.clicked() {
                    close = true;
                }
            });

            if let Some((rail_rect, pick_h)) = desktop_rail {
                ui.scope_builder(egui::UiBuilder::new().max_rect(rail_rect), |ui| {
                    draw_leader_rail(
                        ui,
                        selected_leader,
                        selected_civilization,
                        asset_loader,
                        avatar_size,
                        pick_h,
                        RAIL_SCROLL_TRACK_TOP_PAD,
                    );
                });
            }
        });

    close
}
