// Unused imports removed

pub(crate) fn menu_footer_height(section_gap: f32, action_min_h: f32, scale: f32) -> f32 {
    let settings_h = action_min_h * 0.75;
    action_min_h // Solo button
        + section_gap
        + action_min_h // Create button
        + section_gap
        + action_min_h // Join button
        + section_gap
        + settings_h // Settings button
        + 6.0 * scale
}

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
        12.0
    } else {
        16.0
    }) * scale;
    let mut action_min_h = (if portrait {
        54.0
    } else if compact {
        64.0
    } else {
        72.0
    }) * scale;
    let mut profile_height = 56.0 * scale;
    if portrait {
        profile_height *= 0.85;
    }

    let mut lobby_h = crate::ui::map_texture::thumbnail_square_side(available_w, compact);
    if portrait {
        lobby_h = (lobby_h * 0.55).clamp(110.0, 160.0);
    }

    let footer_h = menu_footer_height(section_gap, action_min_h, scale);
    let header_h = profile_height + section_gap;
    let needed = if portrait {
        header_h + section_gap + footer_h + section_gap + lobby_h
    } else {
        header_h + section_gap + footer_h.max(lobby_h)
    };
    let shrink = sow_ui_kit::theme::fit_scale(needed, panel_h);
    if shrink < 1.0 {
        section_gap *= shrink;
        action_min_h *= shrink;
        profile_height *= shrink;
        lobby_h *= shrink;
    }
    (section_gap, action_min_h, profile_height, lobby_h)
}


