use super::*;

const REQUEST_WEBP: &[u8] = sow_core::repo_asset_bytes!("icons/request.webp");
const HANDSHAKE_WEBP: &[u8] = sow_core::repo_asset_bytes!("icons/handshake.webp");
const BETRAY_WEBP: &[u8] = sow_core::repo_asset_bytes!("icons/betray.webp");

/// Damped spring overshoot: approaches 1.0 with a single bounce.
#[inline]
pub(crate) fn spring_overshoot(t: f32) -> f32 {
    1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
}

fn ensure_text_readability(rgb: [f32; 3], target_lum: f32) -> egui::Color32 {
    let lum = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    if lum < target_lum {
        let t = (target_lum - lum) / (1.0 - lum).max(0.001);
        let r = rgb[0] + (1.0 - rgb[0]) * t;
        let g = rgb[1] + (1.0 - rgb[1]) * t;
        let b = rgb[2] + (1.0 - rgb[2]) * t;
        egui::Color32::from_rgb(
            (r * 255.0).clamp(0.0, 255.0) as u8,
            (g * 255.0).clamp(0.0, 255.0) as u8,
            (b * 255.0).clamp(0.0, 255.0) as u8,
        )
    } else {
        egui::Color32::from_rgb(
            (rgb[0] * 255.0).clamp(0.0, 255.0) as u8,
            (rgb[1] * 255.0).clamp(0.0, 255.0) as u8,
            (rgb[2] * 255.0).clamp(0.0, 255.0) as u8,
        )
    }
}

