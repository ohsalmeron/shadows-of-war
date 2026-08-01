pub(crate) struct SideBadgeOpts<'a> {
    pub pos: egui::Pos2,
    pub size: f32,
    pub player_id: u16,
    pub is_me: bool,
    pub active: bool,
    pub anim_id_str: &'static str,
    pub emoji: &'a str,
    pub color_glow: Option<egui::Color32>,
    pub flash_alpha: f32,
}

/// Status badge drawn at an absolute screen position (beside avatar). Spring entrance + glow.
pub(crate) fn draw_side_status_badge(
    painter: &egui::Painter,
    opts: &SideBadgeOpts,
) {
    let pos = opts.pos;
    let size = opts.size;
    let player_id = opts.player_id;
    let is_me = opts.is_me;
    let active = opts.active;
    let anim_id_str = opts.anim_id_str;
    let emoji = opts.emoji;
    let color_glow = opts.color_glow;
    let flash_alpha = opts.flash_alpha;
    let anim_id = egui::Id::new((anim_id_str, player_id));
    if !sow_ui_kit::theme::dev_config::DevConfig::get().vfx_status_emojis {
        return;
    }
    let anim = painter.ctx().animate_bool_with_time(anim_id, active, 0.25);
    if anim <= 0.01 {
        return;
    }
    let anim_scale = if active {
        if anim >= 1.0 {
            1.0
        } else {
            sow_ui::ui::animation::spring_overshoot(anim)
        }
    } else {
        anim
    };
    let final_size = (size * anim_scale).round();
    if final_size <= 1.0 {
        return;
    }
    let rect = egui::Rect::from_center_size(pos, egui::vec2(final_size, final_size));

    if is_me {
        if let Some(glow_color) = color_glow {
            let glow_r = final_size * 0.8;
            let glow_a = anim * flash_alpha * 0.35;
            painter.circle_filled(
                rect.center(),
                glow_r * 1.4,
                egui::Color32::from_rgba_unmultiplied(
                    glow_color.r(),
                    glow_color.g(),
                    glow_color.b(),
                    (glow_a * 120.0) as u8,
                ),
            );
            painter.circle_filled(
                rect.center(),
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
    let painted = if let Some(uv) = sow_ui_kit::atlas_uv(emoji) {
        if let Some(texture) = sow_ui_kit::atlas_texture(painter.ctx()) {
            painter.image(
                texture.id(),
                rect,
                uv,
                egui::Color32::from_white_alpha(tint.a()),
            );
            true
        } else {
            false
        }
    } else {
        false
    };
    if !painted {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            emoji,
            egui::FontId::proportional(final_size * 0.7),
            tint,
        );
    }
}

/// Express emoji drawn at an absolute screen position (beside avatar). Spring entrance + hold.
pub(crate) fn draw_side_express_emoji(
    painter: &egui::Painter,
    pos: egui::Pos2,
    size: f32,
    player_id: u16,
    active_emoji: Option<&String>,
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
                sow_ui::ui::animation::spring_overshoot(anim_progress)
            }
        } else {
            anim_progress
        };
        let final_size = (size * anim_scale).round();
        if final_size > 1.0 {
            let rect = egui::Rect::from_center_size(pos, egui::vec2(final_size, final_size));
            let painted = if let Some(uv) = sow_ui_kit::atlas_uv(emoji_str) {
                if let Some(texture) = sow_ui_kit::atlas_texture(painter.ctx()) {
                    painter.image(texture.id(), rect, uv, egui::Color32::WHITE);
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if !painted {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    emoji_str,
                    egui::FontId::proportional(final_size),
                    egui::Color32::WHITE,
                );
            }
        }
    }
}
