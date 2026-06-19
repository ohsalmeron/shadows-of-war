use egui::{emath::remap, lerp, Color32, Rangef, Rect, Sense, Stroke};

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
            title_font: s(if is_mobile { 24.0 } else { 36.0 }),
            title_gap: s(if is_mobile { 18.0 } else { 30.0 }),
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
