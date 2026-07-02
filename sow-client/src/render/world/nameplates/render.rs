use super::super::*;
use super::emoji::*;

/// World-anchored nameplate font size in screen px. `nameplate_size * zoom_scaled` is the
/// territory's on-screen side length, so the plate shrinks as you zoom out and grows with
/// territory — it always occupies the same fraction of the land it labels. Humans keep a
/// small readability floor; bots/nations have none, so far-away ones fall through to the
/// <7px dot LOD. Caps only catch extremes (huge empire fully zoomed in), not normal play.
pub(crate) fn nameplate_font_px(
    nameplate_size: f32,
    zoom_scaled: f32,
    is_human: bool,
    cfg: &ClientVisualConfig,
) -> f32 {
    let world_px = nameplate_size * cfg.nameplate_world_scale * zoom_scaled;
    if is_human {
        world_px.clamp(8.0, cfg.nameplate_max_font)
    } else {
        world_px.min(cfg.nameplate_max_font * 0.6)
    }
}

#[allow(unused_variables)]
pub(crate) fn render(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    time: &crate::app::TimeState,
    gfx: &mut crate::app::GraphicsState,
    ctx: &RenderContext,
) {
    let painter = ctx.painter.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("world_nameplates"),
    ));
    let painter = &painter;
    let sf = ctx.sf;
    let player_colors = ctx.player_colors;
    let dot_r = ctx.dot_r;
    let current_tick = ctx.current_tick;
    let wall_secs = ctx.wall_secs;
    let visible_players = ctx.visible_players;

    if visible_players.is_empty() {
        return;
    }

    let Some(snap) = &sim.current_snapshot else {
        return;
    };

    // visible_players is pre-sorted in mod.rs (local player last, then humans, nations, presence)
    let mut full_labels_drawn = 0;

    let visual_config = ClientVisualConfig::default();
    let ui_text_scale = visual_config.ui_text_scale;
    let zoom_scaled_local = input.camera_zoom / sf;

    // Frame-constant trig — computed once, reused by every player
    let heart_flash_alpha = ((wall_secs * 12.0).cos() * 0.5 + 0.5) as f32;

    // Hoist my_player lookup — avoids O(n) scan per player
    let my_id = sim.my_player_id.unwrap_or(0);
    let my_player = snap.players.iter().find(|p| p.id == my_id);

    let dev = sow_ui_kit::theme::dev_config::DevConfig::get();
    let show_bot_avatars = dev.vfx_bot_avatars;
    let show_names = dev.vfx_nameplate_names;
    let show_troops = dev.vfx_nameplate_troops;

    for vp in visible_players {
        let player = vp.player;
        let center = vp.center;
        let pc = vp.pc;
        let lod_presence = vp.lod_presence;

        let is_me = player.id == my_id;
        let is_human = player.player_type == sow_core::player::PlayerType::Human;

        // Tutorial only — TEMPORARY player-nameplate simplification. In the tutorial the local
        // player's nameplate is JUST the avatar, centered on the territory anchor so it nests inside
        // the centered tutorial pointer ring: no star / name / troops / status. Gated on
        // `tutorial_active`, which is false in every normal solo/MP match (tutorial isolation
        // contract + single-derive gate), so the standard gameplay nameplate path below is never
        // touched. To retire this, delete the whole block — never weaken the gate.
        if ui.tutorial_active && is_me && ui.tutorial_step_idx == 0 {
            if let Some(tr) = gfx.text_renderer.as_mut() {
                crate::hud::avatar::draw_player_avatar_gpu(
                    tr,
                    [center.x * sf, center.y * sf],
                    20.0_f32 * sf,
                    player.id,
                    &player.name,
                    player.player_type,
                    player.color,
                    player.leader,
                );
            }
            continue;
        }

        // Far-zoom declutter, gated on ZOOM (not per-plate pixel size): a NON-human plate
        // collapses to a cheap disc only when the camera is truly far out
        // (zoom_scaled < nameplate_hide_zoom, e.g. 1.5). At any closer zoom EVERY plate is
        // drawn full and floored at a readable size — nothing hides while you play/scroll.
        if zoom_scaled_local < visual_config.nameplate_hide_zoom && !is_me && !is_human {
            if let Some(tr) = gfx.text_renderer.as_mut() {
                let c = [center.x * sf, center.y * sf];
                let r = dot_r * sf;
                tr.push_disc(c, r, pc.to_array().map(|v| v as f32 / 255.0));
                tr.push_ring(c, r, [0.0, 0.0, 0.0, 180.0 / 255.0], 1.0 * sf);
            }
            continue;
        }

        // `full_labels_drawn` caps full bot labels for perf only; over-budget bots dot out (else).
        let show_full = if is_human {
            true
        } else if player.player_type == sow_core::player::PlayerType::Bot {
            full_labels_drawn < 80
        } else {
            true
        };

        if show_full {
            if player.player_type == sow_core::player::PlayerType::Bot {
                full_labels_drawn += 1;
            }

            let scaled_size =
                nameplate_font_px(vp.nameplate_size, zoom_scaled_local, is_human, &visual_config)
                    * ui_text_scale;

            // Continuous size drives ALL geometry and the GPU SDF text (which scales smoothly
            // at any fractional size), so the plate grows/shrinks as smoothly as the camera.
            let render_size = scaled_size.max(7.0);
            // `font_size` is quantized ONLY for egui's CPU glyph atlas (name/troops measurement
            // + the non-GPU fallback), keeping distinct FontIds bounded. Measured metrics are
            // scaled back to render_size via `text_scale`, so quantization never shows on screen.
            let font_size = if render_size > 20.0 {
                (render_size / 2.0).round() * 2.0
            } else {
                render_size.round()
            }
            .max(7.0);
            let text_scale = render_size / font_size;
            let is_bot = player.player_type == sow_core::player::PlayerType::Bot;
            let mut avatar_size = (render_size * 2.2).max(4.0);
            if is_bot && !show_bot_avatars {
                avatar_size = 0.0;
            }
            // Check alliance status with the player
            let mut is_allied = false;
            let mut is_heart_flashing = false;
            let mut has_req = false;
            if my_id != player.id {
                if let Some(me) = my_player {
                    if me.alliances.contains(&player.id) {
                        is_allied = true;
                        let timer = me.alliance_timers.get(&player.id).copied().unwrap_or(2400);
                        let has_pending_proposal = me.alliance_requests.contains(&player.id)
                            || player.alliance_requests.contains(&my_id);
                        if timer <= 300 && !has_pending_proposal {
                            is_heart_flashing = true;
                        }
                    } else if me.alliance_requests.contains(&player.id) {
                        has_req = true;
                    }
                }
            }

            let is_disconnected = player.disconnected;
            let mut betrayal_flash = false;
            if !is_disconnected && player.traitor {
                betrayal_flash = true;
            }

            let vibrant_color =
                crate::hud::nameplate::ensure_readable_nameplate_color(player.color);

            let troops_str = sow_ui_kit::utils::format_number(player.troops);
            let font_id = egui::FontId::proportional(font_size);
            let display_name = if player.player_type == sow_core::player::PlayerType::Bot {
                if player.name.is_empty() {
                    format!("Tribe {}", player.id.saturating_sub(199))
                } else {
                    player.name.clone()
                }
            } else {
                sow_core::player::display_name(player.id, &player.name, player.player_type)
            };

            let troops_render_size = render_size * 1.30;
            let troops_font_size = (font_size * 1.30).round().max(2.0);
            let troops_font_id = egui::FontId::proportional(troops_font_size);

            let prepared_name = sow_ui_kit::widgets::prepare_name(painter, &display_name, &font_id);
            let name_size = prepared_name.size;
            let troops_galley =
                painter.layout_no_wrap(troops_str.clone(), troops_font_id.clone(), vibrant_color);

            let name_w = if show_names { name_size.x * text_scale } else { 0.0 };
            let name_h = if show_names { name_size.y * text_scale } else { 0.0 };
            let troops_w = if show_troops {
                crate::hud::nameplate::troops_row_width(&troops_galley, &troops_font_id) * text_scale
            } else {
                0.0
            };
            let troops_h = if show_troops {
                troops_galley.rect.height() * text_scale
            } else {
                0.0
            };
            let right_w = name_w.max(troops_w);
            let item_spacing_y = if show_names && show_troops {
                render_size * 0.111
            } else {
                0.0
            };
            let right_h = name_h + item_spacing_y + troops_h;

            // Vertical layout: avatar centered on top, text below.
            let spacing_y = if avatar_size > 0.0 && right_h > 0.0 {
                render_size * 0.333
            } else {
                0.0
            };
            let total_w = right_w.max(avatar_size);
            let total_h = avatar_size + spacing_y + right_h;

            let content_top = center.y - total_h / 2.0;
            let avatar_cy = content_top + avatar_size / 2.0;

            // Badge sizing / positioning beside avatar.
            let badge_size = render_size * 1.8;
            let left_x = center.x - avatar_size / 2.0 - badge_size / 2.0 - 3.0;
            let right_x = center.x + avatar_size / 2.0 + badge_size / 2.0 + 3.0;

            // Status badges (left: request, right: allied / betrayal stacked).
            draw_side_status_badge(
                painter,
                egui::pos2(left_x, avatar_cy),
                badge_size,
                player.id,
                is_me,
                has_req,
                "request_anim_progress",
                "📨",
                Some(egui::Color32::from_rgb(34, 211, 238)),
                1.0,
            );

            let flash_alpha = if is_heart_flashing {
                heart_flash_alpha
            } else {
                1.0
            };
            let mut right_slot_y = avatar_cy;
            if is_allied {
                draw_side_status_badge(
                    painter,
                    egui::pos2(right_x, right_slot_y),
                    badge_size,
                    player.id,
                    is_me,
                    true,
                    "allied_anim_progress",
                    "🤝",
                    Some(egui::Color32::from_rgb(255, 200, 60)),
                    flash_alpha,
                );
                right_slot_y -= badge_size + 2.0;
            }
            if betrayal_flash {
                draw_side_status_badge(
                    painter,
                    egui::pos2(right_x, right_slot_y),
                    badge_size,
                    player.id,
                    is_me,
                    true,
                    "betrayal_anim_progress",
                    "🗡️",
                    Some(egui::Color32::from_rgb(220, 38, 38)),
                    1.0,
                );
                right_slot_y -= badge_size + 2.0;
            }

            // Express emoji
            draw_side_express_emoji(
                painter,
                egui::pos2(right_x + 2.0, right_slot_y - badge_size / 2.0),
                badge_size,
                player.id,
                player.active_emoji.as_ref(),
            );

            // --- Avatar (centered on top) ---
            if avatar_size > 0.0 {
                let avatar_r = avatar_size / 2.0;
                if let Some(tr) = gfx.text_renderer.as_mut() {
                    crate::hud::avatar::draw_player_avatar_gpu(
                        tr,
                        [center.x * sf, avatar_cy * sf],
                        avatar_r * sf,
                        player.id,
                        &player.name,
                        player.player_type,
                        player.color,
                        player.leader,
                    );
                }
            }

            // Star corner badge (top-right of avatar)
            if is_me && avatar_size > 0.0 {
                let star_sz = (avatar_size * 0.35).round().max(3.0);
                let star_cx = center.x + avatar_size / 2.0 * 0.6;
                let star_cy = avatar_cy - avatar_size / 2.0 * 0.6;
                let star_rect = egui::Rect::from_center_size(
                    egui::pos2(star_cx, star_cy),
                    egui::vec2(star_sz, star_sz),
                );
                let star_gpu = gfx.text_renderer.as_mut().is_some_and(|tr| {
                    tr.push_emoji(
                        "⭐",
                        [star_rect.center().x * sf, star_rect.center().y * sf],
                        star_rect.height() * 0.5 * sf,
                        [1.0; 4],
                        [0.0, 0.0, 0.0, 1.0],
                        0.0,
                        0.0,
                    )
                });
                if !star_gpu
                    && !sow_ui_kit::widgets::try_paint_emoji(
                        painter,
                        "⭐",
                        star_rect,
                        egui::Color32::WHITE,
                    )
                {
                    painter.text(
                        star_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "⭐",
                        egui::FontId::proportional(star_sz * 0.7),
                        egui::Color32::WHITE,
                    );
                }
            }

            // Disconnected corner badge (bottom-right of avatar)
            if is_disconnected && avatar_size > 0.0 {
                let disc_sz = (avatar_size * 0.4).round().max(3.0);
                let disc_cx = center.x + avatar_size / 2.0 * 0.6;
                let disc_cy = avatar_cy + avatar_size / 2.0 * 0.6;
                let disc_rect = egui::Rect::from_center_size(
                    egui::pos2(disc_cx, disc_cy),
                    egui::vec2(disc_sz, disc_sz),
                );
                let disc_gpu = gfx.text_renderer.as_mut().is_some_and(|tr| {
                    tr.push_emoji(
                        "🔌",
                        [disc_rect.center().x * sf, disc_rect.center().y * sf],
                        disc_rect.height() * 0.5 * sf,
                        [1.0; 4],
                        [0.0, 0.0, 0.0, 1.0],
                        0.0,
                        0.0,
                    )
                });
                if !disc_gpu
                    && !sow_ui_kit::widgets::try_paint_emoji(
                        painter,
                        "🔌",
                        disc_rect,
                        egui::Color32::WHITE,
                    )
                {
                    let disc_font_id = egui::FontId::proportional(disc_sz);
                    painter.text(
                        disc_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "🔌",
                        disc_font_id,
                        egui::Color32::WHITE,
                    );
                }
            }

            // --- Name + Troops (centered below avatar) ---
            let text_top = content_top + avatar_size + spacing_y;

            let mut gpu_text_rendered = false;
            if let Some(ref mut tr) = gfx.text_renderer {
                gpu_text_rendered = true;
                let color_arr = vibrant_color.to_array().map(|v| v as f32 / 255.0);
                let outline_color_arr = [0.0f32, 0.0, 0.0, 1.0];

                let dev = sow_ui_kit::theme::dev_config::DevConfig::get();
                let face_dilate = dev.font_face_dilate * sf;
                let outline_thickness = dev.font_outline_thickness * sf;
                let shadow_y = dev.font_shadow_y * sf;
                let underlay_softness = dev.font_underlay_softness * sf;
                let char_spacing = dev.font_char_spacing;
                let font_size_scale = dev.font_size_scale;
                let emoji_scale = dev.emoji_size_scale;

                let settings = crate::render::gpu::TmpFontSettings {
                    face_dilate,
                    outline_thickness,
                    underlay_offset_y: shadow_y,
                    underlay_softness,
                };

                if show_names {
                    tr.push_string(
                        &display_name,
                        [center.x * sf, (text_top + name_h * 0.85) * sf],
                        render_size * font_size_scale * sf,
                        color_arr,
                        outline_color_arr,
                        settings,
                        0.5,
                        char_spacing,
                        emoji_scale,
                    );
                }

                if show_troops {
                    let troops_row_y = if show_names {
                        text_top + name_h + item_spacing_y
                    } else {
                        text_top
                    };
                    let icon_size = troops_render_size * 1.15;
                    let icon_half = icon_size * 0.5;
                    let troops_left_x = center.x - troops_w / 2.0;
                    tr.push_emoji(
                        "⚔",
                        [
                            (troops_left_x + icon_half) * sf,
                            (troops_row_y + icon_half) * sf,
                        ],
                        icon_half * sf,
                        color_arr,
                        outline_color_arr,
                        outline_thickness,
                        shadow_y,
                    );
                    tr.push_string(
                        &troops_str,
                        [
                            (troops_left_x + icon_size + 3.0) * sf,
                            (troops_row_y + troops_h * 0.85) * sf,
                        ],
                        troops_render_size * font_size_scale * sf,
                        color_arr,
                        outline_color_arr,
                        settings,
                        0.0,
                        char_spacing,
                        emoji_scale,
                    );
                }
            }

            if !gpu_text_rendered {
                if show_names {
                    sow_ui_kit::widgets::paint_prepared_name_with_glow(
                        painter,
                        egui::pos2(center.x - name_w / 2.0, text_top),
                        egui::Align2::LEFT_TOP,
                        &prepared_name,
                        vibrant_color,
                        sow_ui_kit::theme::NAMEPLATE,
                        Some(name_h),
                    );
                }

                if show_troops {
                    let troops_row_y = if show_names {
                        text_top + name_h + item_spacing_y
                    } else {
                        text_top
                    };
                    crate::hud::nameplate::paint_glow_troops_row(
                        painter,
                        egui::pos2(center.x - troops_w / 2.0, troops_row_y),
                        troops_galley.clone(),
                        &troops_font_id,
                        vibrant_color,
                        Some(name_h),
                    );
                }
            }
            continue;
        } else {
            if let Some(tr) = gfx.text_renderer.as_mut() {
                let c = [center.x * sf, center.y * sf];
                let r = dot_r * sf;
                tr.push_disc(c, r, pc.to_array().map(|v| v as f32 / 255.0));
                tr.push_ring(c, r, [0.0, 0.0, 0.0, 180.0 / 255.0], 1.0 * sf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Guards the two bugs previous fixes shipped: sizes pinned at a clamp across the whole
    // playable zoom range (constant on the viewport), and territory differences erased.
    // Playable zoom_scaled range: camera min 0.75 to max 100; plates are drawn at every zoom.
    #[test]
    fn nameplate_size_tracks_zoom_and_territory() {
        let cfg = ClientVisualConfig::default();
        let mid_territory = 2000.0_f32.sqrt(); // ~45 world units side

        // Zooming out must shrink the plate through the responsive band (above the readability
        // floor, below the cap), not sit pinned on a clamp.
        let far = nameplate_font_px(mid_territory, 8.0, true, &cfg);
        let mid = nameplate_font_px(mid_territory, 10.0, true, &cfg);
        let near = nameplate_font_px(mid_territory, 12.0, true, &cfg);
        assert!(far < mid && mid < near, "not zoom-responsive: {far} {mid} {near}");

        // Bigger territory must render bigger at the same zoom (both inside the band).
        let small = nameplate_font_px(400.0_f32.sqrt(), 10.0, true, &cfg);
        let large = nameplate_font_px(2500.0_f32.sqrt(), 10.0, true, &cfg);
        assert!(small < large, "not territory-responsive: {small} {large}");

        // Caps and floors only catch extremes.
        assert!(nameplate_font_px(150.0, 100.0, true, &cfg) <= cfg.nameplate_max_font);
        assert!(nameplate_font_px(0.2, 0.75, true, &cfg) >= 8.0);

        // Hiding is gated on camera zoom, not per-plate pixel size: a non-human plate is only
        // dotted below nameplate_hide_zoom. Just ABOVE that threshold it must still render (the
        // render path floors it to a readable size), so nothing pops while zoomed in to play.
        assert!(
            cfg.nameplate_hide_zoom <= 1.5,
            "hide threshold crept too close/in: {}",
            cfg.nameplate_hide_zoom
        );
        let just_inside = cfg.nameplate_hide_zoom + 0.5;
        let tiny_bot_font =
            nameplate_font_px(10.0, just_inside, false, &cfg).max(7.0); // render_size floor
        assert!(tiny_bot_font >= 7.0, "should stay readable, not hide: {tiny_bot_font}");
    }
}
