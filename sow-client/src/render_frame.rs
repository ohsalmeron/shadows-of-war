#![allow(unused_imports)]
use sow_render::{RenderContext, MapRenderer, MapGlobals};
use crate::sim_bridge::{SimBridge, PlatformSimBridge};
use sow_core::protocol::{SimCommand, SimSnapshot};

use sow_core::game_config::GameConfig;

use blade_egui::GuiPainter;
use egui::{Context, RawInput, Pos2, Rect, Vec2};
use sow_ui::{ClientApp, app::ClientPhase, UiAction};
use web_time::{Instant, Duration};
use sow_net::client::SowClient;
use std::collections::HashMap;
use crate::{CAMERA_MIN_ZOOM, camera_zoom_upper_bound, NAMEPLATE_REFERENCE_ZOOM};
use crate::{spawn_sow_client_connect, get_build_version, get_maps_url};
use crate::nameplates::*;
use crate::client_config::ClientVisualConfig;
use crate::{MapDownloadEvent, EngineInitEvent};
use winit::event::{WindowEvent, MouseButton, ElementState, MouseScrollDelta};

use blade_graphics as gpu;
use crate::app_state::SowApp;
use std::io::Read;






impl SowApp {
    pub fn render_frame(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
                        #[cfg(target_arch = "wasm32")]
                        if let Some(win) = self.window.as_ref() {
                            let web_win = web_sys::window().unwrap();
                            let w = web_win.inner_width().unwrap().as_f64().unwrap();
                            let h = web_win.inner_height().unwrap().as_f64().unwrap();
                            
                            // Use the logical size and sf to calculate current physical size
                            let sf = win.scale_factor();
                            let expected_w = (w * sf) as u32;
                            let expected_h = (h * sf) as u32;
                            
                            if expected_w.abs_diff(self.screen_w as u32) > 1 || expected_h.abs_diff(self.screen_h as u32) > 1 {
                                let _ = win.request_surface_size(winit::dpi::LogicalSize::new(w, h).into());
                            }
                        }

                        if let Some(ref mut s) = self.surface {
                            if let Some(win) = self.window.as_ref() {
                                win.pre_present_notify();
                            }
                            let frame = s.acquire_frame();

                            if let Some(sp) = self.prev_sync_point.take() {
                                let _ = self.render_ctx.context.wait_for(&sp, !0);
                            }

                            self.render_ctx.command_encoder.start();
                            self.render_ctx.command_encoder.init_texture(frame.texture());

                            if let Some(ref mut mr) = self.map_renderer {
                                // Upload map state on first frame or after each tick
                                if self.needs_first_upload {
                                    self.render_ctx.command_encoder.init_texture(mr.texture);
                                    self.needs_first_upload = false;
                                }
                                mr.update(&mut self.render_ctx.command_encoder, &self.render_ctx.context, &self.current_snapshot.as_ref().map(|s| &s.dirty_tiles).unwrap_or(&vec![]));
                                if let Some(snap) = &mut self.current_snapshot {
                                    snap.dirty_tiles.clear();
                                }

                                let globals = MapGlobals {
                                    camera_pos: [self.camera_x, self.camera_y],
                                    zoom: self.camera_zoom,
                                    time: self.start_time.elapsed().as_secs_f32(),
                                    screen_size: [self.screen_w, self.screen_h],
                                    map_size: [self.map_w as f32, self.map_h as f32],
                                };
                                mr.draw(&mut self.render_ctx.command_encoder, frame.texture_view(), globals);
                            }

                            // ── UI UPDATE ───────────────────────────────────────
                            let mut sf = self.window.as_ref().map_or(1.0, |w| w.scale_factor() as f32);
                            if cfg!(any(target_os = "android", target_os = "ios")) {
                                if sf < 1.5 && self.screen_h > 800.0 {
                                    sf = 2.0; // Force higher scale on dense mobile displays if OS reports 1.0
                                } else if sf > 2.0 {
                                    sf = 2.0; // Don't let the GUI get too huge on iOS devices that report 3.0
                                }
                            }
                            
                            self.egui_ctx.set_pixels_per_point(sf);
                            self.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::Vec2::new(self.screen_w / sf, self.screen_h / sf)
                            ));
                            
