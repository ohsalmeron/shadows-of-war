use egui::{emath::remap, lerp, Color32, Rangef, Rect, Sense, Stroke};
use std::sync::Once;

const LEADER_SELECT_GROW: f32 = 0.1;
const LEADER_SELECT_ANIM_SECS: f32 = 0.25;

const DESKTOP_NARROW_BREAKPOINT: f32 = 1024.0;
const PICKER_VIEWPORT_KEY: &str = "leader_picker_viewport";
const DESKTOP_RAIL_SCROLL_KEY: &str = "leader_desktop_rail_scroll_id";
const MOBILE_CAROUSEL_SCROLL_KEY: &str = "leader_mobile_carousel_scroll_id";

struct LeaderPickerMetrics {
    is_mobile: bool,
    rail_scrollbar_gap: f32,
    rail_bar_lane: f32,
    rail_header_gap: f32,
    rail_scroll_track_top_pad: f32,
    confirm_btn_h: f32,
    confirm_btn_w_mobile: f32,
    confirm_btn_w_desktop: f32,
    confirm_gap: f32,
    mobile_stack_gap: f32,
    mobile_text_stack_h: f32,
    rail_text_gap: f32,
    desktop_text_w: f32,
    desktop_narrow_hero_h: f32,
    content_margin_x: f32,
    content_margin_top: f32,
    content_margin_bottom: f32,
    content_shrink: f32,
    avatar_size: f32,
    scroll_area_pad: f32,
    carousel_spacing: f32,
    title_font: f32,
    title_gap: f32,
    title_line_extra: f32,
    name_font: f32,
    caps_line_font: f32,
    caps_para_font: f32,
    confirm_text_size: f32,
    hero_text_spacing: f32,
    avatar_list_spacing: f32,
    avatar_list_pad: f32,
    rail_min_extra: f32,
}

impl LeaderPickerMetrics {
    fn new(ctx: &egui::Context) -> Self {
        let scale = crate::ui::theme::viewport_scale(ctx);
        let is_mobile = crate::ui::theme::compact_viewport(ctx);
        let s = |v: f32| v * scale;
        Self {
            is_mobile,
            rail_scrollbar_gap: s(12.0),
            rail_bar_lane: s(10.0),
            rail_header_gap: s(12.0),
            rail_scroll_track_top_pad: s(10.0),
            confirm_btn_h: s(44.0),
            confirm_btn_w_mobile: s(200.0),
            confirm_btn_w_desktop: s(140.0),
            confirm_gap: s(12.0),
            mobile_stack_gap: s(12.0),
            mobile_text_stack_h: s(120.0),
            rail_text_gap: s(24.0),
            desktop_text_w: s(420.0),
            desktop_narrow_hero_h: s(150.0),
            content_margin_x: s(24.0),
            content_margin_top: s(56.0),
            content_margin_bottom: s(36.0),
            content_shrink: s(40.0),
            avatar_size: s(if is_mobile { 64.0 } else { 54.0 }),
            scroll_area_pad: s(4.0),
            carousel_spacing: s(12.0),
            title_font: s(if is_mobile { 20.0 } else { 28.0 }),
            title_gap: s(12.0),
            title_line_extra: s(10.0),
            name_font: s(if is_mobile { 32.0 } else { 48.0 }),
            caps_line_font: s(if is_mobile { 15.0 } else { 18.0 }),
            caps_para_font: s(if is_mobile { 15.0 } else { 16.0 }),
            confirm_text_size: s(16.0),
            hero_text_spacing: s(8.0),
            avatar_list_spacing: s(10.0),
            avatar_list_pad: s(4.0),
            rail_min_extra: s(48.0),
        }
    }

    fn rail_scroll_extent(&self) -> f32 {
        self.rail_bar_lane + self.rail_scrollbar_gap
    }
}

fn reset_picker_scroll_if_resized(ctx: &egui::Context) {
    let size = ctx.content_rect().size();
    let viewport_id = egui::Id::new(PICKER_VIEWPORT_KEY);
    let prev = ctx.data(|d| d.get_temp::<egui::Vec2>(viewport_id));
    let changed = prev.is_some_and(|p| (p.x - size.x).abs() > 2.0 || (p.y - size.y).abs() > 2.0);
    if changed {
        for key in [DESKTOP_RAIL_SCROLL_KEY, MOBILE_CAROUSEL_SCROLL_KEY] {
            if let Some(scroll_id) = ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new(key))) {
                egui::scroll_area::State::default().store(ctx, scroll_id);
            }
        }
    }
    ctx.data_mut(|d| d.insert_temp(viewport_id, size));
}

fn store_picker_scroll_id(ctx: &egui::Context, key: &str, scroll_id: egui::Id) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(key), scroll_id));
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

    let is_hovering_outer_rect =
        ui.rect_contains_pointer(scroll_outer_rect) || ui.rect_contains_pointer(bar_lane_rect);

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
    let is_hovering_bar_area_t = ui
        .ctx()
        .animate_bool_responsive(scroll_id.with((1_usize, "bar_hover")), is_hovering_bar_area);

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
    let accent = crate::ui::theme::palette::neon_cyan();
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

