use crate::config::ClientVisualConfig;
use crate::hud::nameplate::*;

use crate::app::SowApp;

impl SowApp {
    pub(crate) fn render_world_overlays(&mut self, ctx: &egui::Context, sf: f32) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("world_overlays"),
        ));
        let wall_secs = self.time.start_time.elapsed().as_secs_f64();
        let current_tick = self.sim.current_snapshot.as_ref().map(|s| s.tick).unwrap_or(0);

        // Configuration variables removed from GameConfig
        let dot_r = ClientVisualConfig::default().ui_lod_dot_radius;

        struct VisPlayer<'a> {
            player: &'a sow_core::protocol::PlayerSnapshot,
            center: egui::Pos2,
            pc: egui::Color32,
            lod_presence: f32,
        }
        let mut visible_players = Vec::new();
        if let Some(snap) = &self.sim.current_snapshot {
            for player in &snap.players {
                if player.tile_count == 0 || !player.alive {
                    continue;
                }

                let avg_col = player.centroid_x;
                let avg_row = player.centroid_y;

                let target_cx = avg_col + 0.5;
                let target_cy = avg_row + 0.5;

                // Smooth position interpolation
                let pos = self
                    .ui
                    .label_positions
                    .entry(player.id)
                    .or_insert((target_cx, target_cy));
                let dx = target_cx - pos.0;
                let dy = target_cy - pos.1;
                let dist = (dx * dx + dy * dy).sqrt();
                let dt = self.ui.raw_input.predicted_dt;
                let smooth_factor = 1.0 - (-10.0 * dt).exp(); // Frame-rate independent
                if dist > 50.0 {
                    pos.0 = target_cx;
                    pos.1 = target_cy;
                } else if dist > 0.1 {
                    pos.0 += dx * smooth_factor;
                    pos.1 += dy * smooth_factor;
                } else {
                    pos.0 = target_cx;
                    pos.1 = target_cy;
                }

                let screen_x = (pos.0 * self.input.camera_zoom + self.input.camera_x) / sf;
                let screen_y = (pos.1 * self.input.camera_zoom + self.input.camera_y) / sf;

                // Frustum cull
                if screen_x < -100.0
                    || screen_x > self.input.screen_w + 100.0
                    || screen_y < -100.0
                    || screen_y > self.input.screen_h + 100.0
                {
                    continue;
                }

                let center = egui::pos2(screen_x, screen_y);
                // Map shader derives human tint from id, not `player.color`; match that for dots + ★.
                let rgb = if player.player_type == sow_core::player::PlayerType::Human {
                    sow_core::player::human_shader_territory_rgb(player.id)
                } else {
                    player.color
                };
                let pc = nameplate_matte_player_rgb(rgb);

                // `lod_presence` uses zoom (when zoomed out, dots only). `sizing_presence`
                // does not, so nameplate font sizes stay stable and egui's glyph atlas is not
                // invalidated every scroll step (fixes garbled glyphs). Font size is rounded
                // to whole points for fewer distinct `FontId`s.
                // Normalize tile count so text size is consistent regardless of total map size.
                // 40_000 is a reference 200x200 map.
                let map_area = (self.sim.map_w * self.sim.map_h).max(1) as f32;
                let normalized_tiles = player.tile_count as f32 * (40_000.0 / map_area);
                let importance = (normalized_tiles * 0.35).max(0.15);

                let lod_presence = importance * (self.input.camera_zoom / sf);

                visible_players.push(VisPlayer {
                    player,
                    center,
                    pc,
                    lod_presence,
                });
            }
        }

        visible_players.sort_unstable_by(|a, b| {
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

        let visual_config = ClientVisualConfig::default();
        let ui_text_scale = visual_config.ui_text_scale;
        let zoom_scale = (self.input.camera_zoom / sf).min(1.0).max(0.1);

        // Precompute scaled nameplate font sizes once per frame for 100% CPU/memory efficiency!
        // Round to whole point sizes to prevent egui glyph atlas invalidations.
        let font_size_my = ((visual_config.nameplate_my_size * ui_text_scale * zoom_scale).round()).max(4.0);
        let font_size_nation = ((visual_config.nameplate_nation_size * ui_text_scale * zoom_scale).round()).max(4.0);
        let font_size_tribe = ((visual_config.nameplate_tribe_size * ui_text_scale * zoom_scale).round()).max(4.0);

        for vp in visible_players {
            let player = vp.player;
            let center = vp.center;
            let pc = vp.pc;
            let lod_presence = vp.lod_presence;

            // Small nations require zooming in to appear.
            let threshold = if player.id >= 200 {
                1.00 // Tribes need to be much closer/bigger to show text
            } else {
                0.5 // Nations can show text further away
            };
            let show_full = lod_presence >= threshold && full_labels_drawn < 100;

            if show_full {
                full_labels_drawn += 1;

                let font_size = if Some(player.id) == self.sim.my_player_id {
                    font_size_my
                } else if player.id < 200 {
                    font_size_nation
                } else {
                    font_size_tribe
                };

                let troops_for_label = self.ui.troop_label_throttle.displayed_troops(
                    current_tick,
                    player.id,
                    player.troops,
                );
                let new_troops_str = sow_ui::utils::format_number(troops_for_label);

                let display_name = if player.name.is_empty() {
                    if player.id >= 200 {
                        format!("Tribe {}", player.id - 199)
                    } else {
                        format!("Nation {}", player.id - 103)
                    }
                } else {
                    player.name.clone()
                };

                let font_id = egui::FontId::proportional(font_size);
                
                let name_galley = layout_nameplate_name_galley(
                    &painter,
                    font_id.clone(),
                    &display_name,
                );
                
                let troops_galley = crate::hud::nameplate::layout_nameplate_troops_galley(
                    &painter,
                    font_id,
                    &new_troops_str,
                );

                let disc_font_id = egui::FontId::proportional(font_size * visual_config.nameplate_disconnected_emoji_scale);
                
                let mut status_list = Vec::new();
                let mut express_emoji = None;
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
                    if let Some(my_id) = self.sim.my_player_id {
                        if my_id != player.id {
                            if let Some(me) = self.sim.current_snapshot.as_ref()
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

                    // Express track: any expressed emoji other than betrayal
                    if player.active_emoji.is_some() && player.active_emoji.as_deref() != Some("🗡️") {
                        express_emoji = player.active_emoji.as_deref();
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

                if let Some(e) = express_emoji {
                    let space = if status_list.is_empty() { "" } else { " " };
                    let express_str = format!("{}{}", space, e);
                    job.append(
                        &express_str,
                        0.0,
                        egui::text::TextFormat::simple(disc_font_id.clone(), egui::Color32::from_rgb(251, 191, 36)),
                    );
                }

                // Log emoji changes using non-spammy thread-local state tracking
                thread_local! {
                    static LAST_EMOJI_STATES: std::cell::RefCell<std::collections::HashMap<u16, Option<String>>> = std::cell::RefCell::new(std::collections::HashMap::new());
                }
                let emoji_changed = LAST_EMOJI_STATES.with(|states| {
                    let mut states = states.borrow_mut();
                    let prev = states.get(&player.id).cloned().flatten();
                    if prev != player.active_emoji {
                        states.insert(player.id, player.active_emoji.clone());
                        true
                    } else {
                        false
                    }
                });
                if emoji_changed {
                    log::info!(
                        "[EMOJI LOG] Player {} ({}) active_emoji updated in nameplate rendering: {:?}",
                        player.id,
                        display_name,
                        player.active_emoji
                    );
                }

                let disc_galley = if !status_list.is_empty() || express_emoji.is_some() || betrayal_flash {
                    Some(painter.layout_job(job))
                } else {
                    None
                };

                let h = name_galley.rect.height() + troops_galley.rect.height() + 2.0;

                let mut current_y = center.y - h / 2.0;

                let my_id = self.sim.my_player_id.unwrap_or(0);
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

                if let Some(dg) = &disc_galley {
                    // Draw the emoji ABOVE the nameplate, centered horizontally!
                    let disc_pos = egui::pos2(
                        center.x - dg.rect.width() / 2.0,
                        current_y - dg.rect.height() - 4.0,
                    );
                    crate::hud::nameplate::paint_nameplate_galley(
                        &painter,
                        disc_pos,
                        dg.clone(),
                    );
                }

                crate::hud::nameplate::paint_nameplate_galley(
                    &painter,
                    name_pos,
                    name_galley.clone(),
                );

                if is_me {
                    let star_pos = egui::pos2(
                        name_pos_start.x,
                        name_pos_start.y + 1.0,
                    );
                    let star_rect = egui::Rect::from_min_size(star_pos, egui::vec2(star_size, star_size));
                    let star_uri = "bytes://star.svg";
                    painter.ctx().include_bytes(star_uri, include_bytes!("../../assets/star.svg"));
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

                    thread_local! {
                        static LAST_SVG_STATE: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
                    }
                    let svg_state_str = match &load_res {
                        Ok(egui::load::TexturePoll::Ready { texture }) => {
                            format!("Ready(size: {:?})", texture.size)
                        }
                        Ok(egui::load::TexturePoll::Pending { size }) => {
                            format!("Pending(size: {:?})", size)
                        }
                        Err(e) => {
                            format!("Err({:?})", e)
                        }
                    };
                    let svg_changed = LAST_SVG_STATE.with(|s| {
                        let mut s = s.borrow_mut();
                        if s.as_ref() != Some(&svg_state_str) {
                            *s = Some(svg_state_str.clone());
                            true
                        } else {
                            false
                        }
                    });
                    if svg_changed {
                        log::info!("[SVG LOG] try_load_texture for star.svg state changed: {}", svg_state_str);
                    }

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

                let troops_pos = egui::pos2(
                    center.x - troops_galley.rect.width() / 2.0,
                    current_y,
                );
                crate::hud::nameplate::paint_nameplate_galley(
                    &painter,
                    troops_pos,
                    troops_galley,
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
        // --- Render Fleets ---
        if let Some(snap) = &self.sim.current_snapshot {
            let now = web_time::Instant::now();
            let sim_dt = now.duration_since(self.time.last_tick).as_secs_f32();
            let tick_dur = self.time.tick_interval.as_secs_f32().max(0.01);
            let mut t = (sim_dt / tick_dur).clamp(0.0, 1.0);
            t = t * t * (3.0 - 2.0 * t); // Smoothstep curve

            for fleet in &snap.fleets {
                let mut r = 0.5;
                let mut g = 0.5;
                let mut b = 0.5;
                if let Some(owner) = snap.players.iter().find(|p| p.id == fleet.owner_id) {
                    let rgb = if owner.player_type == sow_core::player::PlayerType::Human {
                        sow_core::player::human_shader_territory_rgb(owner.id)
                    } else {
                        owner.color
                    };
                    r = rgb[0];
                    g = rgb[1];
                    b = rgb[2];
                }
                let color = egui::Color32::from_rgb(
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                );
                let trail_color = egui::Color32::from_rgba_premultiplied(
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                    150,
                );

                // Render trail as a single line shape for massive performance boost
                let trail_len = fleet.path_cursor.min(fleet.path.len());
                let mut points = Vec::with_capacity(trail_len);
                for &tile in &fleet.path[..trail_len] {
                    let wx = (tile % self.sim.map_w) as f32;
                    let wy = (tile / self.sim.map_w) as f32;
                    // Center the points in the tile
                    let screen_x = (self.input.camera_x + (wx + 0.5) * self.input.camera_zoom) / sf;
                    let screen_y = (self.input.camera_y + (wy + 0.5) * self.input.camera_zoom) / sf;
                    points.push(egui::pos2(screen_x, screen_y));
                }
                let zoom_scaled = self.input.camera_zoom / sf;
                if points.len() > 1 {
                    painter.add(egui::Shape::line(
                        points,
                        egui::Stroke::new(zoom_scaled * 0.4, trail_color),
                    ));
                } else if points.len() == 1 {
                    painter.circle_filled(points[0], zoom_scaled * 0.2, trail_color);
                }

                // Render boat with smooth visual interpolation
                let wx_curr = (fleet.current_tile % self.sim.map_w) as f32;
                let wy_curr = (fleet.current_tile / self.sim.map_w) as f32;

                let mut wx = wx_curr;
                let mut wy = wy_curr;

                if fleet.path_cursor > 0 && !fleet.path.is_empty() {
                    let prev_idx = fleet
                        .path_cursor
                        .saturating_sub(2)
                        .min(fleet.path.len().saturating_sub(1));
                    let prev_tile = fleet.path[prev_idx];
                    let wx_prev = (prev_tile % self.sim.map_w) as f32;
                    let wy_prev = (prev_tile / self.sim.map_w) as f32;

                    wx = wx_prev + (wx_curr - wx_prev) * t;
                    wy = wy_prev + (wy_curr - wy_prev) * t;
                }

                let screen_x = (self.input.camera_x + wx * self.input.camera_zoom) / sf;
                let screen_y = (self.input.camera_y + wy * self.input.camera_zoom) / sf;
                let zoom_scaled = self.input.camera_zoom / sf;

                let margin = zoom_scaled * 0.15;
                let rect = egui::Rect::from_min_max(
                    egui::pos2(screen_x + margin, screen_y + margin),
                    egui::pos2(
                        screen_x + zoom_scaled - margin,
                        screen_y + zoom_scaled - margin,
                    ),
                );

                let uri = match fleet.unit_type {
                    sow_core::game::UnitType::TransportShip => "bytes://transport_ship.svg",
                    sow_core::game::UnitType::TradeShip => "bytes://trade_ship.svg",
                    sow_core::game::UnitType::Warship => "bytes://battleship.svg",
                };

                let tex_id = match painter.ctx().try_load_texture(
                    uri,
                    egui::TextureOptions::LINEAR,
                    egui::SizeHint::Scale(2.0.into()),
                ) {
                    Ok(egui::load::TexturePoll::Ready { texture }) => texture.id,
                    _ => egui::TextureId::default(),
                };

                let mut mesh = egui::Mesh::with_texture(tex_id);
                mesh.add_rect_with_uv(
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    color,
                );
                painter.add(mesh);

                if self.input.selected_warships.contains(&fleet.id) {
                    painter.rect_stroke(
                        rect.expand(2.0),
                        0.0,
                        egui::Stroke::new(2.0_f32, egui::Color32::YELLOW),
                        egui::StrokeKind::Middle,
                    );
                }

                if fleet.retreating
                    && (self.time.start_time.elapsed().as_millis() / 500).is_multiple_of(2)
                {
                    let center = rect.center();
                    painter.line_segment(
                        [
                            egui::pos2(center.x - margin, center.y - margin),
                            egui::pos2(center.x + margin, center.y + margin),
                        ],
                        egui::Stroke::new(2.0_f32, egui::Color32::BLACK),
                    );
                    painter.line_segment(
                        [
                            egui::pos2(center.x + margin, center.y - margin),
                            egui::pos2(center.x - margin, center.y + margin),
                        ],
                        egui::Stroke::new(2.0_f32, egui::Color32::BLACK),
                    );
                }
            }

            // --- Render Buildings ---
            let zoom_scaled = self.input.camera_zoom / sf;
            
            // Build O(1) player color lookup map to avoid O(N * M) nested linear scans
            let max_pid = snap.players.iter().map(|p| p.id).max().unwrap_or(0) as usize;
            let mut player_colors = vec![egui::Color32::GRAY; max_pid + 1];
            for p in &snap.players {
                let id = p.id as usize;
                let rgb = if p.player_type == sow_core::player::PlayerType::Human {
                    sow_core::player::human_shader_territory_rgb(p.id)
                } else {
                    p.color
                };
                player_colors[id] = egui::Color32::from_rgb(
                    (rgb[0] * 255.0) as u8,
                    (rgb[1] * 255.0) as u8,
                    (rgb[2] * 255.0) as u8,
                );
            }

            for b in &snap.buildings {
                if zoom_scaled < 0.25 {
                    // Zoomed out too far - don't render buildings at all for maximum FPS
                    continue;
                }

                let bx = (b.tile_idx % self.sim.map_w) as f32;
                let by = (b.tile_idx / self.sim.map_w) as f32;
                let screen_x = (self.input.camera_x + (bx + 0.5) * self.input.camera_zoom) / sf;
                let screen_y = (self.input.camera_y + (by + 0.5) * self.input.camera_zoom) / sf;

                // Frustum cull
                let margin = zoom_scaled * 2.0;
                if screen_x < -margin
                    || screen_x > self.input.screen_w / sf + margin
                    || screen_y < -margin
                    || screen_y > self.input.screen_h / sf + margin
                {
                    continue;
                }

                let center = egui::pos2(screen_x, screen_y);

                // O(1) Owner color lookup
                let color = player_colors.get(b.owner_id as usize).copied().unwrap_or(egui::Color32::GRAY);

                let uri = match b.kind {
                    sow_core::game::BuildingKind::City => "bytes://city.svg",
                    sow_core::game::BuildingKind::Factory => "bytes://factory.svg",
                    sow_core::game::BuildingKind::Port => "bytes://port.svg",
                    sow_core::game::BuildingKind::DefensePost => "bytes://defense_post.svg",
                    sow_core::game::BuildingKind::SamLauncher => "bytes://sam_launcher.svg",
                    sow_core::game::BuildingKind::MissileSilo => "bytes://missile_silo.svg",
                };

                if zoom_scaled < 1.0 {
                    // Tier 1: tiny square dot
                    let dot_r = (zoom_scaled * 1.5).max(1.5);
                    painter.rect_filled(
                        egui::Rect::from_center_size(center, egui::vec2(dot_r * 2.0, dot_r * 2.0)),
                        0.0,
                        color,
                    );
                } else {
                    let base_size = if zoom_scaled < 10.0 { zoom_scaled * 2.0 } else { zoom_scaled * 1.5 }.clamp(12.0, 64.0);
                    let rect = egui::Rect::from_center_size(center, egui::vec2(base_size, base_size));
                    
                    let size_hint = egui::load::SizeHint::Size {
                        width: 64, // Fixed rasterization size for caching performance
                        height: 64,
                        maintain_aspect_ratio: true,
                    };
                    
                    let load_res = painter.ctx().try_load_texture(
                        uri,
                        egui::TextureOptions::LINEAR,
                        size_hint,
                    );

                    if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                        let tint = if b.under_construction {
                            egui::Color32::from_black_alpha(128) // Semi-transparent black if under construction
                        } else {
                            egui::Color32::BLACK
                        };
                        painter.image(
                            texture.id,
                            rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            tint,
                        );
                    }
                    
                    // Render level circle if zoomed in enough
                    if zoom_scaled >= 10.0 {
                        let is_constructing = b.under_construction;
                        let text_val = if is_constructing { "🔨".to_string() } else { b.level.to_string() };
                        let font_size = (zoom_scaled * 0.4).clamp(8.0, 12.0);
                        let bg_radius = font_size * 0.8;
                        
                        // Place circle at top right of the building
                        let bg_center = egui::pos2(center.x + base_size * 0.35, center.y - base_size * 0.35);
                        
                        painter.circle_filled(
                            bg_center,
                            bg_radius,
                            egui::Color32::WHITE,
                        );
                        painter.circle_stroke(
                            bg_center,
                            bg_radius,
                            egui::Stroke::new(1.0_f32, egui::Color32::BLACK),
                        );
                        painter.text(
                            bg_center,
                            egui::Align2::CENTER_CENTER,
                            text_val,
                            egui::FontId::proportional(font_size),
                            egui::Color32::BLACK,
                        );
                    }
                }
            }

            // --- Track and Spawn Detonations ---
            let mut new_detonations = Vec::new();
            for (id, prev_proj) in &self.ui.last_projectiles {
                if !snap.projectiles.iter().any(|p| p.id == *id) {
                    if prev_proj.progress >= 0.9 {
                        new_detonations.push((prev_proj.dst_x, prev_proj.dst_y, prev_proj.kind));
                    }
                }
            }

            // Sync last_projectiles
            self.ui.last_projectiles.clear();
            for proj in &snap.projectiles {
                self.ui.last_projectiles.insert(proj.id, proj.clone());
            }

            // Spawn active explosions and fallout zones for new detonations
            let current_time = web_time::Instant::now();
            for (dx, dy, kind) in new_detonations {
                match kind {
                    sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::AtomBomb) => {
                        self.ui.active_explosions.push(crate::app::ActiveExplosion {
                            x: dx,
                            y: dy,
                            start_time: current_time,
                            max_radius: 45.0,
                            kind: crate::app::ExplosionKind::Atom,
                        });
                        self.ui.fallout_zones.push(crate::app::FalloutZone {
                            x: dx,
                            y: dy,
                            radius: 30.0,
                            start_time: current_time,
                        });
                    }
                    sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::HydrogenBomb) => {
                        self.ui.active_explosions.push(crate::app::ActiveExplosion {
                            x: dx,
                            y: dy,
                            start_time: current_time,
                            max_radius: 120.0,
                            kind: crate::app::ExplosionKind::Hydrogen,
                        });
                        self.ui.fallout_zones.push(crate::app::FalloutZone {
                            x: dx,
                            y: dy,
                            radius: 100.0,
                            start_time: current_time,
                        });
                    }
                    sow_core::game::ProjectileKind::MIRVWarhead => {
                        self.ui.active_explosions.push(crate::app::ActiveExplosion {
                            x: dx,
                            y: dy,
                            start_time: current_time,
                            max_radius: 20.0,
                            kind: crate::app::ExplosionKind::MIRVWarhead,
                        });
                        self.ui.fallout_zones.push(crate::app::FalloutZone {
                            x: dx,
                            y: dy,
                            radius: 18.0,
                            start_time: current_time,
                        });
                    }
                    _ => {}
                }
            }

            // --- Render Fallout Zones (glowing pulsing green contaminated terrain, dashed borders, rising dust particles) ---
            self.ui.fallout_zones.retain(|fz| {
                let elapsed = current_time.duration_since(fz.start_time).as_secs_f32();
                let duration = 15.0; // Contamination duration
                if elapsed >= duration {
                    return false;
                }

                let p = elapsed / duration;
                let alpha_p = (1.0 - p).max(0.0);

                let pulse = (wall_secs * 3.0).sin() as f32 * 0.15 + 0.85;
                let base_alpha = 45.0 * alpha_p * pulse;

                let screen_x = (self.input.camera_x + (fz.x + 0.5) * self.input.camera_zoom) / sf;
                let screen_y = (self.input.camera_y + (fz.y + 0.5) * self.input.camera_zoom) / sf;
                let center = egui::pos2(screen_x, screen_y);
                let zoom = self.input.camera_zoom / sf;
                let radius = fz.radius * zoom;

                // Glowing green contaminated aura
                painter.circle_filled(
                    center,
                    radius,
                    egui::Color32::from_rgba_unmultiplied(60, 220, 90, base_alpha as u8),
                );

                // CRISP high-contrast radioactive outer border
                let border_color = egui::Color32::from_rgba_unmultiplied(100, 255, 140, (base_alpha * 2.0) as u8);
                painter.circle_stroke(
                    center,
                    radius,
                    egui::Stroke::new(1.0f32, border_color),
                );

                // Deterministic floating glowing radioactive green dust particles!
                let seed = (fz.x * 123.45 + fz.y * 678.9) as i32;
                let particle_count = (fz.radius * 0.5) as i32;
                for i in 0..particle_count {
                    let angle = ((seed + i * 37) as f32).sin() * std::f32::consts::TAU;
                    let dist_ratio = (((seed + i * 19) as f32).cos() * 0.5 + 0.5).sqrt();
                    let dist = dist_ratio * fz.radius;

                    let px = fz.x + angle.cos() * dist;
                    let py = fz.y + angle.sin() * dist;

                    let speed = 0.4 + ((seed + i * 13) as f32).sin().abs() * 0.8;
                    let drift_y = (wall_secs as f32 * speed) % 6.0;
                    let py_drifted = py - drift_y;

                    let p_screen_x = (self.input.camera_x + (px + 0.5) * self.input.camera_zoom) / sf;
                    let p_screen_y = (self.input.camera_y + (py_drifted + 0.5) * self.input.camera_zoom) / sf;

                    let particle_alpha = (base_alpha * (1.0 - drift_y / 6.0)).max(0.0) as u8;

                    painter.circle_filled(
                        egui::pos2(p_screen_x, p_screen_y),
                        (1.2 * zoom).max(1.0),
                        egui::Color32::from_rgba_unmultiplied(120, 255, 150, particle_alpha),
                    );
                }

                true
            });

            // --- Render Active Explosions (rising mushroom clouds, shockwaves) ---
            self.ui.active_explosions.retain(|exp| {
                let elapsed = current_time.duration_since(exp.start_time).as_secs_f32();
                let duration = match exp.kind {
                    crate::app::ExplosionKind::Hydrogen => 3.5,
                    crate::app::ExplosionKind::Atom => 2.2,
                    crate::app::ExplosionKind::MIRVWarhead => 1.2,
                };
                if elapsed >= duration {
                    return false;
                }

                let p = elapsed / duration;

                let screen_x = (self.input.camera_x + (exp.x + 0.5) * self.input.camera_zoom) / sf;
                let screen_y = (self.input.camera_y + (exp.y + 0.5) * self.input.camera_zoom) / sf;
                let center = egui::pos2(screen_x, screen_y);
                let zoom = self.input.camera_zoom / sf;

                // 1. Expanding Shockwave Circle
                let shockwave_max = exp.max_radius * 1.6;
                let shockwave_radius = p * shockwave_max * zoom;
                let shockwave_alpha = (1.0 - p).max(0.0);
                let shockwave_color = egui::Color32::from_rgba_unmultiplied(
                    255, 255, 255,
                    (shockwave_alpha * 190.0) as u8,
                );
                painter.circle_stroke(
                    center,
                    shockwave_radius,
                    egui::Stroke::new(1.5f32, shockwave_color),
                );

                // 2. Rising Mushroom Cloud / Fireball caps
                let cloud_scale = match exp.kind {
                    crate::app::ExplosionKind::Hydrogen => 1.0,
                    crate::app::ExplosionKind::Atom => 0.45,
                    crate::app::ExplosionKind::MIRVWarhead => 0.18,
                };

                let cap_rise = p * 45.0 * cloud_scale * zoom;
                let cap_center = egui::pos2(center.x, center.y - cap_rise);
                let cap_radius = (p * 2.0).min(1.0) * exp.max_radius * zoom;

                let smoke_alpha = ((1.0 - p) * 195.0) as u8;
                let fire_alpha = ((1.0 - p) * 240.0) as u8;
                let core_alpha = (((1.0 - p).powi(2)) * 255.0) as u8;

                // Cap layers:
                // Outer dark fire-smoke
                painter.circle_filled(
                    cap_center,
                    cap_radius,
                    egui::Color32::from_rgba_unmultiplied(225, 50, 0, smoke_alpha),
                );
                // Middle glowing orange
                painter.circle_filled(
                    cap_center,
                    cap_radius * 0.75,
                    egui::Color32::from_rgba_unmultiplied(255, 130, 0, fire_alpha),
                );
                // Inner white-hot blast core
                painter.circle_filled(
                    cap_center,
                    cap_radius * 0.45,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 190, core_alpha),
                );

                // Mushroom Stem
                let stem_w = cap_radius * 0.22;
                let stem_rect = egui::Rect::from_min_max(
                    egui::pos2(center.x - stem_w, cap_center.y),
                    egui::pos2(center.x + stem_w, center.y),
                );
                painter.rect_filled(
                    stem_rect,
                    2.0,
                    egui::Color32::from_rgba_unmultiplied(255, 90, 0, (smoke_alpha as f32 * 0.75) as u8),
                );

                true
            });

            // --- Render Projectiles (Nukes, SAM Missiles) ---
            for proj in &snap.projectiles {
                let cur_x = proj.src_x + (proj.dst_x - proj.src_x) * proj.progress;
                let cur_y = proj.src_y + (proj.dst_y - proj.src_y) * proj.progress;

                // Parabolic height for nukes (peak at progress=0.5)
                let height = 4.0 * proj.progress * (1.0 - proj.progress);

                let screen_x = (self.input.camera_x + (cur_x + 0.5) * self.input.camera_zoom) / sf;
                let screen_y = (self.input.camera_y + (cur_y + 0.5 - height * 20.0) * self.input.camera_zoom) / sf;

                // Frustum cull
                if screen_x < -50.0 || screen_x > self.input.screen_w / sf + 50.0
                    || screen_y < -50.0 || screen_y > self.input.screen_h / sf + 50.0 {
                    continue;
                }

                let is_nuke = matches!(
                    proj.kind,
                    sow_core::game::ProjectileKind::Nuke(_)
                        | sow_core::game::ProjectileKind::MIRVWarhead
                );

                let center = egui::pos2(screen_x, screen_y);

                // 1. Draw glowing flight trajectory trail curve for Flying Nukes & MIRV Warheads!
                if is_nuke {
                    let steps = 15;
                    let mut curve_points = Vec::with_capacity(steps + 1);
                    for i in 0..=steps {
                        let p = (i as f32 / steps as f32) * proj.progress;
                        let t_x = proj.src_x + (proj.dst_x - proj.src_x) * p;
                        let t_y = proj.src_y + (proj.dst_y - proj.src_y) * p;
                        let t_h = 4.0 * p * (1.0 - p);

                        let sc_x = (self.input.camera_x + (t_x + 0.5) * self.input.camera_zoom) / sf;
                        let sc_y = (self.input.camera_y + (t_y + 0.5 - t_h * 20.0) * self.input.camera_zoom) / sf;
                        curve_points.push(egui::pos2(sc_x, sc_y));
                    }

                    let trail_color = match proj.kind {
                        sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::HydrogenBomb) => {
                            egui::Color32::from_rgba_unmultiplied(255, 50, 0, 150)
                        }
                        sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::MIRV) => {
                            egui::Color32::from_rgba_unmultiplied(255, 170, 0, 150)
                        }
                        sow_core::game::ProjectileKind::MIRVWarhead => {
                            egui::Color32::from_rgba_unmultiplied(255, 140, 0, 110)
                        }
                        _ => {
                            egui::Color32::from_rgba_unmultiplied(255, 90, 0, 140)
                        }
                    };

                    for win in curve_points.windows(2) {
                        painter.line_segment(
                            [win[0], win[1]],
                            egui::Stroke::new(1.8f32, trail_color),
                        );
                    }

                    // 2. Draw glowing rocket exhaust engine flame tail at the back of the missile!
                    if curve_points.len() >= 2 {
                        let tip = curve_points[curve_points.len() - 1];
                        let prev = curve_points[curve_points.len() - 2];
                        let dir = tip - prev;
                        let dir_len = (dir.x * dir.x + dir.y * dir.y).sqrt().max(0.1);
                        let dir_norm = dir / dir_len;

                        let flame_len = match proj.kind {
                            sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::HydrogenBomb) => 14.0,
                            sow_core::game::ProjectileKind::MIRVWarhead => 6.0,
                            _ => 10.0,
                        };
                        let flame_back = tip - dir_norm * flame_len;
                        let perp = egui::vec2(-dir_norm.y, dir_norm.x) * (flame_len * 0.28);

                        let flame_left = flame_back - perp;
                        let flame_right = flame_back + perp;
                        painter.add(egui::Shape::convex_polygon(
                            vec![tip, flame_left, flame_right],
                            egui::Color32::from_rgb(255, 140, 0),
                            egui::Stroke::NONE,
                        ));

                        let core_back = tip - dir_norm * (flame_len * 0.45);
                        let core_perp = perp * 0.45;
                        painter.add(egui::Shape::convex_polygon(
                            vec![tip, core_back - core_perp, core_back + core_perp],
                            egui::Color32::from_rgb(255, 255, 200),
                            egui::Stroke::NONE,
                        ));
                    }
                }

                let uri = match proj.kind {
                    sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::AtomBomb) => "bytes://atombomb.png",
                    sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::HydrogenBomb) => "bytes://hydrogenbomb.png",
                    sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::MIRV) => "bytes://mirv.png",
                    sow_core::game::ProjectileKind::MIRVWarhead => "bytes://atombomb.png",
                    sow_core::game::ProjectileKind::SAMMissile => "bytes://sam_missile.png",
                    sow_core::game::ProjectileKind::Shell => continue,
                };

                let base_size = match proj.kind {
                    sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::HydrogenBomb) => 24.0,
                    sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::MIRV) => 20.0,
                    sow_core::game::ProjectileKind::Nuke(_) => 16.0,
                    sow_core::game::ProjectileKind::MIRVWarhead => 8.0,
                    sow_core::game::ProjectileKind::SAMMissile => 10.0,
                    _ => 12.0,
                };

                let scale = (1.0 + height * 0.5).min(2.0);
                let size = base_size * scale;
                let rect = egui::Rect::from_center_size(center, egui::vec2(size, size));

                let size_hint = egui::load::SizeHint::Size { width: 64, height: 64, maintain_aspect_ratio: true };
                if let Ok(egui::load::TexturePoll::Ready { texture }) = painter.ctx().try_load_texture(uri, egui::TextureOptions::LINEAR, size_hint) {
                    painter.image(
                        texture.id,
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }

                // SAM missile trail
                if matches!(proj.kind, sow_core::game::ProjectileKind::SAMMissile) {
                    let trail_x = (self.input.camera_x + (proj.src_x + 0.5) * self.input.camera_zoom) / sf;
                    let trail_y = (self.input.camera_y + (proj.src_y + 0.5) * self.input.camera_zoom) / sf;
                    painter.line_segment(
                        [egui::pos2(trail_x, trail_y), center],
                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(100, 200, 255, 100)),
                    );
                }
            }

            for attack in &snap.attacks {
                if attack.target_owner == 0 {
                    continue;
                }
                if attack.owner_id != self.sim.my_player_id.unwrap_or(0) {
                    continue;
                }

                let mut rx = 0.5;
                let mut ry = 0.5;
                let mut tx = 0.5;
                let mut ty = 0.5;
                let mut r = 0.5;
                let mut g = 0.5;
                let mut b = 0.5;

                if let Some(attacker) = snap.players.iter().find(|p| p.id == attack.owner_id) {
                    rx = attacker.centroid_x + 0.5;
                    ry = attacker.centroid_y + 0.5;
                    let rgb = if attacker.player_type == sow_core::player::PlayerType::Human {
                        sow_core::player::human_shader_territory_rgb(attacker.id)
                    } else {
                        attacker.color
                    };
                    r = rgb[0];
                    g = rgb[1];
                    b = rgb[2];
                }
                if let Some(target) = snap.players.iter().find(|p| p.id == attack.target_owner) {
                    tx = target.centroid_x + 0.5;
                    ty = target.centroid_y + 0.5;
                }

                let start_x = (self.input.camera_x + rx * self.input.camera_zoom) / sf;
                let start_y = (self.input.camera_y + ry * self.input.camera_zoom) / sf;
                let end_x = (self.input.camera_x + tx * self.input.camera_zoom) / sf;
                let end_y = (self.input.camera_y + ty * self.input.camera_zoom) / sf;

                let color = egui::Color32::from_rgb(
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                );
                let start_pos = egui::pos2(start_x, start_y);
                let end_pos = egui::pos2(end_x, end_y);

                // Simple thick line to represent attack
                painter.line_segment(
                    [start_pos, end_pos],
                    egui::Stroke::new(3.0_f32, egui::Color32::from_black_alpha(150)),
                );
                painter.line_segment([start_pos, end_pos], egui::Stroke::new(1.5_f32, color));

                if attack.retreating
                    && (self.time.start_time.elapsed().as_millis() / 500).is_multiple_of(2)
                {
                    let center = start_pos.lerp(end_pos, 0.5);
                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        "[X]",
                        egui::FontId::proportional(20.0),
                        egui::Color32::RED,
                    );
                }
            }

            // --- Building Placement Preview ---
            if let Some(kind) = self.ui.app.hud_state.selected_building_kind {
                let mx = self.input.last_mouse_x as f32;
                let my = self.input.last_mouse_y as f32;
                
                let world_x = (mx - self.input.camera_x) / self.input.camera_zoom;
                let world_y = (my - self.input.camera_y) / self.input.camera_zoom;
                
                let col = world_x.floor() as i32;
                let row = world_y.floor() as i32;
                
                if col >= 0 && row >= 0 && col < self.sim.map_w as i32 && row < self.sim.map_h as i32 {
                    let map_w = self.sim.map_w;
                    let map_h = self.sim.map_h;
                    let owners = self.gfx.map_renderer.as_ref().map(|mr| mr.owners.as_slice()).unwrap_or(&[]);
                    let terrain = self.gfx.map_renderer.as_ref().map(|mr| mr.terrain.as_slice()).unwrap_or(&[]);
                    let my_id = self.sim.my_player_id.unwrap_or(0);
                    let buildings = self.sim.current_snapshot.as_ref().map(|s| s.buildings.as_slice()).unwrap_or(&[]);

                    let snapped_idx = crate::input::resolve_building_placement_tile(
                        kind,
                        col,
                        row,
                        map_w,
                        map_h,
                        owners,
                        terrain,
                        my_id,
                        buildings,
                    );

                    let can_afford = {
                        let i = sow_core::game::BuildingKind::ALL.iter().position(|&k| k == kind).unwrap_or(0);
                        self.ui.app.hud_state.gold >= self.ui.app.hud_state.building_costs[i]
                    };

                    let (draw_col, draw_row, is_valid) = if let Some(idx) = snapped_idx {
                        ((idx % map_w) as i32, (idx / map_w) as i32, can_afford)
                    } else {
                        (col, row, false)
                    };
                    
                    let tile_screen_x = (self.input.camera_x + (draw_col as f32 + 0.5) * self.input.camera_zoom) / sf;
                    let tile_screen_y = (self.input.camera_y + (draw_row as f32 + 0.5) * self.input.camera_zoom) / sf;
                    
                    let fill_color = if is_valid {
                        egui::Color32::from_rgba_unmultiplied(74, 222, 128, 80) // Green
                    } else {
                        egui::Color32::from_rgba_unmultiplied(239, 68, 68, 80) // Red
                    };
                    let stroke_color = if is_valid {
                        egui::Color32::from_rgb(74, 222, 128)
                    } else {
                        egui::Color32::from_rgb(239, 68, 68)
                    };
                    
                    // Draw highlight rect
                    let tile_size = self.input.camera_zoom / sf;
                    let tile_rect = egui::Rect::from_center_size(
                        egui::pos2(tile_screen_x, tile_screen_y),
                        egui::vec2(tile_size, tile_size)
                    );
                    painter.rect(tile_rect, 0.0, fill_color, egui::Stroke::new(1.0_f32, stroke_color), egui::StrokeKind::Inside);
                    
                    // Draw ghost SVG
                    if tile_size > 12.0 {
                        let uri = match kind {
                            sow_core::game::BuildingKind::City => "bytes://city.svg",
                            sow_core::game::BuildingKind::Factory => "bytes://factory.svg",
                            sow_core::game::BuildingKind::Port => "bytes://port.svg",
                            sow_core::game::BuildingKind::DefensePost => "bytes://defense_post.svg",
                            sow_core::game::BuildingKind::SamLauncher => "bytes://sam_launcher.svg",
                            sow_core::game::BuildingKind::MissileSilo => "bytes://missile_silo.svg",
                        };
                        let base_size = if tile_size < 10.0 { tile_size * 2.0 } else { tile_size * 1.5 }.clamp(12.0, 64.0);
                        let size_hint = egui::load::SizeHint::Size { width: 64, height: 64, maintain_aspect_ratio: true };
                        if let Ok(egui::load::TexturePoll::Ready { texture }) = painter.ctx().try_load_texture(uri, egui::TextureOptions::LINEAR, size_hint) {
                            painter.image(
                                texture.id,
                                egui::Rect::from_center_size(egui::pos2(tile_screen_x, tile_screen_y), egui::vec2(base_size, base_size)),
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                stroke_color,
                            );
                        }
                    }
                }
            }
        }
    }
}