                            for ev in &mut self.raw_input.events {
                                match ev {
                                    egui::Event::PointerMoved(pos) | egui::Event::PointerButton { pos, .. } => {
                                        pos.x /= sf;
                                        pos.y /= sf;
                                    }
                                    _ => {}
                                }
                            }
                            
                            let frame_now = Instant::now();
                            let dt = frame_now.duration_since(self.last_frame_time).as_secs_f32();
                            self.last_frame_time = frame_now;
                            self.raw_input.predicted_dt = dt.min(0.1);
                            
                            if self.app.main_menu_state.is_waiting && self.app.main_menu_state.wait_timer_secs > 0.0 {
                                self.app.main_menu_state.wait_timer_secs = (self.app.main_menu_state.wait_timer_secs - self.raw_input.predicted_dt).max(0.0);
                            }
                            if let Some(ref mut secs) = self.app.hud_state.spawn_timer_secs {
                                *secs = (*secs - self.raw_input.predicted_dt).max(0.0);
                            }
                            if let Some(ref mut sync) = self.app.hud_state.sync_state {
                                sync.time_remaining = (sync.time_remaining - self.raw_input.predicted_dt).max(0.0);
                            }
                            let mut local_cancel_intents = Vec::new();
                            
                            let egui_output = self.egui_ctx.run_ui(self.raw_input.clone(), |ctx| {
                                if self.app.phase == ClientPhase::Playing {
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
                                    if let Some(snap) = &self.current_snapshot {
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
                                            
                                            let screen_x = (pos.0 * self.camera_zoom + self.camera_x) / sf;
                                            let screen_y = (pos.1 * self.camera_zoom + self.camera_y) / sf;
                                            
                                            // Frustum cull
                                            if screen_x < -100.0 || screen_x > self.screen_w + 100.0 || screen_y < -100.0 || screen_y > self.screen_h + 100.0 { continue; }
                                            
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
                                            let lod_presence = importance * (self.camera_zoom / sf);
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
                                            let cache_entry = self.nameplate_cache.entry(player.id).or_insert_with(|| {
                                                let font_id = egui::FontId::proportional(font_size);
                                                let troops_str = new_troops_str.clone();
                                                
                                                CachedNameplate {
                                                    name_galley: layout_nameplate_name_galley(
                                                        &painter,
                                                        font_id.clone(),
                                                        &player.name,
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
                                                    &player.name,
                                                    is_human,
                                                    pc,
                                                );
                                                cache_entry.troops_galley = crate::nameplates::layout_nameplate_troops_galley(
                                                    &painter,
                                                    font_id,
                                                    &new_troops_str,
                                                );
                                                cache_entry.last_formatted_troops = new_troops_str.clone();
                                                cache_entry.last_font_size = font_size;
                                            } else if new_troops_str != cache_entry.last_formatted_troops {
                                                let font_id = egui::FontId::proportional(font_size);
                                                cache_entry.troops_galley = crate::nameplates::layout_nameplate_troops_galley(
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
                                            crate::nameplates::paint_nameplate_galley(&painter, name_pos, name_galley.clone());
                                            crate::nameplates::paint_nameplate_galley(&painter, troops_pos, troops_galley.clone());
                                        } else {
                                            // Dot only — zero text layout, bare metal fast
                                            painter.circle_filled(center, dot_r, pc);
                                            painter.circle_stroke(center, dot_r, egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(180)));
                                        }
                                    }
                                    // --- Render Fleets ---
                                    if let Some(snap) = &self.current_snapshot {
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
                                                let wx = (tile % self.map_w) as f32;
                                                let wy = (tile / self.map_w) as f32;
                                                let screen_x = self.camera_x + wx * self.camera_zoom;
                                                let screen_y = self.camera_y + wy * self.camera_zoom;
                                                let rect = egui::Rect::from_min_size(
                                                    egui::pos2(screen_x, screen_y),
                                                    egui::vec2(self.camera_zoom, self.camera_zoom)
                                                );
                                                painter.rect_filled(rect, 0.0, trail_color);
                                            }

                                            // Render boat
                                            let wx = (fleet.current_tile % self.map_w) as f32;
                                            let wy = (fleet.current_tile / self.map_w) as f32;
                                            let screen_x = self.camera_x + wx * self.camera_zoom;
                                            let screen_y = self.camera_y + wy * self.camera_zoom;
                                            
                                            let margin = self.camera_zoom * 0.15;
                                            let rect = egui::Rect::from_min_max(
                                                egui::pos2(screen_x + margin, screen_y + margin),
                                                egui::pos2(screen_x + self.camera_zoom - margin, screen_y + self.camera_zoom - margin)
                                            );
                                            
                                            painter.rect(rect, 2.0, color, egui::Stroke::new(1.5_f32, egui::Color32::from_black_alpha(200)), egui::StrokeKind::Middle);

                                            if fleet.retreating && (self.start_time.elapsed().as_millis() / 500) % 2 == 0 {
                                                let center = rect.center();
                                                painter.line_segment([egui::pos2(center.x - margin, center.y - margin), egui::pos2(center.x + margin, center.y + margin)], egui::Stroke::new(2.0_f32, egui::Color32::BLACK));
                                                painter.line_segment([egui::pos2(center.x + margin, center.y - margin), egui::pos2(center.x - margin, center.y + margin)], egui::Stroke::new(2.0_f32, egui::Color32::BLACK));
                                            }
                                        }
                                        
                                        for attack in &snap.attacks {
                                            if attack.target_owner == 0 { continue; }
                                            if attack.owner_id != self.my_player_id.unwrap_or(0) { continue; }
                                            
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
                                            
                                            let start_x = self.camera_x + rx * self.camera_zoom;
                                            let start_y = self.camera_y + ry * self.camera_zoom;
                                            let end_x = self.camera_x + tx * self.camera_zoom;
                                            let end_y = self.camera_y + ty * self.camera_zoom;
                                            
                                            let color = egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
                                            let start_pos = egui::pos2(start_x, start_y);
                                            let end_pos = egui::pos2(end_x, end_y);
                                            
                                            // Simple thick line to represent attack
                                            painter.line_segment([start_pos, end_pos], egui::Stroke::new(3.0_f32, egui::Color32::from_black_alpha(150)));
                                            painter.line_segment([start_pos, end_pos], egui::Stroke::new(1.5_f32, color));
                                            
                                            if attack.retreating && (self.start_time.elapsed().as_millis() / 500) % 2 == 0 {
                                                let center = start_pos.lerp(end_pos, 0.5);
                                                painter.text(center, egui::Align2::CENTER_CENTER, "[X]", egui::FontId::proportional(20.0), egui::Color32::RED);
                                            }
                                        }
                                    }
                                }

                                self.frame_count += 1;
                                if self.last_fps_time.elapsed().as_secs_f64() >= 1.0 {
                                    self.current_fps = self.frame_count;
                                    self.frame_count = 0;
                                    self.last_fps_time = Instant::now();
                                }

                                if self.last_ping_time.elapsed().as_secs_f64() >= 1.0 {
                                    if let Some(c) = self.net_client.as_ref() {
                                        let ping_msg = sow_core::protocol::ClientMessage::Ping {
                                            client_time: self.start_time.elapsed().as_secs_f64(),
                                        };
                                        if let Ok(json) = bincode::serialize(&ping_msg) {
                                            c.send(json);
                                        }
                                    }
                                    self.last_ping_time = Instant::now();
                                }
                                
                                if self.app.phase == ClientPhase::Playing {
                                    // Check long press
                                    if let Some((start, mx, my)) = self.map_touch_start {
                                        if start.elapsed().as_millis() > 500 {
                                            let world_x = (mx as f32 - self.camera_x) / self.camera_zoom;
                                            let world_y = (my as f32 - self.camera_y) / self.camera_zoom;
                                            let col = world_x.floor() as i32;
                                            let row = world_y.floor() as i32;
                                            if col >= 0 && row >= 0 && col < self.map_w as i32 && row < self.map_h as i32 {
                                                let idx = (row * self.map_w as i32 + col) as u32;
                                                self.map_context_menu = Some((mx as f32, my as f32, idx));
                                            }
                                            self.map_touch_start = None; // clear it so it doesn't re-trigger
                                        }
                                    }

                                    if let Some((mx, my, tile_idx)) = self.map_context_menu {
                                        let terrain_byte = self.map_renderer.as_ref().map(|mr| mr.terrain[tile_idx as usize]).unwrap_or(0);
                                        let is_land = (terrain_byte & 0x80) != 0;
                                        
                                        egui::Area::new(egui::Id::new("map_context_menu"))
                                            .anchor(egui::Align2::LEFT_TOP, egui::vec2(mx, my))
                                            .order(egui::Order::Foreground)
                                            .show(ctx, |ui| {
                                                egui::Frame::menu(&ctx.style()).show(ui, |ui| {
                                                    if is_land {
                                                        ui.label("Land Tile");
                                                    } else {
                                                        if ui.button("★ Send Fleet").clicked() {
                                                            let troops = Some(self.app.hud_state.troops * (self.app.hud_state.attack_ratio as f64));
                                                            let intent = sow_core::protocol::GameplayIntent::LaunchFleet {
                                                                target_tile: tile_idx,
                                                                troops,
                                                            };
                                                            if let Some(c) = self.net_client.as_ref() {
                                                                if let Ok(json) = bincode::serialize(&sow_core::protocol::ClientMessage::Gameplay { intent: intent.clone() }) {
                                                                    c.send(json);
                                                                }
                                                            } else {
                                                                let stamped = sow_core::protocol::StampedIntent { player_id: self.my_player_id.unwrap_or(1), intent };
                                                                self.bridge.send_command(sow_core::protocol::SimCommand::Turn(sow_core::protocol::Turn { turn_number: 0, intents: vec![stamped] }));
                                                            }
                                                            self.map_context_menu = None;
                                                        }
                                                    }
                                                    if ui.button("[X] Cancel").clicked() {
                                                        self.map_context_menu = None;
                                                    }
                                                });
                                            });
                                            
                                        // Auto-close if clicked elsewhere
                                        if ctx.input(|i| i.pointer.any_pressed()) && !ctx.egui_wants_pointer_input() {
                                            self.map_context_menu = None;
                                        }
                                    }

                                    egui::Area::new(egui::Id::new("fps_counter"))
                                        .anchor(egui::Align2::LEFT_TOP, egui::vec2(10.0, 10.0))
                                        .show(ctx, |ui| {
                                            ui.horizontal(|ui| {
                                                if let Some(ping) = self.current_ping_ms {
                                                    ui.label(
                                                        egui::RichText::new(format!("Ping: {}ms", ping))
                                                            .color(egui::Color32::WHITE)
                                                            .strong()
                                                    );
                                                }
                                                ui.label(
                                                    egui::RichText::new(format!("FPS: {}", self.current_fps))
                                                        .color(egui::Color32::YELLOW)
                                                        .strong()
                                                );
                                                ui.label(
                                                    egui::RichText::new(format!("Zoom: {:.2}", self.camera_zoom))
                                                        .color(egui::Color32::LIGHT_BLUE)
                                                        .strong()
                                                );
                                            });
                                        });
                                }

                                if self.app.phase == ClientPhase::Playing {
                                    if let Some(snap) = &self.current_snapshot {
                                        let my_pid = self.my_player_id.unwrap_or(0);
                                        if my_pid > 0 && (!snap.attacks.is_empty() || !snap.fleets.is_empty()) {
                                            egui::Window::new("Attacks")
                                                .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -140.0))
                                                .title_bar(false)
                                                .resizable(false)
                                                .collapsible(false)
                                                .frame(egui::Frame::window(&ctx.style()).fill(egui::Color32::from_black_alpha(200)))
                                                .show(ctx, |ui| {
                                                    ui.set_max_height(150.0);
                                                    egui::ScrollArea::vertical().show(ui, |ui| {
                                                        for attack in &snap.attacks {
                                                            if attack.owner_id == my_pid {
                                                                ui.horizontal(|ui| {
                                                                    ui.label(egui::RichText::new(format!("⚔ OUT {:.0}", attack.troops)).color(egui::Color32::from_rgb(0, 200, 255)));
                                                                    if let Some(target) = snap.players.iter().find(|p| p.id == attack.target_owner) {
                                                                        ui.label(&target.name);
                                                                    } else {
                                                                        ui.label("Wilderness");
                                                                    }
                                                                    if attack.retreating {
                                                                        ui.label("(Retreating...)");
                                                                    } else {
                                                                        if ui.button("[X]").clicked() {
                                                                            local_cancel_intents.push(sow_core::protocol::GameplayIntent::CancelAttack { attack_id: attack.id });
                                                                        }
                                                                    }
                                                                });
                                                            }
                                                        }
                                                        for fleet in &snap.fleets {
                                                            if fleet.owner_id == my_pid {
                                                                ui.horizontal(|ui| {
                                                                    ui.label(egui::RichText::new(format!("★ NAVY {:.0}", fleet.troops)).color(egui::Color32::from_rgb(0, 200, 255)));
                                                                    ui.label("Naval Invasion");
                                                                    if fleet.retreating {
                                                                        ui.label("(Retreating...)");
                                                                    } else {
                                                                        if ui.button("[X]").clicked() {
                                                                            local_cancel_intents.push(sow_core::protocol::GameplayIntent::RecallFleet { fleet_id: fleet.id });
                                                                        }
                                                                    }
                                                                });
                                                            }
                                                        }
                                                        for attack in &snap.attacks {
                                                            if attack.target_owner == my_pid {
                                                                ui.horizontal(|ui| {
                                                                    ui.label(egui::RichText::new(format!("⚔ IN {:.0}", attack.troops)).color(egui::Color32::RED));
                                                                    if let Some(attacker) = snap.players.iter().find(|p| p.id == attack.owner_id) {
                                                                        ui.label(&attacker.name);
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    });
                                                });
                                        }
                                    }
                                }

                                if let Some(action) = self.app.draw(ctx) {
                                    match action {
                                        UiAction::StartSinglePlayer => {
                                            self.app.phase = ClientPhase::Playing;
                                        }
                                        UiAction::ConnectToServer(addr) => {
                                            self.app.main_menu_state.is_connecting = true;
                                            let url = addr.clone();
                                            #[cfg(target_arch = "wasm32")]
                                            spawn_sow_client_connect(url, &self.connect_tx);
                                            #[cfg(not(target_arch = "wasm32"))]
                                            spawn_sow_client_connect(url, &self.connect_tx, &self.tokio_rt);
                                        }
                                        UiAction::JoinLobby(id) => {
                                            let join_msg = sow_core::protocol::ClientMessage::Join {
                                                name: self.app.main_menu_state.player_name.clone(),
                                                is_observer: false,
                                                target_lobby_id: Some(id),
                                                build_version: get_build_version(),
                                            };
                                            self.app.main_menu_state.pending_join_lobby_id = Some(id);
                                            if let Ok(json) = bincode::serialize(&join_msg) {
                                                if let Some(c) = self.net_client.as_ref() {
                                                    c.send(json);
                                                }
                                            }
                                            self.app.main_menu_state.is_waiting = true;
                                        }
                                        UiAction::LeaveLobby => {
                                            if let Some(c) = self.net_client.as_ref() {
                                                let leave = sow_core::protocol::ClientMessage::Leave {};
                                                if let Ok(json) = bincode::serialize(&leave) {
                                                    c.send(json);
                                                }
                                            }
                                            self.app.hud_state.connection_lost = false;
                                            self.app.main_menu_state.is_waiting = false;
                                            self.app.main_menu_state.pending_join_lobby_id = None;
                                            self.app.main_menu_state.joined_lobby_id = None;
                                            self.my_lobby_id = None;
                                            self.my_player_id = None;
                                            self.camera_x = 0.0;
                                            self.camera_y = 0.0;
                                            self.camera_zoom = 2.0;
                                            self.app.phase = ClientPhase::Splash;
                                            self.app.splash_state.job = sow_ui::ui::loading_screen::SplashJob::ExitGame;
                                            self.app.splash_state.gpu_load_step = 0;
                                            self.app.splash_state.frames_drawn = 0;
                                        }
                                        UiAction::SetAttackRatio(r) => {
                                            self.app.hud_state.attack_ratio = r;
                                        }
                                        UiAction::CenterCamera => {
                                            let pid = self.my_player_id.unwrap_or(1);
                                            if let Some(player) =
                                                self.current_snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.id == pid))
                                            {
                                                if player.tile_count > 0 && player.alive {
                                                    let cx = player.centroid_x;
                                                    let cy = player.centroid_y;
                                                    
                                                    let world_cx = cx + 0.5;
                                                    let world_cy = cy + 0.5;

                                                    self.camera_x = self.screen_w * 0.5 - world_cx * self.camera_zoom;
                                                    self.camera_y = self.screen_h * 0.5 - world_cy * self.camera_zoom;
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }

                                // The new nameplates are rendered before self.app.draw()
                            });

                            for intent in local_cancel_intents {
                                if let Some(c) = self.net_client.as_ref() {
                                    let msg = sow_core::protocol::ClientMessage::Gameplay { intent };
                                    if let Ok(json) = bincode::serialize(&msg) {
                                        c.send(json);
                                    }
                                } else {
                                    let stamped = sow_core::protocol::StampedIntent {
                                        player_id: self.my_player_id.unwrap_or(1),
                                        intent,
                                    };
                                    self.bridge.send_command(sow_core::protocol::SimCommand::Turn(sow_core::protocol::Turn { turn_number: 0, intents: vec![stamped] }));
                                }
                            }

                            if let Some(win) = self.window.as_ref() {
                                let ime_opt = egui_output.platform_output.ime;
                                let allow_ime = ime_opt.is_some();
                                
                                if let Some(ime_out) = ime_opt {
                                    let ppp = egui_output.pixels_per_point;
                                    let ime_rect_px = ppp * ime_out.rect;
                                    let had_input_events = !self.raw_input.events.is_empty();
                                    let toggling = self.ime_allowed_state != allow_ime;
                                    
                                    if toggling || self.ime_cursor_rect_px != Some(ime_rect_px) || had_input_events {
                                        self.ime_allowed_state = true;
                                        self.ime_cursor_rect_px = Some(ime_rect_px);
                                        
                                        let request_data = winit::window::ImeRequestData::default()
                                            .with_cursor_area(
                                                winit::dpi::PhysicalPosition::new(
                                                    ime_rect_px.min.x.round() as i32,
                                                    ime_rect_px.min.y.round() as i32,
                                                ).into(),
                                                winit::dpi::PhysicalSize::new(
                                                    ime_rect_px.width().round().max(1.0) as u32,
                                                    ime_rect_px.height().round().max(1.0) as u32,
                                                ).into()
                                            );
                                            
                                        if toggling {
                                            let caps = winit::window::ImeCapabilities::new().with_cursor_area();
                                            if let Some(req) = winit::window::ImeEnableRequest::new(caps, request_data) {
                                                let _ = win.request_ime_update(winit::window::ImeRequest::Enable(req));
                                            }
                                        } else {
                                            let _ = win.request_ime_update(winit::window::ImeRequest::Update(request_data));
                                        }
                                    }
                                } else if self.ime_allowed_state {
                                    self.ime_allowed_state = false;
                                    self.ime_cursor_rect_px = None;
                                    let _ = win.request_ime_update(winit::window::ImeRequest::Disable);
                                }
                            }

                            self.raw_input.events.clear();

                            // ── DRAWING UI ──────────────────────────────────────────
                            if let Some(ref mut gp) = self.gui_painter {
                                let screen_desc = blade_egui::ScreenDescriptor {
                                    physical_size: (self.screen_w as u32, self.screen_h as u32),
                                    scale_factor: sf,
                                };
                                let paint_jobs = self.egui_ctx.tessellate(egui_output.shapes, sf);
                                gp.update_textures(
                                    &mut self.render_ctx.command_encoder,
                                    &egui_output.textures_delta,
                                    &self.render_ctx.context,
                                );

                                let mut pass = self.render_ctx.command_encoder.render("ui_pass", gpu::RenderTargetSet {
                                    colors: &[gpu::RenderTarget {
                                        view: frame.texture_view(),
                                        init_op: gpu::InitOp::Load,
                                        finish_op: gpu::FinishOp::Store,
                                    }],
                                    depth_stencil: None,
                                });

                                gp.paint(&mut pass, &paint_jobs, &screen_desc, &self.render_ctx.context);
                                drop(pass);
                            }
                            if let Some(ref mut gp) = self.gui_painter {
                                gp.sync(&self.render_ctx.context);
                            }
                            self.render_ctx.command_encoder.present(frame);
                            let sync_point = self.render_ctx.context.submit(&mut self.render_ctx.command_encoder);
                            
                            if let Some(ref mut gp) = self.gui_painter {
                                gp.after_submit(&sync_point);
                            }
                            
                            self.prev_sync_point = Some(sync_point);
                        }

    }
}