#[allow(clippy::too_many_arguments)]
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
            let new_top =
                (pos.y - drag_offset).clamp(scroll_track.top(), scroll_track.top() + handle_travel);
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

fn leader_picker_column_bottom(
    ctx: &egui::Context,
    metrics: &LeaderPickerMetrics,
    content_max_x: f32,
) -> f32 {
    let back_rect = crate::ui::main_menu::profile::main_menu_avatar_button_rect(ctx);
    let title_line_h = metrics.title_font + metrics.title_line_extra;
    let title_top = back_rect.center().y - title_line_h * 0.5;
    let title_rect = Rect::from_min_max(
        egui::pos2(back_rect.max.x + metrics.title_gap, title_top),
        egui::pos2(content_max_x, title_top + title_line_h),
    );
    back_rect.max.y.max(title_rect.max.y)
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

pub(crate) fn leader_civilization(leader: sow_core::player::Leader) -> sow_core::player::Civilization {
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
    selected_leader: &mut sow_core::player::Leader,
    selected_civilization: &mut sow_core::player::Civilization,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    metrics: &LeaderPickerMetrics,
    scroll_area_h: f32,
    panel_w: f32,
) {
    let avatar_size = metrics.avatar_size;
    let leader_count = sow_core::player::Leader::ALL.len() as f32;
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
                        ui.add_space(spacing);
                    }
                });
            });
        store_picker_scroll_id(ui.ctx(), MOBILE_CAROUSEL_SCROLL_KEY, scroll_output.id);
    });
}

fn draw_leader_picker_back_button(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    rect: Rect,
) -> egui::Response {
    let response = ui.put(
        rect,
        egui::Button::new("◀")
            .min_size(rect.size())
            .corner_radius(egui::CornerRadius::same(6))
            .fill(crate::ui::theme::palette::button_inactive()),
    );
    response
}

fn draw_leader_picker_top_column(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    metrics: &LeaderPickerMetrics,
    content_max_x: f32,
) -> (bool, f32) {
    let back_rect = crate::ui::main_menu::profile::main_menu_avatar_button_rect(ctx);
    let title_line_h = metrics.title_font + metrics.title_line_extra;
    let title_top = back_rect.center().y - title_line_h * 0.5;
    let title_rect = Rect::from_min_max(
        egui::pos2(back_rect.max.x + metrics.title_gap, title_top),
        egui::pos2(content_max_x, title_top + title_line_h),
    );

    let back_response = draw_leader_picker_back_button(ui, ctx, back_rect);

    ui.scope_builder(egui::UiBuilder::new().max_rect(title_rect), |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
            crate::ui::theme::outlined_label(
                ui,
                "CHOOSE YOUR LEADER",
                egui::FontId::proportional(metrics.title_font),
                Color32::WHITE,
            );
        });
    });

    let column_bottom = leader_picker_column_bottom(ctx, metrics, content_max_x);
    (back_response.clicked(), column_bottom)
}

