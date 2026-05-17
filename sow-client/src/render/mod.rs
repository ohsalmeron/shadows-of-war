use sow_render::MapGlobals;
use crate::{CAMERA_MIN_ZOOM, camera_zoom_upper_bound};



use sow_ui::app::ClientPhase;
use web_time::Instant;

use blade_graphics as gpu;
use crate::app::SowApp;



pub mod world;
pub mod interact;






impl SowApp {
    pub fn render_frame(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
                        #[cfg(target_arch = "wasm32")]
                        if let Some(win) = self.gfx.window.as_ref() {
                            let web_win = web_sys::window().unwrap();
                            let w = web_win.inner_width().unwrap().as_f64().unwrap();
                            let h = web_win.inner_height().unwrap().as_f64().unwrap();
                            
                            // Use the logical size and sf to calculate current physical size
                            let sf = win.scale_factor();
                            let expected_w = (w * sf) as u32;
                            let expected_h = (h * sf) as u32;
                            
                            if expected_w.abs_diff(self.input.screen_w as u32) > 1 || expected_h.abs_diff(self.input.screen_h as u32) > 1 {
                                let _ = win.request_surface_size(winit::dpi::LogicalSize::new(w, h).into());
                            }
                        }

                        if let Some(ref mut s) = self.gfx.surface {
                            if let Some(win) = self.gfx.window.as_ref() {
                                win.pre_present_notify();
                            }
                            let frame = s.acquire_frame();

                            if let Some(sp) = self.gfx.prev_sync_point.take() {
                                let _ = self.gfx.render_ctx.context.wait_for(&sp, !0);
                            }

                            self.gfx.render_ctx.command_encoder.start();
                            self.gfx.render_ctx.command_encoder.init_texture(frame.texture());

                            if let Some(ref mut mr) = self.gfx.map_renderer {
                                // Upload map state on first frame or after each tick
                                if self.gfx.needs_first_upload {
                                    self.gfx.render_ctx.command_encoder.init_texture(mr.texture);
                                    self.gfx.needs_first_upload = false;
                                    // Full buffer→texture copy so terrain is visible before any dirty tiles arrive
                                    self.gfx.render_ctx.context.sync_buffer(mr.raw_buffer);
                                    let src_piece: blade_graphics::BufferPiece = mr.raw_buffer.into();
                                    let dst_piece: blade_graphics::TexturePiece = mr.texture.into();
                                    let mut transfer = self.gfx.render_ctx.command_encoder.transfer("map_init_upload");
                                    transfer.copy_buffer_to_texture(
                                        src_piece,
                                        mr.bytes_per_row,
                                        dst_piece,
                                        blade_graphics::Extent { width: mr.width, height: mr.height, depth: 1 },
                                    );
                                }

                                // Perform CPU-side update of the map
                                let dirty = self.sim.current_snapshot.as_ref().map(|s| s.dirty_tiles.as_slice()).unwrap_or(&[]);
                                mr.update(&mut self.gfx.render_ctx.command_encoder, &self.gfx.render_ctx.context, dirty);
                                if let Some(snap) = &mut self.sim.current_snapshot {
                                    snap.dirty_tiles.clear();
                                }
                                let mut border_thickness = 0.4f32;
                                let mut border_darkness = 0.15f32;
                                let mut shore_thickness = 0.4f32;
                                let mut shore_darkness = 0.15f32;
                                let mut border_roundness = 0.5f32;

                                self.ui.egui_ctx.data_mut(|d| {
                                    border_thickness = *d.get_temp_mut_or_insert_with(egui::Id::new("dev_thickness"), || 0.4f32);
                                    border_darkness = *d.get_temp_mut_or_insert_with(egui::Id::new("dev_darkness"), || 0.15f32);
                                    shore_thickness = *d.get_temp_mut_or_insert_with(egui::Id::new("dev_shore_thickness"), || 0.4f32);
                                    shore_darkness = *d.get_temp_mut_or_insert_with(egui::Id::new("dev_shore_darkness"), || 0.15f32);
                                    border_roundness = *d.get_temp_mut_or_insert_with(egui::Id::new("dev_roundness"), || 0.5f32);
                                });

                                let globals = MapGlobals {
                                    camera_pos: [self.input.camera_x, self.input.camera_y],
                                    zoom: self.input.camera_zoom,
                                    time: self.time.start_time.elapsed().as_secs_f32(),
                                    screen_size: [self.input.screen_w, self.input.screen_h],
                                    map_size: [self.sim.map_w as f32, self.sim.map_h as f32],
                                    border_thickness,
                                    border_darkness,
                                    shore_thickness,
                                    shore_darkness,
                                    border_roundness,
                                    _pad1: 0.0,
                                    _pad2: 0.0,
                                    _pad3: 0.0,
                                };
                                mr.draw(&mut self.gfx.render_ctx.command_encoder, frame.texture_view(), globals);
                            }

                            // ── UI UPDATE ───────────────────────────────────────
                            let mut sf = self.gfx.window.as_ref().map_or(1.0, |w| w.scale_factor() as f32);
                            if cfg!(any(target_os = "android", target_os = "ios")) {
                                if sf < 1.5 && self.input.screen_h > 800.0 {
                                    sf = 2.0; // Force higher scale on dense mobile displays if OS reports 1.0
                                } else if sf > 2.0 {
                                    sf = 2.0; // Don't let the GUI get too huge on iOS devices that report 3.0
                                }
                            }
                            
                            self.ui.egui_ctx.set_pixels_per_point(sf);
                            self.ui.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::Vec2::new(self.input.screen_w / sf, self.input.screen_h / sf)
                            ));

