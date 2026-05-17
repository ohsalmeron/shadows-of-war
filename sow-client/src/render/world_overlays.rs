

use crate::NAMEPLATE_REFERENCE_ZOOM;
use crate::nameplate::*;
use crate::config::ClientVisualConfig;

use crate::app::SowApp;



impl SowApp {
    pub(crate) fn render_world_overlays(&mut self, ctx: &egui::Context, sf: f32) {
                                    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("world_overlays")));
                                    let wall_secs = self.start_time.elapsed().as_secs_f64();

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
                                            let pos = self.label_positions.entry(player.id).or_insert((target_cx, target_cy));
                                            let dx = target_cx - pos.0;
                                            let dy = target_cy - pos.1;
                                            let dist = (dx * dx + dy * dy).sqrt();
                                            if dist > 50.0 {
                                                pos.0 = target_cx;
                                                pos.1 = target_cy;
                                            } else if dist > 0.1 {
                                                pos.0 += dx * 0.2;
                                                pos.1 += dy * 0.2;
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
                                            let importance = (player.tile_count as f32).sqrt().max(1.0);
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
                                        let sizing_presence = vp.sizing_presence;
                                        let lod_presence = vp.lod_presence;

                                        // Small nations require zooming in to appear.
                                        let threshold = if player.id >= 200 {
                                            24.0 // Tribes need to be much closer/bigger to show text
                                        } else {
                                            8.0 // Nations can show text further away
                                        };
                                        let show_full = lod_presence >= threshold && full_labels_drawn < 100;

                                        if show_full {
                                            full_labels_drawn += 1;
                                            let ui_text_scale = ClientVisualConfig::default().ui_text_scale;

                                            // 1. Bounding box for font fitting (reference zoom, not current zoom)
                                            let empire_width_px = sizing_presence * 2.5; // Hexagons spread out
                                            let empire_height_px = sizing_presence * 1.5;

                                            // 2. Constrain font size so the text fits INSIDE those pixels
                                            let name_len = player.name.len().max(1) as f32;
                                            let max_by_width = empire_width_px / (name_len * 0.6); // Avg char width is ~60% of height
                                            let max_by_height = empire_height_px / 2.5; // Need space for 2 lines of text (name + troops)

                                            // 3. Raw font size that inscribes the territory at reference zoom
                                            let raw_font_size = max_by_width.min(max_by_height);

                                            // 4. Integer pt sizes → stable galley cache, stable atlas entries
                                            let target_font_size = raw_font_size * ui_text_scale;
                                            // Quantize to 2pt steps so float jitter does not rebuild galleys every frame.
                                            let font_size = (((target_font_size.round() as i32).clamp(14, 64) + 1) / 2 * 2) as f32;

                                            let is_human = player.player_type == sow_core::player::PlayerType::Human;
                                            let troops_for_label = self.troop_label_throttle
                                                .displayed_troops(wall_secs, player.id, player.troops);
                                            let new_troops_str = render_troops(troops_for_label);
                                            
                                            let display_name = if player.name.is_empty() {
                                                if player.id >= 200 { format!("Tribe {}", player.id - 199) } 
                                                else { format!("Nation {}", player.id - 103) }
                                            } else {
                                                player.name.clone()
                                            };

                                            let cache_entry = self.nameplate_cache.entry(player.id).or_insert_with(|| {
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
                                                cache_entry.troops_galley = crate::nameplate::layout_nameplate_troops_galley(
                                                    &painter,
                                                    font_id,
                                                    &new_troops_str,
                                                );
                                                cache_entry.last_formatted_troops = new_troops_str.clone();
                                                cache_entry.last_font_size = font_size;
                                            } else if new_troops_str != cache_entry.last_formatted_troops {
                                                let font_id = egui::FontId::proportional(font_size);
                                                cache_entry.troops_galley = crate::nameplate::layout_nameplate_troops_galley(
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
                                            crate::nameplate::paint_nameplate_galley(&painter, name_pos, name_galley.clone());
                                            crate::nameplate::paint_nameplate_galley(&painter, troops_pos, troops_galley.clone());
                                        } else {
                                            // Dot only — zero text layout, bare metal fast
                                            painter.circle_filled(center, dot_r, pc);
                                            painter.circle_stroke(center, dot_r, egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(180)));
                                        }
                                    }
                                    // --- Render Fleets ---
                                    if let Some(snap) = &self.sim.current_snapshot {
                                        for fleet in &snap.fleets {
                                            let mut r = 0.5; let mut g = 0.5; let mut b = 0.5;
                                            if let Some(owner) = snap.players.iter().find(|p| p.id == fleet.owner_id) {
                                                r = owner.color[0]; g = owner.color[1]; b = owner.color[2];
                                            }
                                            let color = egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
                                            let trail_color = egui::Color32::from_rgba_premultiplied((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 150);

                                            // Render trail
                                            let trail_len = fleet.path_cursor.min(fleet.path.len());
                                            for &tile in &fleet.path[..trail_len] {
                                                let wx = (tile % self.sim.map_w) as f32;
                                                let wy = (tile / self.sim.map_w) as f32;
                                                let screen_x = self.input.camera_x + wx * self.input.camera_zoom;
                                                let screen_y = self.input.camera_y + wy * self.input.camera_zoom;
                                                let rect = egui::Rect::from_min_size(
                                                    egui::pos2(screen_x, screen_y),
                                                    egui::vec2(self.input.camera_zoom, self.input.camera_zoom)
                                                );
                                                painter.rect_filled(rect, 0.0, trail_color);
                                            }

                                            // Render boat
                                            let wx = (fleet.current_tile % self.sim.map_w) as f32;
                                            let wy = (fleet.current_tile / self.sim.map_w) as f32;
                                            let screen_x = self.input.camera_x + wx * self.input.camera_zoom;
                                            let screen_y = self.input.camera_y + wy * self.input.camera_zoom;
                                            
                                            let margin = self.input.camera_zoom * 0.15;
                                            let rect = egui::Rect::from_min_max(
                                                egui::pos2(screen_x + margin, screen_y + margin),
                                                egui::pos2(screen_x + self.input.camera_zoom - margin, screen_y + self.input.camera_zoom - margin)
                                            );
                                            
                                            painter.rect(rect, 2.0, color, egui::Stroke::new(1.5_f32, egui::Color32::from_black_alpha(200)), egui::StrokeKind::Middle);

                                            if fleet.retreating && (self.start_time.elapsed().as_millis() / 500).is_multiple_of(2) {
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
                                                r = attacker.color[0]; g = attacker.color[1]; b = attacker.color[2];
                                            }
                                            if let Some(target) = snap.players.iter().find(|p| p.id == attack.target_owner) {
                                                tx = target.centroid_x + 0.5;
                                                ty = target.centroid_y + 0.5;
                                            }
                                            
                                            let start_x = self.input.camera_x + rx * self.input.camera_zoom;
                                            let start_y = self.input.camera_y + ry * self.input.camera_zoom;
                                            let end_x = self.input.camera_x + tx * self.input.camera_zoom;
                                            let end_y = self.input.camera_y + ty * self.input.camera_zoom;
                                            
                                            let color = egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
                                            let start_pos = egui::pos2(start_x, start_y);
                                            let end_pos = egui::pos2(end_x, end_y);
                                            
                                            // Simple thick line to represent attack
                                            painter.line_segment([start_pos, end_pos], egui::Stroke::new(3.0_f32, egui::Color32::from_black_alpha(150)));
                                            painter.line_segment([start_pos, end_pos], egui::Stroke::new(1.5_f32, color));
                                            
                                            if attack.retreating && (self.start_time.elapsed().as_millis() / 500).is_multiple_of(2) {
                                                let center = start_pos.lerp(end_pos, 0.5);
                                                painter.text(center, egui::Align2::CENTER_CENTER, "[X]", egui::FontId::proportional(20.0), egui::Color32::RED);
                                            }
                                        }
                                    }

    }
}