pub fn draw_leader_picker_modal(
    ctx: &egui::Context,
    selected_leader: &mut sow_core::player::Leader,
    selected_civilization: &mut sow_core::player::Civilization,
    asset_loader: &mut crate::ui::asset_loader::AssetLoader,
    leader_backdrop: &mut crate::widgets::leader_backdrop::LeaderBackdropTransition,
    lang: sow_i18n::Language,
) -> bool {
    let mut close = false;
    reset_picker_scroll_if_resized(ctx);

    egui::Area::new(egui::Id::new("leader_picker_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let screen_rect = ctx.content_rect();
            let metrics = LeaderPickerMetrics::new(ctx);
            let is_mobile = metrics.is_mobile;

            let content_rect = if is_mobile {
                let mut rect = screen_rect;
                rect.min.x += metrics.content_margin_x;
                rect.max.x -= metrics.content_margin_x;
                rect.min.y += metrics.content_margin_top;
                rect.max.y -= metrics.content_margin_bottom;
                rect
            } else {
                screen_rect.shrink(metrics.content_shrink)
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

            let loading_label = &sow_i18n::get(lang).main_menu.loading_leader_portrait;
            crate::widgets::leader_backdrop::draw_leader_hero_backdrop(
                ui,
                screen_rect,
                *selected_leader,
                is_mobile,
                asset_loader,
                leader_backdrop,
                loading_label,
                true,
            );

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

            let avatar_size = metrics.avatar_size;
            let scroll_area_h = avatar_size + metrics.scroll_area_pad;
            let carousel_block_h = scroll_area_h;
            let card_w = if is_mobile {
                content_rect.width()
            } else {
                metrics.desktop_text_w
            };
            let confirm_top = content_rect.max.y - metrics.confirm_btn_h - metrics.confirm_gap;
            let confirm_w = if is_mobile {
                metrics.confirm_btn_w_mobile
            } else {
                metrics.confirm_btn_w_desktop
            };
            let confirm_x = if is_mobile {
                content_rect.center().x - confirm_w * 0.5
            } else {
                content_rect.max.x - confirm_w
            };

            let mut desktop_rail: Option<(egui::Rect, f32)> = None;

            ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                if is_mobile {
                    let (back, column_bottom) =
                        draw_leader_picker_top_column(ui, ctx, &metrics, content_rect.max.x);
                    if back {
                        close = true;
                    }

                    // Bottom → top: CONFIRM, avatar carousel, hero text, title (portrait fills middle).
                    let confirm_bottom = content_rect.max.y;
                    let confirm_top_y = confirm_bottom - metrics.confirm_btn_h;
                    let carousel_bottom = confirm_top_y - metrics.confirm_gap;
                    let carousel_top = carousel_bottom - carousel_block_h;
                    let text_bottom = carousel_top - metrics.mobile_stack_gap;
                    let text_top = (text_bottom - metrics.mobile_text_stack_h)
                        .max(column_bottom + metrics.mobile_stack_gap);

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
                            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                                ui.set_width(card_w);
                                draw_leader_hero_text(
                                    ui,
                                    *selected_leader,
                                    *selected_civilization,
                                    reign_dates,
                                    card_w,
                                    &metrics,
                                    true,
                                );
                            });
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
                                &metrics,
                                scroll_area_h,
                                content_rect.width(),
                            );
                        });
                    });
                } else {
                    let is_desktop_narrow = screen_rect.width() < DESKTOP_NARROW_BREAKPOINT;
                    let back_rect =
                        crate::ui::main_menu::profile::main_menu_avatar_button_rect(ctx);
                    let (back, column_bottom) =
                        draw_leader_picker_top_column(ui, ctx, &metrics, content_rect.max.x);
                    if back {
                        close = true;
                    }

                    let avatar_lane_x = back_rect.min.x;
                    let pick_top = column_bottom.max(back_rect.max.y) + metrics.rail_header_gap;
                    let pick_bottom = confirm_top - metrics.confirm_gap;
                    let pick_h = (pick_bottom - pick_top).max(avatar_size + metrics.rail_min_extra);
                    let rail_rect = egui::Rect::from_min_max(
                        egui::pos2(avatar_lane_x - metrics.rail_scroll_extent(), pick_top),
                        egui::pos2(avatar_lane_x + avatar_size, pick_bottom),
                    );
                    desktop_rail = Some((rail_rect, pick_h));

                    if is_desktop_narrow {
                        let hero_left = avatar_lane_x + avatar_size + metrics.rail_text_gap;
                        let hero_right = (hero_left + card_w).min(content_rect.max.x);
                        let hero_rect = egui::Rect::from_min_max(
                            egui::pos2(
                                hero_left,
                                confirm_top - metrics.confirm_gap - metrics.desktop_narrow_hero_h,
                            ),
                            egui::pos2(hero_right, confirm_top - metrics.confirm_gap),
                        );
                        if hero_rect.height() > 0.0 && hero_rect.width() > 0.0 {
                            ui.scope_builder(egui::UiBuilder::new().max_rect(hero_rect), |ui| {
                                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                                    ui.set_width(hero_rect.width());
                                    draw_leader_hero_text(
                                        ui,
                                        *selected_leader,
                                        *selected_civilization,
                                        reign_dates,
                                        hero_rect.width(),
                                        &metrics,
                                        false,
                                    );
                                });
                            });
                        }
                    } else {
                        let text_rect = egui::Rect::from_min_max(
                            egui::pos2(
                                avatar_lane_x + avatar_size + metrics.rail_text_gap,
                                pick_top,
                            ),
                            egui::pos2(
                                (avatar_lane_x + avatar_size + metrics.rail_text_gap + card_w)
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
                                    &metrics,
                                    false,
                                );
                            });
                        });
                    }
                }

                let confirm_rect = egui::Rect::from_min_max(
                    egui::pos2(confirm_x, content_rect.max.y - metrics.confirm_btn_h),
                    egui::pos2(confirm_x + confirm_w, content_rect.max.y),
                );
                let confirm_button = if is_mobile {
                    crate::widgets::ThemeButton::new("CONFIRM")
                        .style(crate::widgets::ThemeButtonStyle::Primary)
                        .min_size(egui::vec2(confirm_w, metrics.confirm_btn_h))
                        .text_size(metrics.confirm_text_size)
                } else {
                    crate::widgets::ThemeButton::new("CONFIRM")
                        .custom_fill(Color32::from_black_alpha(175))
                        .min_size(egui::vec2(confirm_w, metrics.confirm_btn_h))
                        .text_size(metrics.confirm_text_size)
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
                        &metrics,
                        pick_h,
                    );
                });
            }
        });

    close
}