                            if cfg!(target_os = "android") {
                                if self.ui.app.phase == sow_ui::app::ClientPhase::MainMenu {
                                    let config = crate::config::ClientVisualConfig::default();
                                    self.ui.raw_input.safe_area_insets = Some(egui::SafeAreaInsets(egui::Margin {
                                        top: config.safe_area_top as i8,
                                        bottom: config.safe_area_bottom as i8,
                                        left: 0,
                                        right: 0,
                                    }.into()));
                                } else {
                                    self.ui.raw_input.safe_area_insets = Some(egui::SafeAreaInsets(egui::Margin {
                                        top: 0,
                                        bottom: 0,
                                        left: 0,
                                        right: 0,
                                    }.into()));
                                }
                            }

                            
                            for ev in &mut self.ui.raw_input.events {
                                match ev {
                                    egui::Event::PointerMoved(pos) | egui::Event::PointerButton { pos, .. } => {
                                        pos.x /= sf;
                                        pos.y /= sf;
                                    }
                                    _ => {}
                                }
                            }
                            
                            let frame_now = Instant::now();
                            let dt = frame_now.duration_since(self.time.last_frame_time).as_secs_f32();
                            self.time.last_frame_time = frame_now;
                            self.ui.raw_input.predicted_dt = dt.min(0.1);
                            
                            if self.ui.app.main_menu_state.is_waiting && self.ui.app.main_menu_state.wait_timer_secs > 0.0 {
                                self.ui.app.main_menu_state.wait_timer_secs = (self.ui.app.main_menu_state.wait_timer_secs - self.ui.raw_input.predicted_dt).max(0.0);
                            }
                            if let Some(ref mut secs) = self.ui.app.hud_state.spawn_timer_secs {
                                *secs = (*secs - self.ui.raw_input.predicted_dt).max(0.0);
                            }
                            if let Some(ref mut sync) = self.ui.app.hud_state.sync_state {
                                sync.time_remaining = (sync.time_remaining - self.ui.raw_input.predicted_dt).max(0.0);
                            }
                            let mut local_cancel_intents = Vec::new();
                            


                            #[cfg(target_arch = "wasm32")]
                            self.ime_bridge
                                .drain_pending_into(&mut self.ui.raw_input.events);

