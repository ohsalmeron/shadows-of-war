use super::*;

/// Damped spring overshoot: approaches 1.0 with a single bounce.
#[inline]
pub(crate) fn spring_overshoot(t: f32) -> f32 {
    1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
}

/// Paints a circular avatar with a decorative ring frame.
/// For textured avatars, clips to a circle via a triangle-fan mesh.
/// For solid-color avatars (nations), fills a circle.
fn paint_circular_avatar(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    texture: Option<egui::TextureId>,
    fill_color: egui::Color32,
    frame_color: egui::Color32,
) {
    const SEGMENTS: usize = 32;

    if let Some(tex_id) = texture {
        // Build a triangle-fan mesh clipped to a circle
        let mut mesh = egui::Mesh::with_texture(tex_id);
        // Center vertex
        mesh.vertices.push(egui::epaint::Vertex {
            pos: center,
            uv: egui::pos2(0.5, 0.5),
            color: egui::Color32::WHITE,
        });
        for i in 0..=SEGMENTS {
            let angle = (i as f32 / SEGMENTS as f32) * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            mesh.vertices.push(egui::epaint::Vertex {
                pos: egui::pos2(center.x + cos * radius, center.y + sin * radius),
                uv: egui::pos2(0.5 + cos * 0.5, 0.5 + sin * 0.5),
                color: egui::Color32::WHITE,
            });
        }
        for i in 1..=SEGMENTS {
            mesh.indices.push(0);
            mesh.indices.push(i as u32);
            mesh.indices.push(i as u32 + 1);
        }
        painter.add(egui::Shape::mesh(mesh));
    } else {
        painter.circle_filled(center, radius, fill_color);
    }

    // Frame rings: dark backdrop → color ring → white highlight
    let border = (radius * 0.12).max(1.0);
    painter.circle_stroke(
        center,
        radius + border * 0.3,
        egui::Stroke::new(border, egui::Color32::from_black_alpha(160)),
    );
    painter.circle_stroke(
        center,
        radius,
        egui::Stroke::new(border * 0.8, frame_color),
    );
    painter.circle_stroke(
        center,
        radius - border * 0.15,
        egui::Stroke::new(border * 0.35, egui::Color32::from_white_alpha(80)),
    );
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
                "bytes://star.svg",
                include_bytes!("../../../assets/star.svg").as_slice(),
            );
        });

        // visible_players is pre-sorted in mod.rs (humans first, nations, by presence desc)
        let mut full_labels_drawn = 0;
        let mut premium_labels_drawn = 0;

        let visual_config = ClientVisualConfig::default();
        let ui_text_scale = visual_config.ui_text_scale;
        let zoom_scale = (input.camera_zoom / sf).clamp(0.1, 1.0);

        // Precompute scaled nameplate font sizes once per frame for 100% CPU/memory efficiency!
        // Round to whole point sizes to prevent egui glyph atlas invalidations.
        let font_size_my =
            ((visual_config.nameplate_my_size * ui_text_scale * zoom_scale).round()).max(2.0);
        let font_size_nation =
            ((visual_config.nameplate_nation_size * ui_text_scale * zoom_scale).round()).max(2.0);
        let font_size_tribe =
            ((visual_config.nameplate_tribe_size * ui_text_scale * zoom_scale).round()).max(3.0);

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
            let draw_as_premium = is_human
                || (player.player_type == sow_core::player::PlayerType::Nation
                    && zoom_scaled >= 1.5);

            if draw_as_premium {
                // --- premium human player drawing ---
                let should_draw_premium =
                    is_human || (zoom_scale >= 0.18_f32 && premium_labels_drawn < 16);

                if should_draw_premium {
                    if !is_human {
                        premium_labels_drawn += 1;
                    }

                    let scale_factor = (zoom_scaled / 4.0).clamp(0.6, 1.2);
                    let font_size =
                        visual_config.nameplate_premium_size * scale_factor * ui_text_scale;
                    let avatar_size = (visual_config.nameplate_premium_size * 2.736)
                        * scale_factor
                        * ui_text_scale;
                    let inner_margin = egui::Margin::symmetric(
                        ((visual_config.nameplate_premium_size * 0.444)
                            * scale_factor
                            * ui_text_scale)
                            .round() as i8,
                        ((visual_config.nameplate_premium_size * 0.333)
                            * scale_factor
                            * ui_text_scale)
                            .round() as i8,
                    );
                    let corner_radius = ((visual_config.nameplate_premium_size * 0.444)
                        * scale_factor
                        * ui_text_scale)
                        .round() as u8;
                    let avatar_corner = (avatar_size / 2.0).round() as u8;

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
                                if timer <= 600 && !has_pending_proposal {
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
                                include_bytes!("../../../assets/request.webp").as_slice(),
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
                                include_bytes!("../../../assets/handshake.webp").as_slice(),
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
                    let betrayal_anim =
                        painter
                            .ctx()
                            .animate_bool_with_time(betrayal_anim_id, betrayal_flash, 0.25);

                    let mut betrayal_offset = 0.0_f32;
                    if betrayal_anim > 0.01 {
                        static REGISTER_BETRAY_ONCE: std::sync::Once = std::sync::Once::new();
                        REGISTER_BETRAY_ONCE.call_once(|| {
                            painter.ctx().include_bytes(
                                "bytes://betray.webp",
                                include_bytes!("../../../assets/betray.webp").as_slice(),
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
                                if t >= 1.0 { 1.0 } else { spring_overshoot(t) }
                            } else {
                                betrayal_anim
                            };
                            let size = betray_icon_size * anim_scale;
                            let betray_y = center.y - (font_size * 1.889) - size / 2.0;
                            let betray_rect = egui::Rect::from_center_size(
                                egui::pos2(center.x, betray_y),
                                egui::vec2(size, size),
                            );

                            let betray_painter =
                                painter.ctx().layer_painter(egui::LayerId::new(
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
                                    220, 38, 38,
                                    (glow_a * 120.0) as u8,
                                ),
                            );
                            betray_painter.circle_filled(
                                betray_rect.center(),
                                glow_r,
                                egui::Color32::from_rgba_unmultiplied(
                                    220, 38, 38,
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

                            let base_emoji_size = font_size * 2.2;
                            let final_emoji_size = base_emoji_size * anim_scale;

                            let mut base_y_offset = font_size * 1.889;
                            let max_float_offset = req_offset.max(allied_offset).max(betrayal_offset);
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
                                if is_me {
                                    let glow_r = final_emoji_size * 0.7;
                                    let glow_a = anim_progress * 0.30;
                                    let emoji_center = egui::pos2(center.x, emoji_y);
                                    emoji_painter.circle_filled(
                                        emoji_center,
                                        glow_r * 1.3,
                                        egui::Color32::from_rgba_unmultiplied(
                                            pc.r(),
                                            pc.g(),
                                            pc.b(),
                                            (glow_a * 100.0) as u8,
                                        ),
                                    );
                                    emoji_painter.circle_filled(
                                        emoji_center,
                                        glow_r,
                                        egui::Color32::from_rgba_unmultiplied(
                                            pc.r(),
                                            pc.g(),
                                            pc.b(),
                                            (glow_a * 255.0) as u8,
                                        ),
                                    );
                                }
                                if emoji_str.contains('⭐') {
                                    let star_size = final_emoji_size * 1.25;
                                    let star_rect = egui::Rect::from_center_size(
                                        egui::pos2(center.x, emoji_y),
                                        egui::vec2(star_size, star_size),
                                    );
                                    let size_hint = egui::load::SizeHint::Size {
                                        width: 128,
                                        height: 128,
                                        maintain_aspect_ratio: true,
                                    };
                                    let load_res = emoji_painter.ctx().try_load_texture(
                                        "bytes://star.svg",
                                        egui::TextureOptions::default(),
                                        size_hint,
                                    );
                                    if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res
                                    {
                                        emoji_painter.image(
                                            texture.id,
                                            star_rect,
                                            egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            egui::Color32::WHITE,
                                        );
                                    }
                                } else {
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
                    let rgb = player.color;
                    let vibrant_color = egui::Color32::from_rgb(
                        (rgb[0] * 255.0).clamp(0.0, 255.0) as u8,
                        (rgb[1] * 255.0).clamp(0.0, 255.0) as u8,
                        (rgb[2] * 255.0).clamp(0.0, 255.0) as u8,
                    );

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
 
                    let mut cached_name = None;
                    let mut cached_troops = None;
 
                    if let Some(entry) = ui.nameplate_galleys.get(&player.id) {
                        let name_matches = if player.name.is_empty() {
                            entry.0.starts_with("Player ")
                                && entry.0["Player ".len()..].parse::<u16>().ok() == Some(player.id)
                        } else {
                            entry.0 == player.name
                        };
 
                        if name_matches && entry.1 == troops_str && entry.2 == font_id {
                            cached_name = Some(entry.3.clone());
                            cached_troops = Some(entry.4.clone());
                        }
                    }
 
                    let (name_galley, troops_galley) = if let (Some(ng), Some(tg)) =
                        (cached_name, cached_troops)
                    {
                        (ng, tg)
                    } else {
                        let display_name = if player.name.is_empty() {
                            format!("Player {}", player.id)
                        } else {
                            player.name.clone()
                        };
 
                        let ng = layout_nameplate_name_galley(painter, font_id.clone(), &display_name);
 
                        let troops_font_size = font_size * 1.30;
                        let troops_font_id = egui::FontId::proportional(troops_font_size);
                        let tg = crate::hud::nameplate::layout_nameplate_troops_galley(
                            painter,
                            troops_font_id,
                            &troops_str,
                        );
 
                        ui.nameplate_galleys.insert(
                            player.id,
                            (
                                display_name,
                                troops_str,
                                font_id.clone(),
                                ng.clone(),
                                tg.clone(),
                            ),
                        );
 
                        (ng, tg)
                    };

                    let right_w = name_galley.rect.width().max(troops_galley.rect.width());
                    let item_spacing_y = (font_size * 0.111).round();
                    let right_h =
                        name_galley.rect.height() + item_spacing_y + troops_galley.rect.height();

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
                        let star_uri = "bytes://star.svg";
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
                    let avatar_center = egui::pos2(
                        cur_x + avatar_size / 2.0,
                        row12_y + total_h / 2.0,
                    );
                    let avatar_r = avatar_size / 2.0;
                    if player.player_type == sow_core::player::PlayerType::Nation {
                        paint_circular_avatar(
                            painter, avatar_center, avatar_r,
                            None, vibrant_color, vibrant_color,
                        );
                    } else {
                        let avatar_tex = ui.app.asset_loader.avatars.get(&player.leader).or(ui
                            .app
                            .asset_loader
                            .avatar_fallback
                            .as_ref());
                        let tex_id = avatar_tex.map(|t| t.id());
                        paint_circular_avatar(
                            painter, avatar_center, avatar_r,
                            tex_id, vibrant_color, vibrant_color,
                        );
                    }
                    cur_x += avatar_size + spacing_x;

                    // 2. Nickname and Troops centered in right block
                    let right_y = row12_y + (total_h - right_h) / 2.0;

                    let name_x = cur_x + (right_w - name_galley.rect.width()) / 2.0;
                    crate::hud::nameplate::paint_glow_nameplate_galley(
                        &painter,
                        egui::pos2(name_x, right_y),
                        name_galley.clone(),
                        vibrant_color,
                        false,
                    );

                    let troops_x = cur_x + (right_w - troops_galley.rect.width()) / 2.0;
                    crate::hud::nameplate::paint_glow_nameplate_galley(
                        &painter,
                        egui::pos2(
                            troops_x,
                            right_y + name_galley.rect.height() + item_spacing_y,
                        ),
                        troops_galley,
                        vibrant_color,
                        false,
                    );
                    continue;
                } else {
                    // High-performance dot fallback for human players who are zoomed out or exceed budget
                    painter.circle_filled(center, dot_r, pc);
                    painter.circle_stroke(
                        center,
                        dot_r,
                        egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(180)),
                    );
                    continue;
                }
            }

            // Show nameplate if the player's territory is large enough at this zoom.
            // Uses tile count directly (normalized to a 200×200 reference map) so
            // visibility is purely size-based — no arbitrary sorting artifacts.
            let map_area = (sim.map_w * sim.map_h).max(1) as f32;
            let normalized_tiles = player.tile_count as f32 * (40_000.0 / map_area);
            let zoom_scaled_local = input.camera_zoom / sf;
            let min_tiles = if player.id >= 200 { 8.0 } else { 2.0 };
            let show_full = (normalized_tiles * zoom_scaled_local) >= min_tiles
                && full_labels_drawn < 100;

            if show_full {
                full_labels_drawn += 1;

                let font_size = if Some(player.id) == sim.my_player_id {
                    font_size_my
                } else if player.id < 200 {
                    font_size_nation
                } else {
                    font_size_tribe
                };

                let font_id = egui::FontId::proportional(font_size);

                let troops_str = sow_ui::utils::format_number(player.troops);

                let mut cached_name = None;
                let mut cached_troops = None;

                if let Some(entry) = ui.nameplate_galleys.get(&player.id) {
                    let name_matches = if player.name.is_empty() {
                        if player.id >= 200 {
                            entry.0.starts_with("Tribe ")
                                && entry.0["Tribe ".len()..].parse::<u16>().ok()
                                    == Some(player.id - 199)
                        } else {
                            entry.0.starts_with("Nation ")
                                && entry.0["Nation ".len()..].parse::<u16>().ok()
                                    == Some(player.id - 103)
                        }
                    } else {
                        entry.0 == player.name
                    };

                    if name_matches && entry.1 == troops_str && entry.2 == font_id {
                        cached_name = Some(entry.3.clone());
                        cached_troops = Some(entry.4.clone());
                    }
                }

                let (name_galley, troops_galley) = if let (Some(ng), Some(tg)) =
                    (cached_name, cached_troops)
                {
                    (ng, tg)
                } else {
                    let display_name = if player.name.is_empty() {
                        if player.id >= 200 {
                            format!("Tribe {}", player.id - 199)
                        } else {
                            format!("Nation {}", player.id - 103)
                        }
                    } else {
                        player.name.clone()
                    };

                    let ng = layout_nameplate_name_galley(painter, font_id.clone(), &display_name);

                    let tg = crate::hud::nameplate::layout_nameplate_troops_galley(
                        painter,
                        font_id.clone(),
                        &troops_str,
                    );

                    ui.nameplate_galleys.insert(
                        player.id,
                        (
                            display_name,
                            troops_str,
                            font_id.clone(),
                            ng.clone(),
                            tg.clone(),
                        ),
                    );

                    (ng, tg)
                };

                let disc_font_id = egui::FontId::proportional(
                    font_size * visual_config.nameplate_disconnected_emoji_scale,
                );

                let is_disconnected = player.disconnected;
                let mut betrayal_flash = false;

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
                            if timer <= 600 && !has_pending_proposal {
                                is_heart_flashing = true;
                            }
                        } else if me.alliance_requests.contains(&player.id) {
                            has_req = true;
                        }
                    }
                }

                if !is_disconnected {
                    let has_betrayal = player.active_emoji.as_deref() == Some("🗡️");
                    if has_betrayal {
                        betrayal_flash = true;
                    }
                }

                let mut job = egui::text::LayoutJob {
                    break_on_newline: false,
                    ..Default::default()
                };

                if is_disconnected {
                    job.append(
                        "🔌",
                        0.0,
                        egui::text::TextFormat::simple(
                            disc_font_id.clone(),
                            egui::Color32::from_rgb(239, 68, 68),
                        ),
                    );
                }

                // Retrieve or save persistent client-side state for the floating emoji
                let last_emoji_id = egui::Id::new(("last_active_emoji", player.id));
                let mut current_emoji = player.active_emoji.clone();
                let is_active = current_emoji.is_some() && current_emoji.as_deref() != Some("🗡️");

                let active_anim_id = egui::Id::new(("emoji_anim_progress", player.id));
                let anim_progress =
                    painter
                        .ctx()
                        .animate_bool_with_time(active_anim_id, is_active, 0.25);

                if current_emoji.is_none() || current_emoji.as_deref() == Some("🗡️") {
                    if anim_progress > 0.01 {
                        current_emoji = painter.ctx().data(|d| d.get_temp::<String>(last_emoji_id));
                    }
                } else {
                    painter
                        .ctx()
                        .data_mut(|d| d.insert_temp(last_emoji_id, current_emoji.clone().unwrap()));
                }

                let disc_galley = if is_disconnected {
                    Some(painter.layout_job(job))
                } else {
                    None
                };

                let star_size = if is_me {
                    name_galley.rect.height() * 1.35
                } else {
                    0.0
                };
                let h_max = name_galley.rect.height().max(star_size);
                let h = h_max + troops_galley.rect.height() + 2.0;

                let mut current_y = center.y - h / 2.0;

                let total_name_w = if is_me {
                    name_galley.rect.width() + 6.0 + star_size
                } else {
                    name_galley.rect.width()
                };

                let name_pos_start = egui::pos2(center.x - total_name_w / 2.0, current_y);

                let name_pos = if is_me {
                    egui::pos2(
                        name_pos_start.x + star_size + 6.0,
                        current_y + (h_max - name_galley.rect.height()) / 2.0,
                    )
                } else {
                    name_pos_start
                };

                let disc_height = if let Some(dg) = &disc_galley {
                    // Draw the small status icons ABOVE the nameplate, centered horizontally!
                    let disc_pos = egui::pos2(
                        center.x - dg.rect.width() / 2.0,
                        current_y - dg.rect.height() - 4.0,
                    );
                    crate::hud::nameplate::paint_nameplate_galley(painter, disc_pos, dg.clone());
                    dg.rect.height() + 4.0
                } else {
                    0.0
                };

                // Request WebP Icon Animation (Spring Overshoot)
                let request_anim_id = egui::Id::new(("request_anim_progress", player.id));
                let req_anim = painter
                    .ctx()
                    .animate_bool_with_time(request_anim_id, has_req, 0.25);

                let mut req_height = 0.0_f32;
                if req_anim > 0.01 {
                    static REGISTER_REQUEST_ONCE: std::sync::Once = std::sync::Once::new();
                    REGISTER_REQUEST_ONCE.call_once(|| {
                        painter.ctx().include_bytes(
                            "bytes://request.webp",
                            include_bytes!("../../../assets/request.webp").as_slice(),
                        );
                    });

                    let request_icon_size = font_size * 1.5 * 2.0;
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
                        // Draw it centered horizontally, above the other status icons!
                        let req_y = current_y - disc_height - size / 2.0 - 4.0;
                        let req_rect = egui::Rect::from_center_size(
                            egui::pos2(center.x, req_y),
                            egui::vec2(size, size),
                        );

                        if is_me {
                            let glow_r = size * 0.8;
                            let glow_a = req_anim * 0.35;
                            painter.circle_filled(
                                req_rect.center(),
                                glow_r * 1.4,
                                egui::Color32::from_rgba_unmultiplied(
                                    34,
                                    211,
                                    238,
                                    (glow_a * 120.0) as u8,
                                ),
                            );
                            painter.circle_filled(
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
                        painter.image(
                            texture.id,
                            req_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE.linear_multiply(req_anim),
                        );
                        req_height = size + 4.0;
                    }
                }

                // Allied WebP Icon Animation (Spring Overshoot)
                let allied_anim_id = egui::Id::new(("allied_anim_progress", player.id));
                let allied_anim =
                    painter
                        .ctx()
                        .animate_bool_with_time(allied_anim_id, is_allied, 0.25);

                let mut allied_height = 0.0_f32;
                if allied_anim > 0.01 {
                    static REGISTER_HANDSHAKE_ONCE: std::sync::Once = std::sync::Once::new();
                    REGISTER_HANDSHAKE_ONCE.call_once(|| {
                        painter.ctx().include_bytes(
                            "bytes://handshake.webp",
                            include_bytes!("../../../assets/handshake.webp").as_slice(),
                        );
                    });

                    let handshake_icon_size = font_size * 1.5 * 2.0;
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
                        // Draw it centered horizontally, above the other status icons!
                        let req_y = current_y - disc_height - size / 2.0 - 4.0;
                        let req_rect = egui::Rect::from_center_size(
                            egui::pos2(center.x, req_y),
                            egui::vec2(size, size),
                        );

                        let flash_alpha = if is_heart_flashing && !has_req {
                            heart_flash_alpha
                        } else {
                            1.0
                        };

                        if is_me {
                            let glow_r = size * 0.8;
                            let glow_a = allied_anim * flash_alpha * 0.35;
                            painter.circle_filled(
                                req_rect.center(),
                                glow_r * 1.4,
                                egui::Color32::from_rgba_unmultiplied(
                                    255,
                                    200,
                                    60,
                                    (glow_a * 120.0) as u8,
                                ),
                            );
                            painter.circle_filled(
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
                        painter.image(
                            texture.id,
                            req_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE.linear_multiply(allied_anim * flash_alpha),
                        );
                        allied_height = size + 4.0;
                    }
                }

                // Betrayal WebP Icon Animation (Spring Overshoot)
                let betrayal_anim_id = egui::Id::new(("betrayal_anim_progress", player.id));
                let betrayal_anim = painter
                    .ctx()
                    .animate_bool_with_time(betrayal_anim_id, betrayal_flash, 0.25);

                let mut betrayal_height = 0.0_f32;
                if betrayal_anim > 0.01 {
                    static REGISTER_BETRAY_ONCE: std::sync::Once = std::sync::Once::new();
                    REGISTER_BETRAY_ONCE.call_once(|| {
                        painter.ctx().include_bytes(
                            "bytes://betray.webp",
                            include_bytes!("../../../assets/betray.webp").as_slice(),
                        );
                    });

                    let betray_icon_size = font_size * 1.5 * 2.0;
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
                            if t >= 1.0 { 1.0 } else { spring_overshoot(t) }
                        } else {
                            betrayal_anim
                        };
                        let size = betray_icon_size * anim_scale;
                        let betray_y = current_y - disc_height - size / 2.0 - 4.0;
                        let betray_rect = egui::Rect::from_center_size(
                            egui::pos2(center.x, betray_y),
                            egui::vec2(size, size),
                        );

                        // Red danger glow
                        let glow_r = size * 0.8;
                        let glow_a = betrayal_anim * 0.4;
                        painter.circle_filled(
                            betray_rect.center(),
                            glow_r * 1.4,
                            egui::Color32::from_rgba_unmultiplied(
                                220, 38, 38,
                                (glow_a * 120.0) as u8,
                            ),
                        );
                        painter.circle_filled(
                            betray_rect.center(),
                            glow_r,
                            egui::Color32::from_rgba_unmultiplied(
                                220, 38, 38,
                                (glow_a * 255.0) as u8,
                            ),
                        );

                        painter.image(
                            texture.id,
                            betray_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE.linear_multiply(betrayal_anim),
                        );
                        betrayal_height = size + 4.0;
                    }
                }

                // Draw the giant animated floating express emoji above the status icons!
                if anim_progress > 0.01 {
                    if let Some(emoji_str) = &current_emoji {
                        // Disney overshoot curve
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

                        let base_emoji_size = font_size * 2.2; // 220% size! Extremely visible!
                        let final_emoji_size = base_emoji_size * anim_scale;
                        let max_float_height = req_height.max(allied_height).max(betrayal_height);
                        let emoji_y =
                            current_y - disc_height - max_float_height - 18.0 * zoom_scale;

                        if final_emoji_size > 1.0 {
                            if is_me {
                                let glow_r = final_emoji_size * 0.7;
                                let glow_a = anim_progress * 0.30;
                                let emoji_center = egui::pos2(center.x, emoji_y);
                                painter.circle_filled(
                                    emoji_center,
                                    glow_r * 1.3,
                                    egui::Color32::from_rgba_unmultiplied(
                                        pc.r(),
                                        pc.g(),
                                        pc.b(),
                                        (glow_a * 100.0) as u8,
                                    ),
                                );
                                painter.circle_filled(
                                    emoji_center,
                                    glow_r,
                                    egui::Color32::from_rgba_unmultiplied(
                                        pc.r(),
                                        pc.g(),
                                        pc.b(),
                                        (glow_a * 255.0) as u8,
                                    ),
                                );
                            }
                            if emoji_str.contains('⭐') {
                                let star_size = final_emoji_size * 1.25;
                                let star_rect = egui::Rect::from_center_size(
                                    egui::pos2(center.x, emoji_y),
                                    egui::vec2(star_size, star_size),
                                );
                                let size_hint = egui::load::SizeHint::Size {
                                    width: 128,
                                    height: 128,
                                    maintain_aspect_ratio: true,
                                };
                                let load_res = painter.ctx().try_load_texture(
                                    "bytes://star.svg",
                                    egui::TextureOptions::default(),
                                    size_hint,
                                );
                                if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
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
                            } else {
                                let emoji_galley = painter.layout_no_wrap(
                                    emoji_str.clone(),
                                    egui::FontId::proportional(final_emoji_size),
                                    egui::Color32::WHITE,
                                );
                                let emoji_pos = egui::pos2(
                                    center.x - emoji_galley.size().x / 2.0,
                                    emoji_y - emoji_galley.size().y / 2.0,
                                );
                                painter.galley(emoji_pos, emoji_galley, egui::Color32::WHITE);
                            }
                        }
                    }
                }

                let is_tribe = player.id >= 200;
                let rgb = player.color;
                let final_rgb = if is_tribe {
                    // Whitewash: blend 65% white with 35% original color to make it highly visible inside
                    [
                        rgb[0] * 0.35 + 0.65,
                        rgb[1] * 0.35 + 0.65,
                        rgb[2] * 0.35 + 0.65,
                    ]
                } else {
                    rgb
                };
                let text_color = egui::Color32::from_rgb(
                    (final_rgb[0] * 255.0).clamp(0.0, 255.0) as u8,
                    (final_rgb[1] * 255.0).clamp(0.0, 255.0) as u8,
                    (final_rgb[2] * 255.0).clamp(0.0, 255.0) as u8,
                );
                crate::hud::nameplate::paint_glow_nameplate_galley(
                    painter,
                    name_pos,
                    name_galley.clone(),
                    text_color,
                    is_tribe,
                );

                if is_me {
                    let star_pos =
                        egui::pos2(name_pos_start.x, current_y + (h_max - star_size) / 2.0);
                    let star_rect =
                        egui::Rect::from_min_size(star_pos, egui::vec2(star_size, star_size));
                    let star_uri = "bytes://star.svg";
                    let size_hint = egui::load::SizeHint::Size {
                        width: 128,
                        height: 128,
                        maintain_aspect_ratio: true,
                    };

                    let load_res = painter.ctx().try_load_texture(
                        star_uri,
                        egui::TextureOptions::default(),
                        size_hint,
                    );

                    if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                        painter.image(
                            texture.id,
                            star_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                }

                current_y += h_max + 0.5;

                // 5. Troops Image instead of emoji for non-human players
                let total_troops_w = troops_galley.rect.width();
                let troops_start_x = center.x - total_troops_w / 2.0;

                let troops_pos = egui::pos2(troops_start_x, current_y);
                let is_tribe = player.id >= 200;
                crate::hud::nameplate::paint_glow_nameplate_galley(
                    painter,
                    troops_pos,
                    troops_galley,
                    text_color,
                    is_tribe,
                );
            } else {
                // Dot only — zero text layout, bare metal fast
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