use crate::hud::avatar::paint_circular_avatar;

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
    let terrain = ctx.terrain;

    if visible_players.is_empty() {
        return;
    }

    let is_water = |tile_idx: u32| {
        let t = terrain.get(tile_idx as usize).copied().unwrap_or(0);
        (t & 0x80) == 0
    };

    if let Some(snap) = &sim.current_snapshot {
        static REGISTER_STAR_ONCE: std::sync::Once = std::sync::Once::new();
        REGISTER_STAR_ONCE.call_once(|| {
            painter.ctx().include_bytes(
                "bytes://star.webp",
                sow_core::repo_asset_bytes!("icons/star.webp").as_slice(),
            );
        });

        // visible_players is pre-sorted in mod.rs (local player last, then humans, nations, presence)
        let mut full_labels_drawn = 0;

        let visual_config = ClientVisualConfig::default();
        let ui_text_scale = visual_config.ui_text_scale;
        let zoom_scale = (input.camera_zoom / sf).clamp(0.1, 1.0);

        // Precompute scaled nameplate font sizes once per frame for 100% CPU/memory efficiency!
        // Round to whole point sizes to prevent egui glyph atlas invalidations.
        let mut font_size_my =
            ((visual_config.nameplate_my_size * ui_text_scale * zoom_scale).round()).max(2.0);
        let mut font_size_nation =
            ((visual_config.nameplate_nation_size * ui_text_scale * zoom_scale).round()).max(2.0);
        let mut font_size_tribe =
            ((visual_config.nameplate_tribe_size * ui_text_scale * zoom_scale).round()).max(3.0);

        if zoom_scaled < 0.6 {
            font_size_my *= 0.5;
            font_size_nation *= 0.5;
            font_size_tribe *= 0.5;
        }

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

            // LOD 3 optimization: Simplify visuals, no avatars, flat text, dot fallbacks
            if zoom_scaled < 0.6 {
                let map_area = (sim.map_w * sim.map_h).max(1) as f32;
                let normalized_tiles = player.tile_count as f32 * (40_000.0 / map_area);
                // Stricter threshold for nameplate text on LOD 3
                let min_tiles = if player.id >= 200 { 24.0 } else { 6.0 };
                let show_full = is_human
                    || ((normalized_tiles * zoom_scaled * zoom_scaled) >= min_tiles
                        && full_labels_drawn < 50);

                if show_full {
                    if !is_human {
                        full_labels_drawn += 1;
                    }
                    let font_size = if vp.nameplate_size > 0.1 {
                        let scaled_size = vp.nameplate_size * 0.4 * ui_text_scale * (input.camera_zoom / sf);
                        scaled_size.round().max(4.0)
                    } else {
                        if is_me {
                            font_size_my
                        } else if player.id < 200 {
                            font_size_nation
                        } else {
                            font_size_tribe
                        }
                    };
                    let font_id = egui::FontId::proportional(font_size);

                    let display_name = sow_core::player::display_name(player.id, &player.name, player.player_type);
                    let name_size =
                        crate::hud::nameplate::name_label_size(painter, &display_name, &font_id);

                    let name_pos = egui::pos2(
                        center.x - name_size.x / 2.0,
                        center.y - name_size.y / 2.0,
                    );
                    let rgb = player.color;
                    // Ensure minimum brightness for flat text (LOD 3 has no outline)
                    let text_color = ensure_text_readability(rgb, 0.55);

                    crate::hud::nameplate::paint_flat_name_label(
                        painter,
                        name_pos,
                        &display_name,
                        font_id.clone(),
                        text_color,
                    );
                } else {
                    // High-performance dot fallback
                    painter.circle_filled(center, dot_r * 0.8, pc);
                    painter.circle_stroke(
                        center,
                        dot_r * 0.8,
                        egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(180)),
                    );
                }
                continue;
            }

            let map_area = (sim.map_w * sim.map_h).max(1) as f32;
            let normalized_tiles = player.tile_count as f32 * (40_000.0 / map_area);
            let zoom_scaled_local = input.camera_zoom / sf;
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

                let scale_factor = (zoom_scaled / 4.0).clamp(0.6, 1.2);
                let base_premium_size = if vp.nameplate_size > 0.1 {
                    (vp.nameplate_size * zoom_scaled_local).clamp(6.0, 24.0)
                } else {
                    visual_config.nameplate_premium_size * scale_factor
                };
                let font_size =
                    base_premium_size * ui_text_scale;
                let avatar_size = (base_premium_size * 2.2)
                    * ui_text_scale;

                    // Check alliance status with the player
                    let mut is_allied = false;
                    let mut is_heart_flashing = false;
                    let mut has_req = false;
                    if my_id != player.id {
                        if let Some(me) = my_player {
                            if me.alliances.contains(&player.id) {
                                is_allied = true;
                                let timer =
                                    me.alliance_timers.get(&player.id).copied().unwrap_or(2400);
                                let has_pending_proposal =
                                    me.alliance_requests.contains(&player.id)
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
                        let has_betrayal = player.active_emoji.as_deref() == Some("🗡️");
                        if has_betrayal {
                            betrayal_flash = true;
                        }
                    }

                    // Retrieve or save persistent client-side state for the floating express emoji
                    let last_emoji_id = egui::Id::new(("last_active_emoji", player.id));
                    let mut current_emoji = player.active_emoji.clone();
                    let is_active =
                        current_emoji.is_some() && current_emoji.as_deref() != Some("🗡️");

                    let active_anim_id = egui::Id::new(("emoji_anim_progress", player.id));
                    let anim_progress =
                        painter
                            .ctx()
                            .animate_bool_with_time(active_anim_id, is_active, 0.25);

                    if current_emoji.is_none() || current_emoji.as_deref() == Some("🗡️") {
                        if anim_progress > 0.01 {
                            current_emoji =
                                painter.ctx().data(|d| d.get_temp::<String>(last_emoji_id));
                        }
                    } else {
                        painter.ctx().data_mut(|d| {
                            d.insert_temp(last_emoji_id, current_emoji.clone().unwrap())
                        });
                    }

                    // Request WebP Icon Animation (Spring Overshoot)
                    let request_anim_id = egui::Id::new(("request_anim_progress", player.id));
                    let req_anim =
                        painter
                            .ctx()
                            .animate_bool_with_time(request_anim_id, has_req, 0.25);

                    let mut req_offset = 0.0_f32;
                    if req_anim > 0.01 {
                        static REGISTER_REQUEST_ONCE: std::sync::Once = std::sync::Once::new();
                        REGISTER_REQUEST_ONCE.call_once(|| {
                            painter.ctx().include_bytes(
                                "bytes://request.webp",
                                REQUEST_WEBP,
                            );
                        });

                        let request_icon_size = font_size * 3.111;
                        let load_res = painter.ctx().try_load_texture(
                            "bytes://request.webp",
                            egui::TextureOptions::default(),
                            egui::load::SizeHint::Size {
                                width: 128,
                                height: 128,
                                maintain_aspect_ratio: true,
                            },
                        );

                        if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                            let anim_scale = if has_req {
                                let t = req_anim;
                                if t >= 1.0 {
                                    1.0
                                } else {
                                    spring_overshoot(t)
                                }
                            } else {
                                req_anim
                            };
                            let size = request_icon_size * anim_scale;
                            // Draw it floating centered above premium avatar
                            let req_y = center.y - (font_size * 1.889) - size / 2.0;
                            let req_rect = egui::Rect::from_center_size(
                                egui::pos2(center.x, req_y),
                                egui::vec2(size, size),
                            );

                            let request_painter = painter.ctx().layer_painter(egui::LayerId::new(
                                egui::Order::Middle,
                                egui::Id::new(("floating_request_icon", player.id)),
                            ));
                            if is_me {
                                let glow_r = size * 0.8;
                                let glow_a = req_anim * 0.35;
                                request_painter.circle_filled(
                                    req_rect.center(),
                                    glow_r * 1.4,
                                    egui::Color32::from_rgba_unmultiplied(
                                        34,
                                        211,
                                        238,
                                        (glow_a * 120.0) as u8,
                                    ),
                                );
                                request_painter.circle_filled(
                                    req_rect.center(),
                                    glow_r,
                                    egui::Color32::from_rgba_unmultiplied(
                                        34,
                                        211,
                                        238,
                                        (glow_a * 255.0) as u8,
                                    ),
                                );
                            }
                            request_painter.image(
                                texture.id,
                                req_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE.linear_multiply(req_anim),
                            );
                            req_offset = size + 4.0;
                        }
                    }

                    // Allied WebP Icon Animation (Spring Overshoot)
                    let allied_anim_id = egui::Id::new(("allied_anim_progress", player.id));
                    let allied_anim =
                        painter
                            .ctx()
                            .animate_bool_with_time(allied_anim_id, is_allied, 0.25);

                    let mut allied_offset = 0.0_f32;
                    if allied_anim > 0.01 {
                        static REGISTER_HANDSHAKE_ONCE: std::sync::Once = std::sync::Once::new();
                        REGISTER_HANDSHAKE_ONCE.call_once(|| {
                            painter.ctx().include_bytes(
                                "bytes://handshake.webp",
                                HANDSHAKE_WEBP,
                            );
                        });

                        let handshake_icon_size = font_size * 3.111;
                        let load_res = painter.ctx().try_load_texture(
                            "bytes://handshake.webp",
                            egui::TextureOptions::default(),
                            egui::load::SizeHint::Size {
                                width: 128,
                                height: 128,
                                maintain_aspect_ratio: true,
                            },
                        );

                        if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                            let anim_scale = if is_allied {
                                let t = allied_anim;
                                if t >= 1.0 {
                                    1.0
                                } else {
                                    spring_overshoot(t)
                                }
                            } else {
                                allied_anim
                            };
                            let size = handshake_icon_size * anim_scale;
                            // Draw it floating centered above premium avatar
                            let req_y = center.y - (font_size * 1.889) - size / 2.0;
                            let req_rect = egui::Rect::from_center_size(
                                egui::pos2(center.x, req_y),
                                egui::vec2(size, size),
                            );

                            let handshake_painter =
                                painter.ctx().layer_painter(egui::LayerId::new(
                                    egui::Order::Middle,
                                    egui::Id::new(("floating_handshake_icon", player.id)),
                                ));

                            let flash_alpha = if is_heart_flashing {
                                heart_flash_alpha
                            } else {
                                1.0
                            };

                            if is_me {
                                let glow_r = size * 0.8;
                                let glow_a = allied_anim * flash_alpha * 0.35;
                                handshake_painter.circle_filled(
                                    req_rect.center(),
                                    glow_r * 1.4,
                                    egui::Color32::from_rgba_unmultiplied(
                                        255,
                                        200,
                                        60,
                                        (glow_a * 120.0) as u8,
                                    ),
                                );
                                handshake_painter.circle_filled(
                                    req_rect.center(),
                                    glow_r,
                                    egui::Color32::from_rgba_unmultiplied(
                                        255,
                                        200,
                                        60,
                                        (glow_a * 255.0) as u8,
                                    ),
                                );
                            }
                            handshake_painter.image(
                                texture.id,
                                req_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE.linear_multiply(allied_anim * flash_alpha),
                            );
                            allied_offset = size + 4.0;
                        }
                    }

                    // Betrayal WebP Icon Animation (Spring Overshoot)
                    let betrayal_anim_id = egui::Id::new(("betrayal_anim_progress", player.id));
                    let betrayal_anim = painter.ctx().animate_bool_with_time(
                        betrayal_anim_id,
                        betrayal_flash,
                        0.25,
                    );

                    let mut betrayal_offset = 0.0_f32;
                    if betrayal_anim > 0.01 {
                        static REGISTER_BETRAY_ONCE: std::sync::Once = std::sync::Once::new();
                        REGISTER_BETRAY_ONCE.call_once(|| {
                            painter.ctx().include_bytes(
                                "bytes://betray.webp",
                                BETRAY_WEBP,
                            );
                        });

                        let betray_icon_size = font_size * 3.111;
                        let load_res = painter.ctx().try_load_texture(
                            "bytes://betray.webp",
                            egui::TextureOptions::default(),
                            egui::load::SizeHint::Size {
                                width: 128,
                                height: 128,
                                maintain_aspect_ratio: true,
                            },
                        );

                        if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                            let anim_scale = if betrayal_flash {
                                let t = betrayal_anim;
                                if t >= 1.0 {
                                    1.0
                                } else {
                                    spring_overshoot(t)
                                }
                            } else {
                                betrayal_anim
                            };
                            let size = betray_icon_size * anim_scale;
                            let betray_y = center.y - (font_size * 1.889) - size / 2.0;
                            let betray_rect = egui::Rect::from_center_size(
                                egui::pos2(center.x, betray_y),
                                egui::vec2(size, size),
                            );

                            let betray_painter = painter.ctx().layer_painter(egui::LayerId::new(
                                egui::Order::Middle,
                                egui::Id::new(("floating_betray_icon", player.id)),
                            ));

                            // Red danger glow
                            let glow_r = size * 0.8;
                            let glow_a = betrayal_anim * 0.4;
                            betray_painter.circle_filled(
                                betray_rect.center(),
                                glow_r * 1.4,
                                egui::Color32::from_rgba_unmultiplied(
                                    220,
                                    38,
                                    38,
                                    (glow_a * 120.0) as u8,
                                ),
                            );
                            betray_painter.circle_filled(
                                betray_rect.center(),
                                glow_r,
                                egui::Color32::from_rgba_unmultiplied(
                                    220,
                                    38,
                                    38,
                                    (glow_a * 255.0) as u8,
                                ),
                            );

                            betray_painter.image(
                                texture.id,
                                betray_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE.linear_multiply(betrayal_anim),
                            );
                            betrayal_offset = size + 4.0;
                        }
                    }

                    // Render animated floating active express emoji ABOVE the nameplate
                    if anim_progress > 0.01 {
                        if let Some(emoji_str) = &current_emoji {
                            let anim_scale = if is_active {
                                let t = anim_progress;
                                if t >= 1.0 {
                                    1.0
                                } else {
                                    spring_overshoot(t)
                                }
                            } else {
                                anim_progress
                            };

                            let base_emoji_size = font_size * 3.2;
                            let final_emoji_size = base_emoji_size * anim_scale;

                            let mut base_y_offset = font_size * 1.889;
                            let max_float_offset =
                                req_offset.max(allied_offset).max(betrayal_offset);
                            if max_float_offset > 0.01 {
                                base_y_offset += max_float_offset;
                            }
                            let emoji_y =
                                center.y - base_y_offset - (12.0 * zoom_scale * ui_text_scale);

                            if final_emoji_size > 1.0 {
                                let emoji_painter =
                                    painter.ctx().layer_painter(egui::LayerId::new(
                                        egui::Order::Middle,
                                        egui::Id::new(("floating_express_emoji", player.id)),
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
                                    emoji_painter.galley(
                                        emoji_pos,
                                        emoji_galley,
                                        egui::Color32::WHITE,
                                    );
                                }
                            }
                        }
                    }
                    let rgb = player.color;
                    let vibrant_color = ensure_text_readability(rgb, 0.45);

                    let mut disc_galley = None;
                    if is_disconnected {
                        let disc_font_id = egui::FontId::proportional(font_size * 0.95 * 3.0);
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
                    let name_size =
                        crate::hud::nameplate::name_label_size(painter, &display_name, &font_id);

                    let mut cached_troops = None;
                    if let Some(entry) = ui.nameplate_galleys.get(&player.id) {
                        if entry.0 == display_name && entry.1 == troops_str && entry.2 == font_id {
                            cached_troops = Some(entry.3.clone());
                        }
                    }

                    let troops_font_size = font_size * 1.30;
                    let troops_font_id = egui::FontId::proportional(troops_font_size);

                    let troops_galley = if let Some(tg) = cached_troops {
                        tg
                    } else {
                        let tg = crate::hud::nameplate::layout_nameplate_troops_galley(
                            painter,
                            troops_font_id.clone(),
                            &troops_str,
                        );

                        ui.nameplate_galleys.insert(
                            player.id,
                            (
                                display_name.clone(),
                                troops_str,
                                font_id.clone(),
                                tg.clone(),
                            ),
                        );

                        tg
                    };

                    let right_w = name_size.x.max(
                        crate::hud::nameplate::troops_row_width(&troops_galley, &troops_font_id),
                    );
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
                    let content_min =
                        egui::pos2(center.x - total_w / 2.0, center.y - content_h / 2.0);

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
                        let star_uri = "bytes://star.webp";
                        let size_hint = egui::load::SizeHint::Size {
                            width: 128,
                            height: 128,
                            maintain_aspect_ratio: true,
                        };
                        if let Ok(egui::load::TexturePoll::Ready { texture }) = painter
                            .ctx()
                            .try_load_texture(star_uri, egui::TextureOptions::default(), size_hint)
                        {
                            painter.image(
                                texture.id,
                                star_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        }
                        cur_x += avatar_size + spacing_x;
                    }

                    // 1. Circular avatar with decorative frame
                    let avatar_center =
                        egui::pos2(cur_x + avatar_size / 2.0, row12_y + total_h / 2.0);
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
                        let animal =
                            sow_core::player::tribe_animal(player.id, &player.name);
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
                    crate::hud::nameplate::paint_glow_name_label(
                        &painter,
                        egui::pos2(name_x, right_y),
                        &display_name,
                        font_id.clone(),
                        vibrant_color,
                        false,
                    );

                    let troops_w =
                        crate::hud::nameplate::troops_row_width(&troops_galley, &troops_font_id);
                    let troops_x = cur_x + (right_w - troops_w) / 2.0;
                    crate::hud::nameplate::paint_glow_troops_row(
                        &painter,
                        egui::pos2(
                            troops_x,
                            right_y + name_size.y + item_spacing_y,
                        ),
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

    // Always request repaint if we have active death animations running
    ui.egui_ctx.request_repaint();

    let painter = ui.egui_ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("death_nameplates"),
    ));
    let painter = &painter;

    let visual_config = ClientVisualConfig::default();
    let ui_text_scale = visual_config.ui_text_scale;

    const PLATE_STAGGER: f32 = 0.4;

    ui.death_nameplates.retain(|anim| {
        let elapsed = now.duration_since(anim.start_time).as_secs_f32();
        let duration = anim.duration.as_secs_f32();
        if elapsed >= duration {
            return false;
        }

        let t = elapsed / duration;
        let s = anim.seed as f32;
        let zoom_scaled_local = input.camera_zoom / sf;
        let zoom_scale = zoom_scaled_local.clamp(0.1, 1.0);

        let plate_center_x = (input.camera_x + anim.world_x * input.camera_zoom) / sf;
        let plate_center_y = (input.camera_y + anim.world_y * input.camera_zoom) / sf;
        let center = egui::pos2(plate_center_x, plate_center_y);

        let base_premium_size = if anim.nameplate_size > 0.1 {
            (anim.nameplate_size * zoom_scaled_local).clamp(10.0, 36.0)
        } else {
            visual_config.death_nameplate_font_size
        };
        let font_size = base_premium_size * ui_text_scale;

        // --- 1. Soul (dove) — upper row, same protocol as floating express emoji ---
        {
            let soul_t = t;
            let rise_dist = 6.0 * soul_t * (2.0 - soul_t);
            let rise_screen = rise_dist * input.camera_zoom / sf;
            let wobble_x = (elapsed * 5.0 + s).sin() * 15.0 * (1.0 - soul_t);

            let entry_scale = spring_overshoot((elapsed / 0.25).clamp(0.0, 1.0));
            let base_emoji_size = font_size * 3.2 * 1.2;
            let final_emoji_size = base_emoji_size * entry_scale;

            let base_y_offset = font_size * 1.889 + 12.0 * zoom_scale * ui_text_scale;
            let emoji_y = center.y - base_y_offset - rise_screen;

            let soul_alpha = if soul_t < 0.08 {
                ((soul_t / 0.08) * 255.0) as u8
            } else if soul_t > 0.7 {
                (((1.0 - soul_t) / 0.3) * 255.0).clamp(0.0, 255.0) as u8
            } else {
                255
            };

            if final_emoji_size > 1.0 && soul_alpha > 0 {
                let soul_painter = painter.ctx().layer_painter(egui::LayerId::new(
                    egui::Order::Middle,
                    egui::Id::new(("death_soul", anim.player_id, anim.seed)),
                ));
                let emoji_rect = egui::Rect::from_center_size(
                    egui::pos2(center.x + wobble_x, emoji_y),
                    egui::vec2(final_emoji_size, final_emoji_size),
                );
                let soul_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, soul_alpha);
                if !sow_ui::widgets::try_paint_emoji(&soul_painter, "🕊️", emoji_rect, soul_color) {
                    let emoji_galley = soul_painter.layout_no_wrap(
                        "🕊️".to_owned(),
                        egui::FontId::proportional(final_emoji_size),
                        soul_color,
                    );
                    let emoji_pos = egui::pos2(
                        center.x + wobble_x - emoji_galley.size().x / 2.0,
                        emoji_y - emoji_galley.size().y / 2.0,
                    );
                    soul_painter.galley(emoji_pos, emoji_galley, soul_color);
                }
            }
        }

        // --- 2. Premium nameplate (staggered entry) ---
        if elapsed > PLATE_STAGGER {
            let n_elapsed = elapsed - PLATE_STAGGER;
            let nt = (n_elapsed / (duration - PLATE_STAGGER)).clamp(0.0, 1.0);

            let sink_y = nt * 1.0;
            let tremble = if n_elapsed < 0.5 {
                (n_elapsed * 50.0).sin() * 3.0 * (1.0 - n_elapsed / 0.5)
            } else {
                0.0
            };

            let nx = plate_center_x;
            let ny = (input.camera_y + (anim.world_y + 2.5 + sink_y) * input.camera_zoom) / sf + tremble;
            let center = egui::pos2(nx, ny);

            if nx < -300.0 || nx > input.screen_w + 300.0 || ny < -300.0 || ny > input.screen_h + 300.0 {
                return true;
            }

            let entry_duration = 0.5;
            let plate_scale = if n_elapsed < entry_duration {
                spring_overshoot((n_elapsed / entry_duration).clamp(0.0, 1.0))
            } else if elapsed > duration - 0.35 {
                let fade_t = (duration - elapsed) / 0.35;
                fade_t * fade_t
            } else {
                1.0
            };

            let alpha = if nt < 0.12 {
                ((nt / 0.12) * 255.0) as u8
            } else if elapsed > duration - 0.35 {
                (((duration - elapsed) / 0.35) * 255.0) as u8
            } else {
                255
            };

            let scaled_size = base_premium_size * plate_scale;
            if scaled_size > 1.0 {
                let font_size = scaled_size * ui_text_scale;
                let font_id = egui::FontId::proportional(font_size.round().max(1.0));

                let display_name = if anim.player_type == sow_core::player::PlayerType::Bot {
                    if anim.name.is_empty() {
                        format!("Tribe {}", anim.player_id.saturating_sub(199))
                    } else {
                        anim.name.clone()
                    }
                } else {
                    sow_core::player::display_name(anim.player_id, &anim.name, anim.player_type)
                };

                let name_size =
                    crate::hud::nameplate::name_label_size(painter, &display_name, &font_id);
                let avatar_size = scaled_size * 2.2 * ui_text_scale;

                let troops_str = sow_ui::utils::format_number(anim.troops);
                let troops_font_size = font_size * 1.30;
                let troops_font_id = egui::FontId::proportional(troops_font_size.round().max(1.0));

                let troops_galley = crate::hud::nameplate::layout_nameplate_troops_galley(
                    painter,
                    troops_font_id.clone(),
                    &troops_str,
                );

                let right_w = name_size.x.max(
                    crate::hud::nameplate::troops_row_width(&troops_galley, &troops_font_id),
                );
                let item_spacing_y = (font_size * 0.111).round();
                let right_h = name_size.y + item_spacing_y + troops_galley.rect.height();

                let spacing_x = (font_size * 0.333).round();
                let total_w = avatar_size + spacing_x + right_w;
                let total_h = avatar_size.max(right_h);

                let content_min = egui::pos2(center.x - total_w / 2.0, center.y - total_h / 2.0);
                let row12_y = content_min.y;
                let mut cur_x = content_min.x;

                let vibrant_color = egui::Color32::from_rgba_unmultiplied(
                    anim.color.r(),
                    anim.color.g(),
                    anim.color.b(),
                    alpha,
                );

                let avatar_center = egui::pos2(cur_x + avatar_size / 2.0, row12_y + total_h / 2.0);
                let avatar_r = avatar_size / 2.0;

                if anim.player_type == sow_core::player::PlayerType::Nation {
                    paint_circular_avatar(
                        painter,
                        avatar_center,
                        avatar_r,
                        None,
                        vibrant_color,
                        vibrant_color,
                    );
                } else if anim.player_type == sow_core::player::PlayerType::Bot {
                    paint_circular_avatar(
                        painter,
                        avatar_center,
                        avatar_r,
                        None,
                        vibrant_color,
                        vibrant_color,
                    );
                    let animal = sow_core::player::tribe_animal(anim.player_id, &anim.name);
                    let emoji_size = avatar_size * 0.7;
                    let emoji_rect = egui::Rect::from_center_size(
                        avatar_center,
                        egui::vec2(emoji_size, emoji_size),
                    );
                    let animal_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
                    if !sow_ui::widgets::try_paint_emoji(painter, animal, emoji_rect, animal_color)
                    {
                        let emoji_galley = painter.layout_no_wrap(
                            animal.to_owned(),
                            egui::FontId::proportional(emoji_size),
                            egui::Color32::WHITE,
                        );
                        let emoji_pos = egui::pos2(
                            avatar_center.x - emoji_galley.size().x / 2.0,
                            avatar_center.y - emoji_galley.size().y / 2.0,
                        );
                        painter.galley(emoji_pos, emoji_galley, animal_color);
                    }
                } else {
                    let leader_rgb = anim.leader.filler_rgb();
                    let leader_color = egui::Color32::from_rgba_unmultiplied(
                        (leader_rgb[0] * 255.0).round() as u8,
                        (leader_rgb[1] * 255.0).round() as u8,
                        (leader_rgb[2] * 255.0).round() as u8,
                        alpha,
                    );
                    let avatar_tex = ui.app.asset_loader.avatars.get(&anim.leader).or(ui
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

                let right_y = row12_y + (total_h - right_h) / 2.0;
                let name_x = cur_x + (right_w - name_size.x) / 2.0;
                crate::hud::nameplate::paint_glow_name_label(
                    painter,
                    egui::pos2(name_x, right_y),
                    &display_name,
                    font_id.clone(),
                    vibrant_color,
                    false,
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
            }
        }

        true
    });
}
