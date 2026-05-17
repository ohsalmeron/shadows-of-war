

use crate::NAMEPLATE_REFERENCE_ZOOM;
use crate::hud::nameplate::*;
use crate::config::ClientVisualConfig;

use crate::app::SowApp;



impl SowApp {
    pub(crate) fn render_world_overlays(&mut self, ctx: &egui::Context, sf: f32) {
                                    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("world_overlays")));
                                    let wall_secs = self.time.start_time.elapsed().as_secs_f64();

                                    // Configuration variables removed from GameConfig
                                    let dot_r = ClientVisualConfig::default().ui_lod_dot_radius;
                                    
                                    struct VisPlayer<'a> {
                                        player: &'a sow_core::protocol::PlayerSnapshot,
                                        center: egui::Pos2,
                                        pc: egui::Color32,
                                        sizing_presence: f32,
                                        lod_presence: f32,
                                    }
                                    let mut visible_players = Vec::new();
                                    if let Some(snap) = &self.sim.current_snapshot {
                                        for player in &snap.players {
                                            if player.tile_count == 0 || !player.alive { continue; }
                                            
                                            let avg_col = player.centroid_x;
                                            let avg_row = player.centroid_y;
                                            
                                            let target_cx = avg_col + 0.5;
                                            let target_cy = avg_row + 0.5;
                                            
                                            // Smooth position interpolation
                                            let pos = self.ui.label_positions.entry(player.id).or_insert((target_cx, target_cy));
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
                                            if screen_x < -100.0 || screen_x > self.input.screen_w + 100.0 || screen_y < -100.0 || screen_y > self.input.screen_h + 100.0 { continue; }
                                            
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
                                            let sizing_presence = importance * (NAMEPLATE_REFERENCE_ZOOM / sf);

                                            visible_players.push(VisPlayer {
                                                player, center, pc, sizing_presence, lod_presence
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
                                        
                                        b.lod_presence.partial_cmp(&a.lod_presence).unwrap_or(std::cmp::Ordering::Equal)
                                    });

                                    let mut full_labels_drawn = 0;

                                    for vp in visible_players {
                                        let player = vp.player;
                                        let center = vp.center;
                                        let pc = vp.pc;
                                        let _sizing_presence = vp.sizing_presence;
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
                                            let ui_text_scale = ClientVisualConfig::default().ui_text_scale;

                                            // 1. Bounding box for font fitting (uses current zoom so text scales!)
                                            let empire_width_px = lod_presence * 1.0; // Hexagons spread out
                                            let empire_height_px = lod_presence * 1.0;

                                            // 2. Constrain font size so the text fits INSIDE those pixels
                                            let name_len = player.name.len().max(1) as f32;
                                            let max_by_width = empire_width_px / (name_len * 0.25); // Avg char width is ~60% of height
                                            let max_by_height = empire_height_px / 2.0; // Need space for 2 lines of text (name + troops)

                                            // 3. Raw font size that inscribes the territory at reference zoom
                                            let raw_font_size = max_by_width.min(max_by_height);

                                            // 4. Integer pt sizes → stable galley cache, stable atlas entries
                                            let target_font_size = raw_font_size * ui_text_scale;
                                            
                                            // --- Dynamic minimum font sizes based on player type ---
                                            let min_font_size = if Some(player.id) == self.sim.my_player_id {
                                                12 // My own player (stays most visible)
                                            } else if player.id < 200 {
                                                8 // AI Nations (medium visibility)
                                            } else {
                                                6 // Tribes (fades into the background when zooming out)
                                            };
                                            // -----------------------------------------------------------------------------------
                                            
                                            // Quantize to 2pt steps so float jitter does not rebuild galleys every frame.
                                            let font_size = (((target_font_size.round() as i32).clamp(min_font_size, 100) + 1) / 2 * 2) as f32;

                                            let is_human = player.player_type == sow_core::player::PlayerType::Human;
                                            let troops_for_label = self.ui.troop_label_throttle
                                                .displayed_troops(wall_secs, player.id, player.troops);
                                            let new_troops_str = render_troops(troops_for_label);
                                            
                                            let display_name = if player.name.is_empty() {
                                                if player.id >= 200 { format!("Tribe {}", player.id - 199) } 
                                                else { format!("Nation {}", player.id - 103) }
                                            } else {
                                                player.name.clone()
                                            };

                                            let cache_entry = self.ui.nameplate_cache.entry(player.id).or_insert_with(|| {
                                                let font_id = egui::FontId::proportional(font_size);
                                                let troops_str = new_troops_str.clone();
                                                
                                                CachedNameplate {
                                                    name_galley: layout_nameplate_name_galley(
                                                        &painter,
                                                        font_id.clone(),
                                                        &display_name,
                                                        is_human,
                                                        pc,
                                                    ),
                                                    troops_galley: painter.layout_no_wrap(format!("⚔ {}", troops_str), font_id, NAMEPLATE_FILL),
                                                    last_formatted_troops: troops_str,
                                                    last_font_size: font_size,
                                                }
                                            });
                                            
                                            if cache_entry.last_font_size != font_size {
                                                let font_id = egui::FontId::proportional(font_size);
                                                cache_entry.name_galley = layout_nameplate_name_galley(
                                                    &painter,
                                                    font_id.clone(),
                                                    &display_name,
                                                    is_human,
                                                    pc,
                                                );
                                                cache_entry.troops_galley = crate::hud::nameplate::layout_nameplate_troops_galley(
                                                    &painter,
                                                    font_id,
                                                    &new_troops_str,
                                                );
                                                cache_entry.last_formatted_troops = new_troops_str.clone();
                                                cache_entry.last_font_size = font_size;
                                            } else if new_troops_str != cache_entry.last_formatted_troops {
                                                let font_id = egui::FontId::proportional(font_size);
                                                cache_entry.troops_galley = crate::hud::nameplate::layout_nameplate_troops_galley(
                                                    &painter,
                                                    font_id,
                                                    &new_troops_str,
                                                );
                                                cache_entry.last_formatted_troops = new_troops_str;
                                            }
                                            
                                            let name_galley = &cache_entry.name_galley;
                                            let troops_galley = &cache_entry.troops_galley;
                                            
                                            let h = name_galley.rect.height() + troops_galley.rect.height() + 2.0;
                                            
                                            let name_pos = egui::pos2(center.x - name_galley.rect.width() / 2.0, center.y - h / 2.0);
                                            let troops_pos = egui::pos2(center.x - troops_galley.rect.width() / 2.0, center.y - h / 2.0 + name_galley.rect.height() + 2.0);
                                            crate::hud::nameplate::paint_nameplate_galley(&painter, name_pos, name_galley.clone());
                                            crate::hud::nameplate::paint_nameplate_galley(&painter, troops_pos, troops_galley.clone());
                                        } else {
                                            // Dot only — zero text layout, bare metal fast
                                            painter.circle_filled(center, dot_r, pc);
                                            painter.circle_stroke(center, dot_r, egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(180)));
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
                                            let mut r = 0.5; let mut g = 0.5; let mut b = 0.5;
                                            if let Some(owner) = snap.players.iter().find(|p| p.id == fleet.owner_id) {
                                                let rgb = if owner.player_type == sow_core::player::PlayerType::Human {
                                                    sow_core::player::human_shader_territory_rgb(owner.id)
                                                } else {
                                                    owner.color
                                                };
                                                r = rgb[0]; g = rgb[1]; b = rgb[2];
                                            }
                                            let color = egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
                                            let trail_color = egui::Color32::from_rgba_premultiplied((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 150);

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
                                                painter.add(egui::Shape::line(points, egui::Stroke::new(zoom_scaled * 0.4, trail_color)));
                                            } else if points.len() == 1 {
                                                painter.circle_filled(points[0], zoom_scaled * 0.2, trail_color);
                                            }

                                            // Render boat with smooth visual interpolation
                                            let wx_curr = (fleet.current_tile % self.sim.map_w) as f32;
                                            let wy_curr = (fleet.current_tile / self.sim.map_w) as f32;
                                            
                                            let mut wx = wx_curr;
                                            let mut wy = wy_curr;
                                            
                                            if fleet.path_cursor > 0 && !fleet.path.is_empty() {
                                                let prev_idx = fleet.path_cursor.saturating_sub(2).min(fleet.path.len().saturating_sub(1));
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
                                                egui::pos2(screen_x + zoom_scaled - margin, screen_y + zoom_scaled - margin)
                                            );
                                            
                                            painter.rect(rect, 2.0, color, egui::Stroke::new(1.5_f32, egui::Color32::from_black_alpha(200)), egui::StrokeKind::Middle);

                                            if fleet.retreating && (self.time.start_time.elapsed().as_millis() / 500).is_multiple_of(2) {
                                                let center = rect.center();
                                                painter.line_segment([egui::pos2(center.x - margin, center.y - margin), egui::pos2(center.x + margin, center.y + margin)], egui::Stroke::new(2.0_f32, egui::Color32::BLACK));
                                                painter.line_segment([egui::pos2(center.x + margin, center.y - margin), egui::pos2(center.x - margin, center.y + margin)], egui::Stroke::new(2.0_f32, egui::Color32::BLACK));
                                            }
                                        }
                                        
                                        for attack in &snap.attacks {
                                            if attack.target_owner == 0 { continue; }
                                            if attack.owner_id != self.sim.my_player_id.unwrap_or(0) { continue; }
                                            
                                            let mut rx = 0.5; let mut ry = 0.5;
                                            let mut tx = 0.5; let mut ty = 0.5;
                                            let mut r = 0.5; let mut g = 0.5; let mut b = 0.5;
                                            
                                            if let Some(attacker) = snap.players.iter().find(|p| p.id == attack.owner_id) {
                                                rx = attacker.centroid_x + 0.5;
                                                ry = attacker.centroid_y + 0.5;
                                                let rgb = if attacker.player_type == sow_core::player::PlayerType::Human {
                                                    sow_core::player::human_shader_territory_rgb(attacker.id)
                                                } else {
                                                    attacker.color
                                                };
                                                r = rgb[0]; g = rgb[1]; b = rgb[2];
                                            }
                                            if let Some(target) = snap.players.iter().find(|p| p.id == attack.target_owner) {
                                                tx = target.centroid_x + 0.5;
                                                ty = target.centroid_y + 0.5;
                                            }
                                            
                                            let start_x = (self.input.camera_x + rx * self.input.camera_zoom) / sf;
                                            let start_y = (self.input.camera_y + ry * self.input.camera_zoom) / sf;
                                            let end_x = (self.input.camera_x + tx * self.input.camera_zoom) / sf;
                                            let end_y = (self.input.camera_y + ty * self.input.camera_zoom) / sf;
                                            
                                            let color = egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
                                            let start_pos = egui::pos2(start_x, start_y);
                                            let end_pos = egui::pos2(end_x, end_y);
                                            
                                            // Simple thick line to represent attack
                                            painter.line_segment([start_pos, end_pos], egui::Stroke::new(3.0_f32, egui::Color32::from_black_alpha(150)));
                                            painter.line_segment([start_pos, end_pos], egui::Stroke::new(1.5_f32, color));
                                            
                                            if attack.retreating && (self.time.start_time.elapsed().as_millis() / 500).is_multiple_of(2) {
                                                let center = start_pos.lerp(end_pos, 0.5);
                                                painter.text(center, egui::Align2::CENTER_CENTER, "[X]", egui::FontId::proportional(20.0), egui::Color32::RED);
                                            }
                                        }
                                    }

    }
}
