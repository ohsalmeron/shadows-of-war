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
                            if cfg!(any(target_os = "android", target_os = "ios"))
                                && sf < 1.5
                                && self.screen_h > 800.0
                            {
                                sf = 2.0; // Force higher scale on dense mobile displays if OS reports 1.0
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
                                            });
                                        });
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