                            let egui_ctx = self.ui.egui_ctx.clone();
                            let egui_output = egui_ctx.run_ui(self.ui.raw_input.clone(), |ctx| {
                                if cfg!(target_os = "android")
                                    && self.ui.app.phase == sow_ui::app::ClientPhase::MainMenu {
                                        let config = crate::config::ClientVisualConfig::default();
                                        let screen_rect = ctx.content_rect();
                                        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("safe_area_bars")));
                                        
                                        let top_c = config.top_bar_color;
                                        painter.rect_filled(
                                            egui::Rect::from_min_max(screen_rect.min, egui::pos2(screen_rect.max.x, screen_rect.min.y + config.safe_area_top)),
                                            0.0,
                                            egui::Color32::from_rgba_premultiplied(top_c[0], top_c[1], top_c[2], top_c[3]),
                                        );
                                        
                                        let bot_c = config.bottom_bar_color;
                                        painter.rect_filled(
                                            egui::Rect::from_min_max(egui::pos2(screen_rect.min.x, screen_rect.max.y - config.safe_area_bottom), screen_rect.max),
                                            0.0,
                                            egui::Color32::from_rgba_premultiplied(bot_c[0], bot_c[1], bot_c[2], bot_c[3]),
                                        );
                                    }

                                if self.ui.app.phase == sow_ui::app::ClientPhase::Playing {
                                    self.render_world_overlays(ctx, sf);
                                    self.render_tutorial_ui(ctx);
                                }
                                
                                self.calculate_fps_and_ping();
                                
                                if self.ui.app.phase == ClientPhase::Playing {
                                    self.handle_map_interactions(ctx);
                                    self.render_endgame_ui(ctx);
                                    self.render_leaderboard(ctx);
                                }
                                
                                self.render_dev_panels(ctx, &mut local_cancel_intents);

                                if self.ui.update_available {
                                    egui::Window::new("Update Available")
                                        .collapsible(false)
                                        .resizable(false)
                                        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                                        .show(ctx, |ui| {
                                            ui.heading("A new version is available!");
                                            ui.add_space(10.0);
                                            ui.label("The server has been updated. Please refresh to continue playing.");
                                            ui.add_space(10.0);
                                            if ui.button("Update Now").clicked() {
                                                #[cfg(target_arch = "wasm32")]
                                                if let Some(window) = web_sys::window() {
                                                    let _ = window.location().reload();
                                                }
                                                #[cfg(not(target_arch = "wasm32"))]
                                                {
                                                    self.ui.update_available = false;
                                                }
                                            }
                                        });
                                }
                                
