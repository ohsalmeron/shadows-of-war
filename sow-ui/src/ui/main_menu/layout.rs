// Layout chrome helper for waiting room and subviews

pub(crate) fn menu_layout_chrome(
    ctx: &egui::Context,
    panel_h: f32,
    available_w: f32,
    compact: bool,
) -> (f32, f32, f32, f32) {
    let scale = sow_ui_kit::theme::viewport_scale(ctx);
    let portrait = sow_ui_kit::theme::portrait_layout(ctx);
    let mut section_gap = (if portrait {
        8.0
    } else if compact {
        10.0
    } else {
        14.0
    }) * scale;
    let mut action_min_h = (if portrait {
        44.0
    } else if compact {
        48.0
    } else {
        52.0
    }) * scale;
    action_min_h = action_min_h.clamp(34.0, 56.0);
    let mut profile_height = (56.0 * scale).clamp(40.0, 56.0);
    if portrait {
        profile_height *= 0.85;
    }

    let mut lobby_h = crate::ui::map_texture::thumbnail_square_side(available_w, compact);
    if portrait {
        lobby_h = (lobby_h * 0.55).clamp(110.0, 160.0);
    }

    let needed = if portrait {
        profile_height + section_gap * 2.0 + action_min_h * 2.0 + lobby_h
    } else {
        profile_height + section_gap * 2.0 + action_min_h.max(lobby_h)
    };
    let shrink = sow_ui_kit::theme::fit_scale(needed, panel_h);
    if shrink < 1.0 {
        section_gap *= shrink;
        action_min_h = (action_min_h * shrink).max(32.0);
        profile_height *= shrink;
        lobby_h *= shrink;
    }
    (section_gap, action_min_h, profile_height, lobby_h)
}
