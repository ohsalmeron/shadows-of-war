use super::*;



/// Damped spring overshoot: approaches 1.0 with a single bounce.
#[inline]
pub(crate) fn spring_overshoot(t: f32) -> f32 {
    1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
}

use crate::hud::avatar::paint_circular_avatar;

/// Draws a floating emoji status icon (Request, Handshake, Betrayal) with spring entrance animation.
#[allow(clippy::too_many_arguments)]
fn draw_floating_status_emoji(
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
    if !sow_ui::widgets::try_paint_emoji(&icon_painter, emoji, req_rect, tint) {
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
fn draw_express_emoji(
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
            if !sow_ui::widgets::try_paint_emoji(
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

#[allow(unused_variables)]
pub(crate) fn render(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    time: &crate::app::TimeState,
    gfx: &crate::app::GraphicsState,
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

        for vp in visible_players {
            let player = vp.player;
            let center = vp.center;
            let pc = vp.pc;
            let lod_presence = vp.lod_presence;

            let is_me = player.id == my_id;
            let is_human = player.player_type == sow_core::player::PlayerType::Human;

            let map_area = (sim.map_w * sim.map_h).max(1) as f32;
            let normalized_tiles = player.tile_count as f32 * (40_000.0 / map_area);
            let is_massive_on_screen =
                normalized_tiles * zoom_scaled_local * zoom_scaled_local >= 1500.0;

            if zoom_scaled < far_zoom_threshold && !is_human && !is_massive_on_screen {
                painter.circle_filled(center, dot_r * 0.8, pc);
                painter.circle_stroke(
                    center,
                    dot_r * 0.8,
                    egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(180)),
                );
                continue;
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
                let raw_scaled = (base_premium_size * ui_text_scale)
                    .min(visual_config.nameplate_max_screen_font);
                let scaled_size = if is_me {
                    raw_scaled.max(visual_config.nameplate_my_size)
                } else if is_human {
                    raw_scaled.max(visual_config.nameplate_premium_size)
                } else {
                    raw_scaled
                };
                if scaled_size < 7.0 && !is_me {
                    painter.circle_filled(center, dot_r, pc);
                    painter.circle_stroke(
                        center,
                        dot_r,
                        egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(180)),
                    );
                    continue;
                }

                // QUANTIZATION: Round the font sizes to nearest whole numbers to prevent glyph atlas invalidations!
                let font_size = scaled_size.round().max(7.0);
                let avatar_size = (font_size * 2.2).round().max(4.0);

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
                if is_disconnected {
                    // QUANTIZATION: Round the disconnected font size
                    let disc_font_size = (font_size * 0.95 * 3.0).round().max(2.0);
                    let disc_font_id = egui::FontId::proportional(disc_font_size);
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

                let troops_str = sow_ui::utils::format_number(player.troops);
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

                let mut cached_prepared = None;
                let mut cached_troops = None;

                if let Some(entry) = ui.nameplate_galleys.get(&player.id) {
                    let now = web_time::Instant::now();
                    let rate_limited = ui
                        .nameplate_troops_last_update
                        .get(&player.id)
                        .copied()
                        .is_some_and(|last| now.duration_since(last).as_secs_f32() < 0.5);

                    if rate_limited {
                        cached_prepared = Some(entry.prepared_name.clone());
                        cached_troops = Some(entry.troops_galley.clone());
                    } else if entry.display_name == display_name && entry.font_id == font_id {
                        cached_prepared = Some(entry.prepared_name.clone());
                        if entry.troops_str == troops_str {
                            cached_troops = Some(entry.troops_galley.clone());
                        }
                    }
                }

                let (name_size, prepared_name, troops_galley) =
                    match (cached_prepared, cached_troops) {
                        (Some(prepared), Some(tg)) => (prepared.size, prepared, tg),
                        (Some(prepared), None) => {
                            let tg = crate::hud::nameplate::layout_nameplate_troops_galley(
                                painter,
                                troops_font_id.clone(),
                                &troops_str,
                            );
                            ui.nameplate_galleys.insert(
                                player.id,
                                crate::app::CachedNameplate {
                                    display_name: display_name.clone(),
                                    troops_str: troops_str.clone(),
                                    font_id: font_id.clone(),
                                    prepared_name: prepared.clone(),
                                    troops_galley: tg.clone(),
                                },
                            );
                            ui.nameplate_troops_last_update
                                .insert(player.id, web_time::Instant::now());
                            (prepared.size, prepared, tg)
                        }
                        _ => {
                            let prepared =
                                sow_ui::widgets::prepare_name(painter, &display_name, &font_id);
                            let tg = crate::hud::nameplate::layout_nameplate_troops_galley(
                                painter,
                                troops_font_id.clone(),
                                &troops_str,
                            );
                            ui.nameplate_galleys.insert(
                                player.id,
                                crate::app::CachedNameplate {
                                    display_name: display_name.clone(),
                                    troops_str: troops_str.clone(),
                                    font_id: font_id.clone(),
                                    prepared_name: prepared.clone(),
                                    troops_galley: tg.clone(),
                                },
                            );
                            ui.nameplate_troops_last_update
                                .insert(player.id, web_time::Instant::now());
                            (prepared.size, prepared, tg)
                        }
                    };

                let right_w = name_size.x.max(crate::hud::nameplate::troops_row_width(
                    &troops_galley,
                    &troops_font_id,
                ));
                let item_spacing_y = (font_size * 0.111).round();
                let right_h = name_size.y + item_spacing_y + troops_galley.rect.height();

                let spacing_x = (font_size * 0.333).round();
                let mut total_w = avatar_size + spacing_x + right_w;
                if is_me {
                    total_w += avatar_size + spacing_x;
                }
                let total_h = avatar_size.max(right_h);

                let mut row0_h = 0.0;
                if let Some(ref dg) = disc_galley {
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
                if let Some(dg) = disc_galley {
                    let row0_pos = egui::pos2(center.x - dg.rect.width() / 2.0, content_min.y);
                    painter.galley(row0_pos, dg, egui::Color32::WHITE);
                }

                // Row 1 & 2 layout
                let row12_y = content_min.y + row0_h;
                let mut cur_x = content_min.x;

                // 0. Star (if me)
                if is_me {
                    let star_rect = egui::Rect::from_min_size(
                        egui::pos2(cur_x, row12_y + (total_h - avatar_size) / 2.0),
                        egui::vec2(avatar_size, avatar_size),
                    );
                    if !sow_ui::widgets::try_paint_emoji(painter, "⭐", star_rect, egui::Color32::WHITE) {
                        painter.text(
                            star_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "⭐",
                            egui::FontId::proportional(avatar_size * 0.7),
                            egui::Color32::WHITE,
                        );
                    }
                    cur_x += avatar_size + spacing_x;
                }

                // 1. Circular avatar with decorative frame
                let avatar_center = egui::pos2(cur_x + avatar_size / 2.0, row12_y + total_h / 2.0);
                let avatar_r = avatar_size / 2.0;
                if player.player_type == sow_core::player::PlayerType::Nation {
                    paint_circular_avatar(
                        painter,
                        avatar_center,
                        avatar_r,
                        None,
                        vibrant_color,
                        vibrant_color,
                    );
                } else if player.player_type == sow_core::player::PlayerType::Bot {
                    paint_circular_avatar(
                        painter,
                        avatar_center,
                        avatar_r,
                        None,
                        vibrant_color,
                        vibrant_color,
                    );
                    let animal = sow_core::player::tribe_animal(player.id, &player.name);
                    let emoji_size = avatar_size * 0.7;
                    let emoji_rect = egui::Rect::from_center_size(
                        avatar_center,
                        egui::vec2(emoji_size, emoji_size),
                    );
                    if !sow_ui::widgets::try_paint_emoji(
                        painter,
                        animal,
                        emoji_rect,
                        egui::Color32::WHITE,
                    ) {
                        let emoji_galley = painter.layout_no_wrap(
                            animal.to_owned(),
                            egui::FontId::proportional(emoji_size),
                            egui::Color32::WHITE,
                        );
                        let emoji_pos = egui::pos2(
                            avatar_center.x - emoji_galley.size().x / 2.0,
                            avatar_center.y - emoji_galley.size().y / 2.0,
                        );
                        painter.galley(emoji_pos, emoji_galley, egui::Color32::WHITE);
                    }
                } else {
                    let leader_rgb = player.leader.filler_rgb();
                    let leader_color = egui::Color32::from_rgb(
                        (leader_rgb[0] * 255.0).round() as u8,
                        (leader_rgb[1] * 255.0).round() as u8,
                        (leader_rgb[2] * 255.0).round() as u8,
                    );
                    let avatar_tex = ui.app.asset_loader.avatars.get(&player.leader).or(ui
                        .app
                        .asset_loader
                        .avatar_fallback
                        .as_ref());
                    let tex_id = avatar_tex.map(|t| t.id());
                    paint_circular_avatar(
                        painter,
                        avatar_center,
                        avatar_r,
                        tex_id,
                        leader_color,
                        leader_color,
                    );
                }
                cur_x += avatar_size + spacing_x;

                // 2. Nickname and Troops centered in right block
                let right_y = row12_y + (total_h - right_h) / 2.0;

                let name_x = cur_x + (right_w - name_size.x) / 2.0;
                sow_ui::widgets::paint_prepared_name(
                    painter,
                    egui::pos2(name_x, right_y),
                    egui::Align2::LEFT_TOP,
                    &prepared_name,
                    vibrant_color,
                    true,
                );

                let troops_w =
                    crate::hud::nameplate::troops_row_width(&troops_galley, &troops_font_id);
                let troops_x = cur_x + (right_w - troops_w) / 2.0;
                crate::hud::nameplate::paint_glow_troops_row(
                    painter,
                    egui::pos2(troops_x, right_y + name_size.y + item_spacing_y),
                    troops_galley,
                    &troops_font_id,
                    vibrant_color,
                    false,
                );
                continue;
            } else {
                painter.circle_filled(center, dot_r, pc);
                painter.circle_stroke(
                    center,
                    dot_r,
                    egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(180)),
                );
            }
        }
}

pub(crate) fn render_death_nameplates(
    ui: &mut crate::app::UiState,
    input: &crate::app::InputState,
    sf: f32,
    now: web_time::Instant,
) {
    if ui.death_nameplates.is_empty() {
        return;
    }

    // Always request repaint if we have active death animations running to keep high FPS
    ui.egui_ctx.request_repaint();

    let painter = ui.egui_ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("death_nameplates"),
    ));
    let painter = &painter;

    let visual_config = ClientVisualConfig::default();
    let ui_text_scale = visual_config.ui_text_scale;

    ui.death_nameplates.retain(|anim| {
        let elapsed = now.duration_since(anim.start_time).as_secs_f32();
        let duration = anim.duration.as_secs_f32();
        if elapsed >= duration {
            return false;
        }

        let t = elapsed / duration;
        let s = anim.seed as f32;

        // --- Layout Coordinates (Smooth Rise & Damped Wobble) ---
        let rise_dist = 6.0 * t * (2.0 - t); // quadratic ease-out rise
        let rise_screen = rise_dist * input.camera_zoom / sf;
        let wobble_x = (elapsed * 5.0 + s).sin() * 15.0 * (1.0 - t); // gentle sway

        let nx = (input.camera_x + anim.world_x * input.camera_zoom) / sf + wobble_x;
        let ny = (input.camera_y + anim.world_y * input.camera_zoom) / sf - rise_screen;
        let center = egui::pos2(nx, ny);

        // Frustum cull
        if nx < -300.0 || nx > input.screen_w + 300.0 || ny < -300.0 || ny > input.screen_h + 300.0
        {
            return true;
        }

        // Spring entry scale (pops up smoothly from shrunk state)
        let entry_scale = spring_overshoot((t / 0.3).clamp(0.0, 1.0));

        // --- Typography & Size Calculations ---
        const NAMEPLATE_RENDER_SCALE: f32 = 0.4;
        let base_premium_size = if anim.player_type == sow_core::player::PlayerType::Bot {
            visual_config.death_nameplate_font_size
        } else if anim.nameplate_size > 0.1 {
            anim.nameplate_size * NAMEPLATE_RENDER_SCALE
        } else {
            visual_config.death_nameplate_font_size
        };

        // QUANTIZATION: Round sizes to nearest integers
        let font_size = (base_premium_size * ui_text_scale * entry_scale)
            .round()
            .max(1.0);
        let font_id = egui::FontId::proportional(font_size);

        let display_name = if anim.player_type == sow_core::player::PlayerType::Bot {
            if anim.name.is_empty() {
                format!("Tribe {}", anim.player_id.saturating_sub(199))
            } else {
                anim.name.clone()
            }
        } else {
            sow_core::player::display_name(anim.player_id, &anim.name, anim.player_type)
        };

        let name_size = crate::hud::nameplate::name_label_size(painter, &display_name, &font_id);

        // Dove represents the final premium avatar (2.2x scale)
        let avatar_size = (base_premium_size * 2.2 * ui_text_scale * entry_scale)
            .round()
            .max(2.0);
        let spacing_x = (font_size * 0.4).round();
        let total_w = avatar_size + spacing_x + name_size.x;

        let start_x = center.x - total_w / 2.0;

        // Bright visibility curve: rapid fade-in, solid middle, smooth late fade-out (no early muddy darks)
        let alpha = if t < 0.10 {
            ((t / 0.10) * 255.0) as u8
        } else if t > 0.60 {
            (((1.0 - t) / 0.40) * 255.0).clamp(0.0, 255.0) as u8
        } else {
            255
        };

        if alpha == 0 {
            return true;
        }

        let vibrant_color = egui::Color32::from_rgba_unmultiplied(
            anim.color.r(),
            anim.color.g(),
            anim.color.b(),
            alpha,
        );

        // --- 1. Draw Dove (Final Avatar) on the Left ---
        let dove_rect = egui::Rect::from_center_size(
            egui::pos2(start_x + avatar_size / 2.0, center.y),
            egui::vec2(avatar_size, avatar_size),
        );
        let soul_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);

        if !sow_ui::widgets::try_paint_emoji(painter, "🕊️", dove_rect, soul_color) {
            let emoji_galley = painter.layout_no_wrap(
                "🕊️".to_owned(),
                egui::FontId::proportional(avatar_size),
                soul_color,
            );
            let emoji_pos = egui::pos2(
                dove_rect.center().x - emoji_galley.size().x / 2.0,
                dove_rect.center().y - emoji_galley.size().y / 2.0,
            );
            painter.galley(emoji_pos, emoji_galley, soul_color);
        }

        // --- 2. Draw Glow Name on the Right ---
        let name_x = start_x + avatar_size + spacing_x;
        let name_y = center.y - name_size.y / 2.0;

        crate::hud::nameplate::paint_glow_name_label(
            painter,
            egui::pos2(name_x, name_y),
            &display_name,
            font_id,
            vibrant_color,
            false,
        );

        true
    });
}
