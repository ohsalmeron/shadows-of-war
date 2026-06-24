/// Damped spring overshoot: approaches 1.0 with a single bounce.
#[inline]
pub(crate) fn spring_overshoot(t: f32) -> f32 {
    1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
}

/// Draws a floating emoji status icon (Request, Handshake, Betrayal) with spring entrance animation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_floating_status_emoji(
    painter: &egui::Painter,
    center: egui::Pos2,
    player_id: u16,
    is_me: bool,
    font_size: f32,
    content_h: f32,
    active: bool,
    anim_id_str: &'static str,
    emoji: &str,
    layer_id_str: &'static str,
    color_glow: Option<egui::Color32>,
    flash_alpha: f32,
) -> f32 {
    let anim_id = egui::Id::new((anim_id_str, player_id));
    if !crate::app::vfx_on(painter.ctx(), |f| f.status_emojis) {
        return 0.0;
    }
    let anim = painter.ctx().animate_bool_with_time(anim_id, active, 0.25);
    if anim <= 0.01 {
        return 0.0;
    }

    let base_icon_size = font_size * 2.5;
    let anim_scale = if active {
        if anim >= 1.0 {
            1.0
        } else {
            spring_overshoot(anim)
        }
    } else {
        anim
    };
    let size = (base_icon_size * anim_scale).round();
    if size <= 1.0 {
        return 0.0;
    }

    let req_y = center.y - (content_h / 2.0) - (font_size * 0.30).round() - size / 2.0;
    let req_rect =
        egui::Rect::from_center_size(egui::pos2(center.x, req_y), egui::vec2(size, size));

    let icon_painter = painter.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new((layer_id_str, player_id)),
    ));

    if is_me {
        if let Some(glow_color) = color_glow {
            let glow_r = size * 0.8;
            let glow_a = anim * flash_alpha * 0.35;
            icon_painter.circle_filled(
                req_rect.center(),
                glow_r * 1.4,
                egui::Color32::from_rgba_unmultiplied(
                    glow_color.r(),
                    glow_color.g(),
                    glow_color.b(),
                    (glow_a * 120.0) as u8,
                ),
            );
            icon_painter.circle_filled(
                req_rect.center(),
                glow_r,
                egui::Color32::from_rgba_unmultiplied(
                    glow_color.r(),
                    glow_color.g(),
                    glow_color.b(),
                    (glow_a * 255.0) as u8,
                ),
            );
        }
    }

    let tint = egui::Color32::WHITE.linear_multiply(anim * flash_alpha);
    if !sow_ui_kit::widgets::try_paint_emoji(&icon_painter, emoji, req_rect, tint) {
        icon_painter.text(
            req_rect.center(),
            egui::Align2::CENTER_CENTER,
            emoji,
            egui::FontId::proportional(size * 0.7),
            tint,
        );
    }

    size + 4.0
}

/// Draws floating animated active express emoji above the nameplate.
pub(crate) fn draw_express_emoji(
    painter: &egui::Painter,
    center: egui::Pos2,
    player_id: u16,
    active_emoji: Option<&String>,
    font_size: f32,
    content_h: f32,
    max_float_offset: f32,
) {
    let last_emoji_id = egui::Id::new(("last_active_emoji", player_id));
    let mut current_emoji = active_emoji.cloned();
    let is_active = current_emoji.is_some() && current_emoji.as_deref() != Some("🗡️");

    let active_anim_id = egui::Id::new(("emoji_anim_progress", player_id));
    let anim_progress = painter
        .ctx()
        .animate_bool_with_time(active_anim_id, is_active, 0.25);

    if anim_progress <= 0.01 {
        return;
    }

    if current_emoji.is_none() || current_emoji.as_deref() == Some("🗡️") {
        current_emoji = painter.ctx().data(|d| d.get_temp::<String>(last_emoji_id));
    } else {
        painter
            .ctx()
            .data_mut(|d| d.insert_temp(last_emoji_id, current_emoji.clone().unwrap()));
    }

    if let Some(emoji_str) = &current_emoji {
        let anim_scale = if is_active {
            if anim_progress >= 1.0 {
                1.0
            } else {
                spring_overshoot(anim_progress)
            }
        } else {
            anim_progress
        };

        // Quantize final emoji size to integer points to prevent texture/glyph atlas pollution
        let base_emoji_size = font_size * 3.2;
        let final_emoji_size = (base_emoji_size * anim_scale).round();

        let mut base_y_offset = content_h / 2.0 + (font_size * 0.30).round();
        if max_float_offset > 0.01 {
            base_y_offset += max_float_offset;
        }
        let emoji_y = (center.y - base_y_offset - final_emoji_size / 2.0).round();

        if final_emoji_size > 1.0 {
            let emoji_painter = painter.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new(("floating_express_emoji", player_id)),
            ));
            let emoji_rect = egui::Rect::from_center_size(
                egui::pos2(center.x, emoji_y),
                egui::vec2(final_emoji_size, final_emoji_size),
            );
            if !sow_ui_kit::widgets::try_paint_emoji(
                &emoji_painter,
                emoji_str,
                emoji_rect,
                egui::Color32::WHITE,
            ) {
                let emoji_galley = emoji_painter.layout_no_wrap(
                    emoji_str.clone(),
                    egui::FontId::proportional(final_emoji_size),
                    egui::Color32::WHITE,
                );
                let emoji_pos = egui::pos2(
                    center.x - emoji_galley.size().x / 2.0,
                    emoji_y - emoji_galley.size().y / 2.0,
                );
                emoji_painter.galley(emoji_pos, emoji_galley, egui::Color32::WHITE);
            }
        }
    }
}
