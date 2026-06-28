use super::super::*;
use super::emoji::*;

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
    let zoom_scaled = ctx.zoom_scaled;
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
    let far_zoom_threshold = visual_config.far_zoom_lod_threshold;
    let ui_text_scale = visual_config.ui_text_scale;
    let zoom_scaled_local = input.camera_zoom / sf;

    // Frame-constant trig — computed once, reused by every player
    let heart_flash_alpha = ((wall_secs * 12.0).cos() * 0.5 + 0.5) as f32;

    // Hoist my_player lookup — avoids O(n) scan per player
    let my_id = sim.my_player_id.unwrap_or(0);
    let my_player = snap.players.iter().find(|p| p.id == my_id);

    let show_bot_avatars = crate::app::vfx_on(painter.ctx(), |f| f.bot_avatars);
    let show_names = crate::app::vfx_on(painter.ctx(), |f| f.nameplate_names);
    let show_troops = crate::app::vfx_on(painter.ctx(), |f| f.nameplate_troops);

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
        if ui.tutorial_active && is_me {
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

        let map_area = (sim.map_w * sim.map_h).max(1) as f32;
        let normalized_tiles = player.tile_count as f32 * (40_000.0 / map_area);
        let is_massive_on_screen =
            normalized_tiles * zoom_scaled_local * zoom_scaled_local >= 1500.0;

        if zoom_scaled < far_zoom_threshold && !is_massive_on_screen {
            if is_human {
                // Hide other human nameplates on second LOD (far zoom) completely
                if !is_me {
                    continue;
                }
            } else {
                // Simplified dot representation for bots/nations — GPU disc + ring.
                if let Some(tr) = gfx.text_renderer.as_mut() {
                    let c = [center.x * sf, center.y * sf];
                    let r = dot_r * 0.8 * sf;
                    tr.push_disc(c, r, pc.to_array().map(|v| v as f32 / 255.0));
                    tr.push_ring(c, r, [0.0, 0.0, 0.0, 180.0 / 255.0], 1.0 * sf);
                }
                continue;
            }
        }

        let show_full = if is_human {
            true
        } else if player.player_type == sow_core::player::PlayerType::Bot {
            (normalized_tiles * zoom_scaled_local) >= 8.0 && full_labels_drawn < 80
        } else {
            (normalized_tiles * zoom_scaled_local) >= 2.0
        };

        if show_full {
            if player.player_type == sow_core::player::PlayerType::Bot {
                full_labels_drawn += 1;
            }

            let base_config_size = if is_me {
                visual_config.nameplate_my_size
            } else if is_human {
                visual_config.nameplate_premium_size
            } else if player.player_type == sow_core::player::PlayerType::Bot {
                visual_config.nameplate_tribe_size
            } else {
                visual_config.nameplate_nation_size
            };
            // Only local player and humans scale with territory; bots/nations stay constant screen size.
            const NAMEPLATE_RENDER_SCALE: f32 = 0.4;
            let base_premium_size = if (is_me || is_human) && vp.nameplate_size > 0.1 {
                vp.nameplate_size * NAMEPLATE_RENDER_SCALE
            } else {
                base_config_size
            };
            let raw_scaled =
                (base_premium_size * ui_text_scale).min(visual_config.nameplate_max_screen_font);
            let scaled_size = if is_me {
                raw_scaled.max(visual_config.nameplate_my_size)
            } else if is_human {
                raw_scaled.max(visual_config.nameplate_premium_size)
            } else {
                raw_scaled
            };
            if scaled_size < 7.0 && !is_me && !is_human {
                if let Some(tr) = gfx.text_renderer.as_mut() {
                    let c = [center.x * sf, center.y * sf];
                    let r = dot_r * sf;
                    tr.push_disc(c, r, pc.to_array().map(|v| v as f32 / 255.0));
                    tr.push_ring(c, r, [0.0, 0.0, 0.0, 180.0 / 255.0], 1.0 * sf);
                }
                continue;
            }

            // QUANTIZATION: Round the font sizes to nearest whole numbers to prevent glyph atlas invalidations!
            let font_size = scaled_size.round().max(7.0);
            let is_bot = player.player_type == sow_core::player::PlayerType::Bot;
            let mut avatar_size = (font_size * 2.2).round().max(4.0);
            if is_bot && !show_bot_avatars {
                avatar_size = 0.0;
            }
            let star_size = (avatar_size * 0.72).round().max(4.0);

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

            // Build status flags for the premium static emojis
            let is_disconnected = player.disconnected;
            let mut betrayal_flash = false;

            if !is_disconnected {
                let has_betrayal = player.traitor;
                if has_betrayal {
                    betrayal_flash = true;
                }
            }

            let rgb = player.color;
            let vibrant_color = crate::hud::nameplate::ensure_readable_nameplate_color(rgb);

            let mut disc_galley = None;
            let mut has_disc_emoji = false;
            let mut disc_rect_size = 0.0;
            if is_disconnected {
                // QUANTIZATION: Round the disconnected size
                let disc_size = (font_size * 0.95 * 3.0).round().max(2.0);
                disc_rect_size = disc_size;

                let test_rect = egui::Rect::from_min_size(
                    egui::pos2(0.0, 0.0),
                    egui::vec2(disc_size, disc_size),
                );
                if sow_ui_kit::widgets::try_paint_emoji(
                    painter,
                    "🔌",
                    test_rect,
                    egui::Color32::WHITE,
                ) {
                    has_disc_emoji = true;
                } else {
                    let disc_font_id = egui::FontId::proportional(disc_size);
                    let mut job = egui::text::LayoutJob {
                        break_on_newline: false,
                        ..Default::default()
                    };
                    job.append(
                        "🔌",
                        0.0,
                        egui::text::TextFormat::simple(
                            disc_font_id,
                            egui::Color32::from_rgb(239, 68, 68),
                        ),
                    );
                    disc_galley = Some(painter.layout_job(job));
                }
            }

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

            // QUANTIZATION: Round the troops font size
            let troops_font_size = (font_size * 1.30).round().max(2.0);
            let troops_font_id = egui::FontId::proportional(troops_font_size);

            let prepared_name = sow_ui_kit::widgets::prepare_name(painter, &display_name, &font_id);
            let name_size = prepared_name.size;
            let troops_galley = painter.layout_no_wrap(
                troops_str.clone(),
                troops_font_id.clone(),
                vibrant_color,
            );

            let name_w = if show_names { name_size.x } else { 0.0 };
            let name_h = if show_names { name_size.y } else { 0.0 };

            let troops_w = if show_troops {
                crate::hud::nameplate::troops_row_width(&troops_galley, &troops_font_id)
            } else {
                0.0
            };
            let troops_h = if show_troops {
                troops_galley.rect.height()
            } else {
                0.0
            };

            let right_w = name_w.max(troops_w);
            let item_spacing_y = if show_names && show_troops { (font_size * 0.111).round() } else { 0.0 };
            let right_h = name_h + item_spacing_y + troops_h;

            let spacing_x = if avatar_size > 0.0 { (font_size * 0.333).round() } else { 0.0 };
            let mut total_w = avatar_size + spacing_x + right_w;
            if is_me {
                total_w += star_size + spacing_x;
            }
            let total_h = avatar_size.max(right_h);

            let mut row0_h = 0.0;
            if has_disc_emoji {
                row0_h = disc_rect_size + (font_size * 0.222).round();
            } else if let Some(ref dg) = disc_galley {
                row0_h = dg.rect.height() + (font_size * 0.222).round();
            }
            let content_h = row0_h + total_h;

            let req_offset = draw_floating_status_emoji(
                painter,
                center,
                player.id,
                is_me,
                font_size,
                content_h,
                has_req,
                "request_anim_progress",
                "📨",
                "floating_request_icon",
                Some(egui::Color32::from_rgb(34, 211, 238)),
                1.0,
            );

            let flash_alpha = if is_heart_flashing {
                heart_flash_alpha
            } else {
                1.0
            };
            let allied_offset = draw_floating_status_emoji(
                painter,
                center,
                player.id,
                is_me,
                font_size,
                content_h,
                is_allied,
                "allied_anim_progress",
                "🤝",
                "floating_handshake_icon",
                Some(egui::Color32::from_rgb(255, 200, 60)),
                flash_alpha,
            );

            let betrayal_offset = draw_floating_status_emoji(
                painter,
                center,
                player.id,
                is_me,
                font_size,
                content_h,
                betrayal_flash,
                "betrayal_anim_progress",
                "🗡️",
                "floating_betray_icon",
                Some(egui::Color32::from_rgb(220, 38, 38)),
                1.0,
            );

            let max_float_offset = req_offset.max(allied_offset).max(betrayal_offset);

            // Render express emoji
            draw_express_emoji(
                painter,
                center,
                player.id,
                player.active_emoji.as_ref(),
                font_size,
                content_h,
                max_float_offset,
            );

            let content_min = egui::pos2(center.x - total_w / 2.0, center.y - content_h / 2.0);

            // Row 0 Status indicators
            if has_disc_emoji {
                let row0_rect = egui::Rect::from_center_size(
                    egui::pos2(center.x, content_min.y + disc_rect_size / 2.0),
                    egui::vec2(disc_rect_size, disc_rect_size),
                );
                let disc_gpu = gfx.text_renderer.as_mut().map_or(false, |tr| {
                    tr.push_emoji(
                        "🔌",
                        [row0_rect.center().x * sf, row0_rect.center().y * sf],
                        row0_rect.height() * 0.5 * sf,
                        [1.0, 1.0, 1.0, 1.0],
                        [0.0, 0.0, 0.0, 1.0],
                        1.0 * sf,
                        1.5 * sf,
                    )
                });
                if !disc_gpu {
                    sow_ui_kit::widgets::try_paint_emoji(
                        painter,
                        "🔌",
                        row0_rect,
                        egui::Color32::WHITE,
                    );
                }
            } else if let Some(dg) = disc_galley {
                let row0_pos = egui::pos2(center.x - dg.rect.width() / 2.0, content_min.y);
                painter.galley(row0_pos, dg, egui::Color32::WHITE);
            }

            // Row 1 & 2 layout
            let row12_y = content_min.y + row0_h;
            let mut cur_x = content_min.x;

            // 0. Star (if me)
            if is_me {
                let star_rect = egui::Rect::from_center_size(
                    egui::pos2(cur_x + star_size / 2.0, row12_y + total_h / 2.0),
                    egui::vec2(star_size, star_size),
                );
                let star_gpu = gfx.text_renderer.as_mut().map_or(false, |tr| {
                    tr.push_emoji(
                        "⭐",
                        [star_rect.center().x * sf, star_rect.center().y * sf],
                        star_rect.height() * 0.5 * sf,
                        [1.0, 1.0, 1.0, 1.0],
                        [0.0, 0.0, 0.0, 1.0],
                        1.0 * sf,
                        1.5 * sf,
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
                        egui::FontId::proportional(star_size * 0.7),
                        egui::Color32::WHITE,
                    );
                }
                cur_x += star_size + spacing_x;
            }

            // 1. Circular avatar with decorative frame — GPU pipeline (disc/portrait + rings + emoji).
            if avatar_size > 0.0 {
                let avatar_cx = cur_x + avatar_size / 2.0;
                let avatar_cy = row12_y + total_h / 2.0;
                let avatar_r = avatar_size / 2.0;
                if let Some(tr) = gfx.text_renderer.as_mut() {
                    crate::hud::avatar::draw_player_avatar_gpu(
                        tr,
                        [avatar_cx * sf, avatar_cy * sf],
                        avatar_r * sf,
                        player.id,
                        &player.name,
                        player.player_type,
                        player.color,
                        player.leader,
                    );
                }
                cur_x += avatar_size + spacing_x;
            }

            // 2. Nickname and Troops centered in right block
            let right_y = row12_y + (total_h - right_h) / 2.0;

            let name_h = name_size.y;
            let troops_h = troops_galley.rect.height();

            let mut gpu_text_rendered = false;
            if let Some(ref mut tr) = gfx.text_renderer {
                gpu_text_rendered = true;
                let color_arr = vibrant_color.to_array().map(|v| v as f32 / 255.0);
                let outline_color_arr = [0.0f32, 0.0, 0.0, 1.0];

                let ctx_ref = painter.ctx();
                let face_dilate = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_face_dilate")).unwrap_or(-0.6f32)) * sf;
                let outline_thickness = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_outline_thickness")).unwrap_or(1.0f32)) * sf;
                let shadow_y = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_shadow_y")).unwrap_or(1.5f32)) * sf;
                let underlay_softness = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_underlay_softness")).unwrap_or(0.0f32)) * sf;
                let char_spacing = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_char_spacing")).unwrap_or(0.95f32));
                let font_size_scale = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_size_scale")).unwrap_or(1.67f32));
                let name_offset_x = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_offset_x")).unwrap_or(16.0f32));
                let emoji_scale = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_emoji_size_scale")).unwrap_or(1.4f32));

                let settings = crate::render::gpu::TmpFontSettings {
                    face_dilate,
                    outline_thickness,
                    underlay_offset_y: shadow_y,
                    underlay_softness,
                };

                if show_names {
                    let name_center_x = (cur_x + right_w / 2.0 - 1.0 + name_offset_x) * sf;
                    let name_baseline_y = (right_y + name_h * 0.85) * sf;
                    tr.push_string(
                        &display_name,
                        [name_center_x, name_baseline_y],
                        font_size * font_size_scale * sf,
                        color_arr,
                        outline_color_arr,
                        settings,
                        0.5,
                        char_spacing,
                        emoji_scale,
                    );
                }

                if show_troops {
                    let troops_row_y = if show_names { right_y + name_h + item_spacing_y } else { right_y };
                    let icon_size = troops_font_size * 1.15;
                    let icon_half = icon_size * 0.5;
                    let troops_left_x = cur_x + (right_w - troops_w) / 2.0;
                    // Sword emoji
                    tr.push_emoji(
                        "⚔",
                        [(troops_left_x + icon_half) * sf, (troops_row_y + icon_half) * sf],
                        icon_half * sf,
                        color_arr,
                        outline_color_arr,
                        outline_thickness,
                        shadow_y,
                    );
                    // Number text left-aligned after sword
                    let text_left = (troops_left_x + icon_size + 3.0) * sf;
                    let troops_baseline_y = (troops_row_y + troops_h * 0.85) * sf;
                    let display_troops_str = &troops_str;
                    tr.push_string(
                        display_troops_str,
                        [text_left, troops_baseline_y],
                        troops_font_size * font_size_scale * sf,
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
                    let name_x = cur_x + (right_w - name_w) / 2.0;
                    let name_text_h = name_size.y;
                    sow_ui_kit::widgets::paint_prepared_name_with_glow(
                        painter,
                        egui::pos2(name_x, right_y),
                        egui::Align2::LEFT_TOP,
                        &prepared_name,
                        vibrant_color,
                        sow_ui_kit::theme::NAMEPLATE,
                        Some(name_text_h),
                    );
                }

                if show_troops {
                    let troops_row_y = if show_names { right_y + name_size.y + item_spacing_y } else { right_y };
                    let troops_x = cur_x + (right_w - troops_w) / 2.0;
                    crate::hud::nameplate::paint_glow_troops_row(
                        painter,
                        egui::pos2(troops_x, troops_row_y),
                        troops_galley.clone(),
                        &troops_font_id,
                        vibrant_color,
                        Some(name_size.y),
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

#[inline]
pub(super) fn seed_hash(seed: u32, idx: u32) -> f32 {
    let mut x = seed.wrapping_add(idx).wrapping_mul(0x9E3779B9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x85EBCA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2AE35);
    x ^= x >> 16;
    (x as f32) / (u32::MAX as f32)
}
