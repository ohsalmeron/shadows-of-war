use super::*;
#[allow(unused_variables)]
pub(crate) fn render(ui: &mut crate::app::UiState, sim: &crate::app::SimState, input: &crate::app::InputState, time: &crate::app::TimeState, gfx: &crate::app::GraphicsState, ctx: &RenderContext) {
    let painter = ctx.painter;
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
            // --- Layer 8: Player Nameplates & Leader Stars (Top-most) ---
            let mut sorted_players = visible_players.to_vec();
            sorted_players.sort_unstable_by(|a, b| {
                let a_is_human = a.player.player_type == sow_core::player::PlayerType::Human;
                let b_is_human = b.player.player_type == sow_core::player::PlayerType::Human;
                if a_is_human != b_is_human {
                    return b_is_human.cmp(&a_is_human); // true > false
                }

                let a_is_nation = a.player.id < 200;
                let b_is_nation = b.player.id < 200;
                if a_is_nation != b_is_nation {
                    return b_is_nation.cmp(&a_is_nation); // true > false
                }

                b.lod_presence
                    .partial_cmp(&a.lod_presence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let mut full_labels_drawn = 0;
            let mut premium_labels_drawn = 0;

            let visual_config = ClientVisualConfig::default();
            let ui_text_scale = visual_config.ui_text_scale;
            let zoom_scale = (input.camera_zoom / sf).min(1.0).max(0.1);

            // Precompute scaled nameplate font sizes once per frame for 100% CPU/memory efficiency!
            // Round to whole point sizes to prevent egui glyph atlas invalidations.
            let font_size_my = ((visual_config.nameplate_my_size * ui_text_scale * zoom_scale).round()).max(4.0);
            let font_size_nation = ((visual_config.nameplate_nation_size * ui_text_scale * zoom_scale).round()).max(4.0);
            let font_size_tribe = ((visual_config.nameplate_tribe_size * ui_text_scale * zoom_scale).round()).max(4.0);

            for vp in &sorted_players {
                let player = vp.player;
                let center = vp.center;
                let pc = vp.pc;
                let lod_presence = vp.lod_presence;

                if player.player_type == sow_core::player::PlayerType::Human {
                    // --- LOD 3: Zoomed-out Avatar-only Gate ---
                    if zoom_scaled < 1.5 {
                        if player.id < 200 {
                            let avatar_tex = ui.app.asset_loader.avatars.get(&player.leader).or(ui.app.asset_loader.avatar_fallback.as_ref());
                            if let Some(tex) = avatar_tex {
                                let avatar_size = 24.0;
                                let avatar_rect = egui::Rect::from_center_size(center, egui::vec2(avatar_size, avatar_size));
                                painter.image(
                                    tex.id(),
                                    avatar_rect,
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    egui::Color32::WHITE,
                                );
                                painter.circle_stroke(center, avatar_size * 0.5, egui::Stroke::new(2.0_f32, pc));
                                continue;
                            }
                        }

                        // Fallback to dot for Tribes or if avatar texture is not ready
                        painter.circle_filled(center, dot_r, pc);
                        painter.circle_stroke(
                            center,
                            dot_r,
                            egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(180)),
                        );
                        continue;
                    }

                    // --- premium human player drawing ---
                    let should_draw_premium = zoom_scale >= 0.18_f32 && premium_labels_drawn < 16;
                    
                    if should_draw_premium {
                        premium_labels_drawn += 1;
                        
                        let scale_factor = (zoom_scaled / 6.0).clamp(0.4, 1.0);
                        let avatar_size = 36.0 * scale_factor; // Increased size of avatar
                        let font_size = 16.0 * scale_factor;   // Increased size of nickname
                        let stroke_width = 1.2_f32 * scale_factor;
                        let inner_margin = egui::Margin::symmetric(
                            (8.0 * scale_factor).round() as i8,
                            (6.0 * scale_factor).round() as i8,
                        );
                        let corner_radius = (8.0 * scale_factor).round() as u8;
                        let avatar_corner = (18.0 * scale_factor).round() as u8;

                        let my_id = sim.my_player_id.unwrap_or(0);
                        let is_me = player.id == my_id;

                        // Check alliance status with the player
                        let mut is_allied = false;
                        let mut is_heart_flashing = false;
                        let mut has_req = false;
                        if my_id != player.id {
                            if let Some(me) = sim.current_snapshot.as_ref()
                                .and_then(|s| s.players.iter().find(|p| p.id == my_id))
                            {
                                if me.alliances.contains(&player.id) {
                                    is_allied = true;
                                    let timer = me.alliance_timers.get(&player.id).copied().unwrap_or(2400);
                                    if timer <= 600 {
                                        is_heart_flashing = true;
                                    }
                                } else if me.alliance_requests.contains(&player.id) {
                                    has_req = true;
                                }
                            }
                        }
                        let show_handshake = is_allied || has_req;

                        let area_id = egui::Id::new(("nameplate_player", player.id));
                        egui::Area::new(area_id)
                            .fixed_pos(center)
                            .pivot(egui::Align2::CENTER_CENTER)
                            .order(egui::Order::Foreground)
                            .show(painter.ctx(), |area_ui| {
                                let (fill_color, stroke) = if zoom_scaled >= 3.0 {
                                    // LOD 1: Plate card frame with player border
                                    (egui::Color32::TRANSPARENT, egui::Stroke::new(stroke_width, pc))
                                } else {
                                    // LOD 2: Naked horizontal layout
                                    (egui::Color32::TRANSPARENT, egui::Stroke::NONE)
                                };

                                egui::Frame::NONE
                                    .fill(fill_color)
                                    .stroke(stroke)
                                    .corner_radius(corner_radius)
                                    .inner_margin(inner_margin)
                                    .show(area_ui, |area_ui| {
                                        area_ui.horizontal(|area_ui| {
                                            area_ui.spacing_mut().item_spacing.x = 6.0 * scale_factor;

                                            // 1. Star (if me)
                                            if is_me {
                                                let star_size_icon = font_size * 1.35; // Larger star icon
                                                let star_uri = sow_core::assets::Asset::Star.uri();
                                                let size_hint = egui::load::SizeHint::Size {
                                                    width: star_size_icon.round() as u32,
                                                    height: star_size_icon.round() as u32,
                                                    maintain_aspect_ratio: true,
                                                };
                                                let load_res = area_ui.ctx().try_load_texture(
                                                    star_uri,
                                                    egui::TextureOptions::default(),
                                                    size_hint,
                                                );
                                                if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                                                    area_ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(star_size_icon, star_size_icon)));
                                                }
                                            }

                                            // 2. Handshake
                                            if show_handshake {
                                                if !ui.handshake_svg_registered {
                                                    ui.handshake_svg_registered = true;
                                                    area_ui.ctx().include_bytes(
                                                        sow_core::assets::Asset::Handshake.uri(),
                                                        include_bytes!("../../../assets/handshake.svg").as_slice(),
                                                    );
                                                }
                                                let handshake_size_icon = font_size * 1.15;
                                                let size_hint = egui::load::SizeHint::Size {
                                                    width: handshake_size_icon.round() as u32,
                                                    height: handshake_size_icon.round() as u32,
                                                    maintain_aspect_ratio: true,
                                                };
                                                let handshake_uri = sow_core::assets::Asset::Handshake.uri();
                                                let load_res = area_ui.ctx().try_load_texture(
                                                    handshake_uri,
                                                    egui::TextureOptions::default(),
                                                    size_hint,
                                                );
                                                if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                                                    let tint = if is_heart_flashing {
                                                        let t = (wall_secs * std::f64::consts::TAU * 2.0).sin() * 0.5 + 0.5;
                                                        if t > 0.5 {
                                                            egui::Color32::from_rgb(239, 68, 68)
                                                        } else {
                                                            egui::Color32::WHITE
                                                        }
                                                    } else if is_allied {
                                                        egui::Color32::from_rgb(74, 222, 128)
                                                    } else {
                                                        egui::Color32::from_rgb(34, 211, 238)
                                                    };
                                                    area_ui.add(egui::Image::new(texture)
                                                        .fit_to_exact_size(egui::vec2(handshake_size_icon, handshake_size_icon))
                                                        .tint(tint));
                                                }
                                            }

                                            // 3. Avatar
                                            let avatar_tex = ui.app.asset_loader.avatars.get(&player.leader).or(ui.app.asset_loader.avatar_fallback.as_ref());
                                            if let Some(tex) = avatar_tex {
                                                area_ui.add(egui::Image::new(tex)
                                                    .fit_to_exact_size(egui::vec2(avatar_size, avatar_size))
                                                    .corner_radius(avatar_corner));
                                            }

                                             // 4. Nickname
                                             let display_name = if player.name.is_empty() {
                                                 format!("Player {}", player.id)
                                             } else {
                                                 player.name.clone()
                                             };
                                             let font_id = egui::FontId::proportional(font_size);
                                             let name_galley = area_ui.painter().layout_no_wrap(display_name, font_id.clone(), egui::Color32::WHITE);
                                             let (rect, _resp) = area_ui.allocate_exact_size(name_galley.rect.size(), egui::Sense::hover());
                                             let rgb = if player.player_type == sow_core::player::PlayerType::Human {
                                                 sow_core::player::human_shader_territory_rgb(player.id)
                                             } else {
                                                 player.color
                                             };
                                             let vibrant_color = egui::Color32::from_rgb(
                                                 (rgb[0] * 255.0).clamp(0.0, 255.0) as u8,
                                                 (rgb[1] * 255.0).clamp(0.0, 255.0) as u8,
                                                 (rgb[2] * 255.0).clamp(0.0, 255.0) as u8,
                                             );
                                             crate::hud::nameplate::paint_glow_nameplate_galley(
                                                 area_ui.painter(),
                                                 rect.min,
                                                 name_galley,
                                                 vibrant_color, // Player custom nation color
                                                 font_id,
                                             );

                                             // 5. Troops Image instead of emoji
                                             if !ui.troops_webp_registered {
                                                 ui.troops_webp_registered = true;
                                                 area_ui.ctx().include_bytes(
                                                     sow_core::assets::Asset::Troops.uri(),
                                                     include_bytes!("../../../assets/troops.webp").as_slice(),
                                                 );
                                             }
                                             let troops_icon_size = font_size * 1.1;
                                             let load_res = area_ui.ctx().try_load_texture(
                                                 sow_core::assets::Asset::Troops.uri(),
                                                 egui::TextureOptions::default(),
                                                 egui::load::SizeHint::Size {
                                                     width: troops_icon_size.round() as u32,
                                                     height: troops_icon_size.round() as u32,
                                                     maintain_aspect_ratio: true,
                                                 },
                                             );
                                             if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                                                 area_ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(troops_icon_size, troops_icon_size)));
                                             }

                                             let formatted_troops = sow_ui::utils::format_number(player.troops);
                                             let troops_font_id = egui::FontId::proportional(font_size);
                                             let troops_galley = area_ui.painter().layout_no_wrap(formatted_troops, troops_font_id.clone(), egui::Color32::WHITE);
                                             let (rect, _resp) = area_ui.allocate_exact_size(troops_galley.rect.size(), egui::Sense::hover());
                                             crate::hud::nameplate::paint_glow_nameplate_galley(
                                                 area_ui.painter(),
                                                 rect.min,
                                                 troops_galley,
                                                 egui::Color32::from_rgb(74, 222, 128), // Vibrant neon green!
                                                 troops_font_id,
                                             );
                                         });
                                    });
                            });
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

                // Small nations require zooming in to appear.
                let threshold = if player.id >= 200 {
                    1.00 // Tribes need to be much closer/bigger to show text
                } else {
                    0.5 // Nations can show text further away
                };
                let show_full = lod_presence >= threshold && full_labels_drawn < 100;

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

                    let troops_for_label = ui.troop_label_throttle.displayed_troops(
                        current_tick,
                        player.id,
                        player.troops,
                    );

                    let mut cached_name = None;
                    let mut cached_troops = None;

                    if let Some(entry) = ui.nameplate_galleys.get(&player.id) {
                        let name_matches = if player.name.is_empty() {
                            if player.id >= 200 {
                                entry.0.starts_with("Tribe ") && entry.0["Tribe ".len()..].parse::<u16>().ok() == Some(player.id - 199)
                            } else {
                                entry.0.starts_with("Nation ") && entry.0["Nation ".len()..].parse::<u16>().ok() == Some(player.id - 103)
                            }
                        } else {
                            entry.0 == player.name
                        };

                        if name_matches && entry.1 == troops_for_label && entry.2 == font_id {
                            cached_name = Some(entry.3.clone());
                            cached_troops = Some(entry.4.clone());
                        }
                    }

                    let (name_galley, troops_galley) = if let (Some(ng), Some(tg)) = (cached_name, cached_troops) {
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

                        let new_troops_str = sow_ui::utils::format_number(troops_for_label);

                        let ng = layout_nameplate_name_galley(
                            &painter,
                            font_id.clone(),
                            &display_name,
                        );

                        let tg = crate::hud::nameplate::layout_nameplate_troops_galley(
                            &painter,
                            font_id.clone(),
                            &new_troops_str,
                        );

                        ui.nameplate_galleys.insert(
                            player.id,
                            (display_name, troops_for_label, font_id.clone(), ng.clone(), tg.clone()),
                        );

                        (ng, tg)
                    };

                    let disc_font_id = egui::FontId::proportional(font_size * visual_config.nameplate_disconnected_emoji_scale);

                    let mut status_list = Vec::new();
                    let mut betrayal_flash = false;

                    if player.disconnected {
                        status_list.push("🔌");
                    } else {
                        let has_betrayal = player.active_emoji.as_deref() == Some("🗡️");
                        if has_betrayal {
                            betrayal_flash = true;
                        }

                        // Check alliance status with the player
                        let mut is_allied = false;
                        let mut is_heart_flashing = false;
                        let mut has_req = false;
                        if let Some(my_id) = sim.my_player_id {
                            if my_id != player.id {
                                if let Some(me) = sim.current_snapshot.as_ref()
                                    .and_then(|s| s.players.iter().find(|p| p.id == my_id))
                                {
                                    if me.alliances.contains(&player.id) {
                                        is_allied = true;
                                        let timer = me.alliance_timers.get(&player.id).copied().unwrap_or(2400);
                                        if timer <= 600 {
                                            is_heart_flashing = true;
                                        }
                                    } else if me.alliance_requests.contains(&player.id) {
                                        has_req = true;
                                    }
                                }
                            }
                        }

                        if is_allied {
                            if is_heart_flashing {
                                let is_flash_red = (wall_secs * 2.0) as u64 % 2 == 0;
                                if is_flash_red {
                                    status_list.push("❤️");
                                } else {
                                    status_list.push("🤍"); // Sleek alternating white heart (0 horizontal layout shifts)
                                }
                            } else {
                                status_list.push("❤️");
                            }
                        } else if has_req {
                            status_list.push("💕");
                        }

                    }

                    let mut job = egui::text::LayoutJob {
                        break_on_newline: false,
                        ..Default::default()
                    };

                    // Betrayal emoji with 1-second fade-in/out pulse
                    if betrayal_flash {
                        let t = (wall_secs * std::f64::consts::TAU).sin() * 0.5 + 0.5; // 0..1 over 1 sec
                        let alpha = (t * 200.0 + 55.0) as u8; // range 55..255
                        let flash_color = egui::Color32::from_rgba_unmultiplied(220, 38, 38, alpha);
                        let space = if status_list.is_empty() { "" } else { " " };
                        job.append(
                            &format!("{}🗡️", space),
                            0.0,
                            egui::text::TextFormat::simple(disc_font_id.clone(), flash_color),
                        );
                    }

                    if !status_list.is_empty() {
                        let space = if betrayal_flash { " " } else { "" };
                        let status_str = format!("{}{}", space, status_list.join(" "));
                        job.append(
                            &status_str,
                            0.0,
                            egui::text::TextFormat::simple(disc_font_id.clone(), egui::Color32::from_rgb(239, 68, 68)),
                        );
                    }

                    // Retrieve or save persistent client-side state for the floating emoji
                    let last_emoji_id = egui::Id::new(("last_active_emoji", player.id));
                    let mut current_emoji = player.active_emoji.clone();
                    let is_active = current_emoji.is_some() && current_emoji.as_deref() != Some("🗡️");

                    let active_anim_id = egui::Id::new(("emoji_anim_progress", player.id));
                    let anim_progress = painter.ctx().animate_bool_with_time(active_anim_id, is_active, 0.25);

                    if current_emoji.is_none() || current_emoji.as_deref() == Some("🗡️") {
                        if anim_progress > 0.01 {
                            current_emoji = painter.ctx().data(|d| d.get_temp::<String>(last_emoji_id));
                        }
                    } else {
                        painter.ctx().data_mut(|d| d.insert_temp(last_emoji_id, current_emoji.clone().unwrap()));
                    }

                    let disc_galley = if !status_list.is_empty() || betrayal_flash {
                        Some(painter.layout_job(job))
                    } else {
                        None
                    };

                    let h = name_galley.rect.height() + troops_galley.rect.height() + 2.0;

                    let mut current_y = center.y - h / 2.0;

                    let my_id = sim.my_player_id.unwrap_or(0);
                    let is_me = player.id == my_id;

                    let star_size = name_galley.rect.height() - 2.0;
                    let total_name_w = if is_me {
                        name_galley.rect.width() + 4.0 + star_size
                    } else {
                        name_galley.rect.width()
                    };

                    let name_pos_start = egui::pos2(
                        center.x - total_name_w / 2.0,
                        current_y,
                    );

                    let name_pos = if is_me {
                        egui::pos2(name_pos_start.x + star_size + 4.0, current_y)
                    } else {
                        name_pos_start
                    };

                    let disc_height = if let Some(dg) = &disc_galley {
                        // Draw the small status icons ABOVE the nameplate, centered horizontally!
                        let disc_pos = egui::pos2(
                            center.x - dg.rect.width() / 2.0,
                            current_y - dg.rect.height() - 4.0,
                        );
                        crate::hud::nameplate::paint_nameplate_galley(
                            &painter,
                            disc_pos,
                            dg.clone(),
                        );
                        dg.rect.height() + 4.0
                    } else {
                        0.0
                    };

                    // Draw the giant animated floating express emoji above the status icons!
                    if anim_progress > 0.01 {
                        if let Some(emoji_str) = &current_emoji {
                            // Disney overshoot curve
                            let anim_scale = if is_active {
                                let t = anim_progress;
                                if t >= 1.0 {
                                    1.0
                                } else {
                                    1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
                                }
                            } else {
                                anim_progress
                            };

                            let base_emoji_size = font_size * 2.2; // 220% size! Extremely visible!
                            let final_emoji_size = base_emoji_size * anim_scale;
                            let emoji_y = current_y - disc_height - 18.0 * zoom_scale;

                            if final_emoji_size > 1.0 {
                                painter.text(
                                    egui::pos2(center.x, emoji_y),
                                    egui::Align2::CENTER_CENTER,
                                    emoji_str,
                                    egui::FontId::proportional(final_emoji_size),
                                    egui::Color32::WHITE,
                                );
                            }
                        }
                    }

                    if player.id >= 200 {
                        crate::hud::nameplate::paint_nameplate_galley(
                            &painter,
                            name_pos,
                            name_galley.clone(),
                        );
                    } else {
                        let rgb = if player.player_type == sow_core::player::PlayerType::Human {
                            sow_core::player::human_shader_territory_rgb(player.id)
                        } else {
                            player.color
                        };
                        let vibrant_color = egui::Color32::from_rgb(
                            (rgb[0] * 255.0).clamp(0.0, 255.0) as u8,
                            (rgb[1] * 255.0).clamp(0.0, 255.0) as u8,
                            (rgb[2] * 255.0).clamp(0.0, 255.0) as u8,
                        );
                        crate::hud::nameplate::paint_glow_nameplate_galley(
                            &painter,
                            name_pos,
                            name_galley.clone(),
                            vibrant_color, // Player custom nation color
                            font_id.clone(),
                        );
                    }

                    if is_me {
                        let star_pos = egui::pos2(
                            name_pos_start.x,
                            name_pos_start.y + 1.0,
                        );
                        let star_rect = egui::Rect::from_min_size(star_pos, egui::vec2(star_size, star_size));
                        let star_uri = sow_core::assets::Asset::Star.uri();
                        let size_hint = egui::load::SizeHint::Size {
                            width: star_size.round() as u32,
                            height: star_size.round() as u32,
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

                    current_y += name_galley.rect.height() + 2.0;

                    // 5. Troops Image instead of emoji for non-human players
                    if !ui.troops_webp_registered {
                        ui.troops_webp_registered = true;
                        painter.ctx().include_bytes(
                            sow_core::assets::Asset::Troops.uri(),
                            include_bytes!("../../../assets/troops.webp").as_slice(),
                        );
                    }
                    let troops_icon_size = font_size * 1.1;
                    let load_res = painter.ctx().try_load_texture(
                        sow_core::assets::Asset::Troops.uri(),
                        egui::TextureOptions::default(),
                        egui::load::SizeHint::Size {
                            width: troops_icon_size.round() as u32,
                            height: troops_icon_size.round() as u32,
                            maintain_aspect_ratio: true,
                        },
                    );

                    let total_troops_w = if let Ok(egui::load::TexturePoll::Ready { .. }) = load_res {
                        troops_icon_size + 4.0 + troops_galley.rect.width()
                    } else {
                        troops_galley.rect.width()
                    };

                    let mut troops_start_x = center.x - total_troops_w / 2.0;

                    if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                        let icon_rect = egui::Rect::from_min_size(
                            egui::pos2(troops_start_x, current_y),
                            egui::vec2(troops_icon_size, troops_icon_size),
                        );
                        painter.image(
                            texture.id,
                            icon_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                        troops_start_x += troops_icon_size + 4.0;
                    }

                    let troops_pos = egui::pos2(
                        troops_start_x,
                        current_y + (troops_icon_size - troops_galley.rect.height()) / 2.0,
                    );
                    if player.id >= 200 {
                        crate::hud::nameplate::paint_nameplate_galley(
                            &painter,
                            troops_pos,
                            troops_galley,
                        );
                    } else {
                        crate::hud::nameplate::paint_glow_nameplate_galley(
                            &painter,
                            troops_pos,
                            troops_galley,
                            egui::Color32::from_rgb(74, 222, 128), // Vibrant neon green!
                            font_id,
                        );
                    }
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