                                self.process_ui_actions(ctx);
                            });

                            for intent in local_cancel_intents {
                                if self.net.is_offline {
                                    self.sim.offline_intents.push(intent);
                                } else if let Some(c) = self.net.client.as_ref() {
                                    let msg = sow_core::protocol::ClientMessage::Gameplay { intent };
                                    if let Ok(json) = bincode::serialize(&msg) {
                                        c.send(json);
                                    }
                                }
                            }

                            // ── OFFLINE TICK GENERATOR ────────────────────────
                            if self.net.is_offline && self.ui.app.phase == ClientPhase::Playing {
                                let mut dt = self.ui.raw_input.predicted_dt;
                                if dt > 0.1 { dt = 0.05; } // Clamp to prevent tick burst
                                self.sim.offline_tick_timer += dt;
                                while self.sim.offline_tick_timer >= 0.05 { // 20 TPS (50ms)
                                    self.sim.offline_tick_timer -= 0.05;
                                    
                                    let raw_intents = std::mem::take(&mut self.sim.offline_intents);
                                    log::debug!("Offline tick generator sending Turn. dt: {}, timer: {}", dt, self.sim.offline_tick_timer);
                                    let mut stamped_intents = Vec::with_capacity(raw_intents.len());
                                    for intent in raw_intents {
                                        stamped_intents.push(sow_core::protocol::StampedIntent {
                                            player_id: self.sim.my_player_id.unwrap_or(1),
                                            intent,
                                        });
                                    }
                                    
                                    let turn = sow_core::protocol::Turn {
                                        turn_number: 0, // Ignored by client simulation
                                        intents: stamped_intents,
                                    };
                                    self.dispatch_sim_command(sow_core::protocol::SimCommand::Turn(turn));
                                }
                            }

                            #[cfg(not(target_arch = "wasm32"))]
                            if let Some(win) = self.gfx.window.as_ref() {
                                let ime_opt = egui_output.platform_output.ime;
                                let allow_ime = ime_opt.is_some();
                                
                                if let Some(ime_out) = ime_opt {
                                    let ppp = egui_output.pixels_per_point;
                                    let ime_rect_px = ppp * ime_out.rect;
                                    let had_input_events = !self.ui.raw_input.events.is_empty();
                                    let toggling = self.input.ime_allowed_state != allow_ime;
                                    
                                    if toggling || self.input.ime_cursor_rect_px != Some(ime_rect_px) || had_input_events {
                                        self.input.ime_allowed_state = true;
                                        self.input.ime_cursor_rect_px = Some(ime_rect_px);
                                        
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
                                } else if self.input.ime_allowed_state {
                                    self.input.ime_allowed_state = false;
                                    self.input.ime_cursor_rect_px = None;
                                    let _ = win.request_ime_update(winit::window::ImeRequest::Disable);
                                }
                            }

                            #[cfg(target_arch = "wasm32")]
                            self.ime_bridge
                                .sync_from_egui_ime(egui_output.platform_output.ime);

                            self.ui.raw_input.events.clear();

                            // ── DRAWING UI ──────────────────────────────────────────
                            if let Some(ref mut gp) = self.gfx.gui_painter {
                                let screen_desc = blade_egui::ScreenDescriptor {
                                    physical_size: (self.input.screen_w as u32, self.input.screen_h as u32),
                                    scale_factor: sf,
                                };
                                let paint_jobs = self.ui.egui_ctx.tessellate(egui_output.shapes, sf);
                                gp.update_textures(
                                    &mut self.gfx.render_ctx.command_encoder,
                                    &egui_output.textures_delta,
                                    &self.gfx.render_ctx.context,
                                );

                                let mut pass = self.gfx.render_ctx.command_encoder.render("ui_pass", gpu::RenderTargetSet {
                                    colors: &[gpu::RenderTarget {
                                        view: frame.texture_view(),
                                        init_op: gpu::InitOp::Load,
                                        finish_op: gpu::FinishOp::Store,
                                    }],
                                    depth_stencil: None,
                                });

                                gp.paint(&mut pass, &paint_jobs, &screen_desc, &self.gfx.render_ctx.context);
                                drop(pass);
                            }
                            if let Some(ref mut gp) = self.gfx.gui_painter {
                                gp.sync(&self.gfx.render_ctx.context);
                            }
                            self.gfx.render_ctx.command_encoder.present(frame);
                            let sync_point = self.gfx.render_ctx.context.submit(&mut self.gfx.render_ctx.command_encoder);
                            
                            if let Some(ref mut gp) = self.gfx.gui_painter {
                                gp.after_submit(&sync_point);
                            }
                            
                            self.gfx.prev_sync_point = Some(sync_point);
                        }

    }
}


impl SowApp {
    pub fn check_surface(&mut self) {
        if self.gfx.surface.is_none() && self.gfx.window.is_some() {
            let win = self.gfx.window.as_ref().unwrap();
            let sz = win.surface_size();
            match self.gfx.render_ctx.create_surface(win, sz.width.max(1), sz.height.max(1)) {
                Ok(s) => {
                    self.input.screen_w = sz.width as f32;
                    self.input.screen_h = sz.height as f32;
                    let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h);
                    self.input.camera_zoom = self.input.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);
                    self.ui.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::new(self.input.screen_w, self.input.screen_h)
                    ));
                    let format = s.info().format;
                    
                    if let Some(sp) = self.gfx.prev_sync_point.take() {
                        let _ = self.gfx.render_ctx.context.wait_for(&sp, !0);
                    }
                    let mut old_terrain = vec![128; (self.sim.map_w * self.sim.map_h) as usize];
                    if let Some(mut old_mr) = self.gfx.map_renderer.take() {
                        old_terrain = old_mr.terrain.clone();
                        old_mr.destroy(&self.gfx.render_ctx);
                    }
                    self.gfx.map_renderer = Some(sow_render::MapRenderer::new(&self.gfx.render_ctx.context, self.sim.map_w, self.sim.map_h, format, &old_terrain));
                    self.gfx.needs_first_upload = true;
                    
                    self.gfx.gui_painter = Some(blade_egui::GuiPainter::new(s.info(), &self.gfx.render_ctx.context));
                    self.gfx.surface = Some(s);
                    
                    self.ui.egui_ctx = egui::Context::default();
                    sow_ui::ui::theme::apply_theme(&self.ui.egui_ctx);
                    log::info!("Successfully created surface on retry.");
                }
                Err(_) => {
                    // Still unavailable
                }
            }
        }

    }
}
