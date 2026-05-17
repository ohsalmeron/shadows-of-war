use crate::sim::SimBridge;
use sow_core::protocol::SimCommand;


use blade_graphics as gpu;
use egui::{Pos2, Rect, Vec2};
use sow_ui::app::ClientPhase;
use crate::{CAMERA_MIN_ZOOM, camera_zoom_upper_bound};
use winit::event::{WindowEvent, MouseButton, ElementState, MouseScrollDelta};
use crate::app::SowApp;


impl SowApp {
    pub fn handle_window_event(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop, event: WindowEvent) {
        match event {

                    WindowEvent::CloseRequested => {
                        if let Some(sp) = self.gfx.prev_sync_point.take() {
                            let _ = self.gfx.render_ctx.context.wait_for(&sp, !0);
                        }
                        if let Some(mut s) = self.gfx.surface.take() {
                            if let Some(mut gp) = self.gfx.gui_painter.take() {
                                gp.destroy(&self.gfx.render_ctx.context);
                            }
                            if let Some(mut mr) = self.gfx.map_renderer.take() {
                                mr.destroy(&self.gfx.render_ctx);
                            }
                            self.gfx.render_ctx.context.destroy_command_encoder(&mut self.gfx.render_ctx.command_encoder);
                            self.gfx.render_ctx.context.destroy_surface(&mut s);
                        }
                        event_loop.exit()
                    }
                    WindowEvent::SurfaceResized(physical_size) => {
                        if physical_size.width > 0 && physical_size.height > 0 {
                            if let Some(sp) = self.gfx.prev_sync_point.take() {
                                let _ = self.gfx.render_ctx.context.wait_for(&sp, !0);
                            }
                            if let Some(ref mut s) = self.gfx.surface {
                                self.gfx.render_ctx.context.reconfigure_surface(s, gpu::SurfaceConfig {
                                    size: gpu::Extent {
                                        width: physical_size.width,
                                        height: physical_size.height,
                                        depth: 1,
                                    },
                                    usage: gpu::TextureUsage::TARGET,
                                    display_sync: gpu::DisplaySync::Tear,
                                    ..Default::default()
                                });
                            }
                            self.input.screen_w = physical_size.width as f32;
                            self.input.screen_h = physical_size.height as f32;
                            let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h);
                            self.input.camera_zoom = self.input.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);
                            self.ui.raw_input.screen_rect = Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(self.input.screen_w, self.input.screen_h)
                            ));
                            if let Some(win) = self.gfx.window.as_ref() {
                                win.request_redraw();
                            }
                        }
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        let pressed = event.state == ElementState::Pressed;
                        if pressed && !self.ui.egui_ctx.egui_wants_keyboard_input() && self.ui.app.phase == ClientPhase::Playing && self.ui.app.hud_state.sync_state.is_none() {
                            if let winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyB) = event.physical_key {
                                let world_x = (self.input.last_mouse_x as f32 - self.input.camera_x) / self.input.camera_zoom;
                                let world_y = (self.input.last_mouse_y as f32 - self.input.camera_y) / self.input.camera_zoom;
                                let col = world_x.floor() as i32;
                                let row = world_y.floor() as i32;
                                if col >= 0 && row >= 0 && col < self.sim.map_w as i32 && row < self.sim.map_h as i32 {
                                    let idx = (row * self.sim.map_w as i32 + col) as usize;

                                    let troops = Some(self.ui.app.hud_state.troops * (self.ui.app.hud_state.attack_ratio as f64));
                                    let intent = sow_core::protocol::GameplayIntent::LaunchFleet { target_tile: idx as u32, troops };
                                    if let Some(c) = self.net.client.as_ref() {
                                        if let Ok(json) = bincode::serialize(&sow_core::protocol::ClientMessage::Gameplay { intent: intent.clone() }) {
                                            c.send(json);
                                        }
                                    } else {

                                                let stamped = sow_core::protocol::StampedIntent { player_id: self.sim.my_player_id.unwrap_or(1), intent };
                                        self.sim.bridge.send_command(SimCommand::Turn(sow_core::protocol::Turn { turn_number: 0, intents: vec![stamped] }));
                                    }
                                }
                            }
                        }

                        if pressed {
                            if let winit::keyboard::Key::Character(text) = &event.logical_key {
                                self.ui.raw_input.events.push(egui::Event::Text(text.to_string()));
                            } else if let winit::keyboard::Key::Named(named) = &event.logical_key {
                                if *named == winit::keyboard::NamedKey::Backspace {
                                    self.ui.raw_input.events.push(egui::Event::Key {
                                        key: egui::Key::Backspace,
                                        physical_key: None,
                                        pressed: true,
                                        repeat: false,
                                        modifiers: Default::default(),
                                    });
                                }
                            }
                        }
                    }
                    WindowEvent::Ime(ime) => {
                        use winit::event::Ime;
                        match ime {
                            Ime::Enabled | Ime::Disabled | Ime::DeleteSurrounding { .. } => {}
                            Ime::Preedit(text, _) => {
                                self.ui.raw_input
                                    .events
                                    .push(egui::Event::Ime(egui::ImeEvent::Preedit(text.clone())));
                            }
                            Ime::Commit(text) => {
                                self.ui.raw_input
                                    .events
                                    .push(egui::Event::Ime(egui::ImeEvent::Commit(text.clone())));
                            }
                        }
                    }
                    WindowEvent::PointerButton { state: btn_state, button, position, .. } => {
                        let pressed = btn_state == ElementState::Pressed;
                        self.input.last_mouse_x = position.x;
                        self.input.last_mouse_y = position.y;

                        let is_primary = match button {
                            winit::event::ButtonSource::Mouse(b) => b == MouseButton::Left,
                            winit::event::ButtonSource::Touch { .. } => true,
                            _ => false,
                        };
                        let is_secondary = match button {
                            winit::event::ButtonSource::Mouse(b) => b == MouseButton::Right,
                            _ => false,
                        };

                        if let winit::event::ButtonSource::Touch { finger_id, .. } = button {
                            let id = finger_id.into_raw() as u64;
                            if pressed {
                                self.input.active_touches.insert(id, (position.x, position.y));
                            } else {
                                self.input.active_touches.remove(&id);
                                if self.input.active_touches.len() < 2 {
                                    self.input.last_pinch_distance = None;
                                }
                            }
                        }

                        let wants_pointer = self.ui.egui_ctx.egui_wants_pointer_input();

                        if is_primary {
                            if pressed {
                                if !wants_pointer {
                                    self.input.dragging = true;
                                }
                            } else {
                                self.input.dragging = false;
                            }
                        }

                        if pressed && !wants_pointer {
                            self.input.map_touch_start = Some((web_time::Instant::now(), position.x, position.y));
                        }

                        if !pressed && !wants_pointer && self.ui.app.phase == ClientPhase::Playing && self.ui.app.hud_state.sync_state.is_none() {
                            if let Some((_, sx, sy)) = self.input.map_touch_start {
                                // Distance check just in case (though movement clears it too)
                                let dx = position.x - sx;
                                let dy = position.y - sy;
                                let dist = dx*dx + dy*dy;

                                if dist <= 400.0 {
                                    let world_x = (sx as f32 - self.input.camera_x) / self.input.camera_zoom;
                                    let world_y = (sy as f32 - self.input.camera_y) / self.input.camera_zoom;
                                    
                                    let col = world_x.floor() as i32;
                                    let row = world_y.floor() as i32;

                                    if col >= 0 && row >= 0 && col < self.sim.map_w as i32 && row < self.sim.map_h as i32 {
                                        let phase = self.sim.current_snapshot.as_ref().map(|s| &s.phase).unwrap_or(&sow_core::game::GamePhase::Lobby);

                                        let mut intent_opt = None;

                                        if matches!(phase, sow_core::game::GamePhase::Spawning { .. }) {
                                            if is_primary {
                                                intent_opt = Some(sow_core::protocol::GameplayIntent::Spawn { x: col as u32, y: row as u32 });
                                            }
                                        } else {
                                            let idx = (row * self.sim.map_w as i32 + col) as usize;
                                            let owner = self.gfx.map_renderer.as_ref().map(|mr| mr.owners[idx]).unwrap_or(0);
                                            let terrain_byte = self.gfx.map_renderer.as_ref().map(|mr| mr.terrain[idx]).unwrap_or(0);
                                            let is_land = (terrain_byte & 0x80) != 0;

                                            if is_secondary {
                                                let troops = Some(self.ui.app.hud_state.troops * (self.ui.app.hud_state.attack_ratio as f64));
                                                intent_opt = Some(sow_core::protocol::GameplayIntent::LaunchFleet {
                                                    target_tile: idx as u32,
                                                    troops,
                                                });
                                            } else if is_primary
                                                && is_land && owner != self.sim.my_player_id.unwrap_or(0) {
                                                    let attack = sow_core::protocol::AttackIntent {
                                                        target_owner: owner,
                                                        troops: Some(self.ui.app.hud_state.troops * (self.ui.app.hud_state.attack_ratio as f64)),
                                                    };
                                                    intent_opt = Some(sow_core::protocol::GameplayIntent::Attack(attack));
                                                }
                                        }
                                        
                                        if let Some(intent) = intent_opt {
                                            if let Some(c) = self.net.client.as_ref() {
                                                let msg = sow_core::protocol::ClientMessage::Gameplay {
                                                    intent: intent.clone(),
                                                };
                                                if let Ok(json) = bincode::serialize(&msg) {
                                                    c.send(json);
                                                }
                                            } else {

                                                let stamped = sow_core::protocol::StampedIntent {
                                                    player_id: self.sim.my_player_id.unwrap_or(1),
                                                    intent,
                                                };
                                                self.sim.bridge.send_command(SimCommand::Turn(sow_core::protocol::Turn { turn_number: 0, intents: vec![stamped] }));
                                            }
                                        }
                                    }
                                }
                                self.input.map_touch_start = None;
                            }
                        }

                        self.ui.raw_input.events.push(egui::Event::PointerButton {
                            pos: Pos2::new(self.input.last_mouse_x as f32, self.input.last_mouse_y as f32),
                            button: match button {
                                winit::event::ButtonSource::Mouse(MouseButton::Right) => egui::PointerButton::Secondary,
                                winit::event::ButtonSource::Mouse(MouseButton::Middle) => egui::PointerButton::Middle,
                                _ => egui::PointerButton::Primary,
                            },
                            pressed,
                            modifiers: Default::default(),
                        });
                    }
                    WindowEvent::PointerMoved { source, position, .. } => {
                        let is_touch = matches!(source, winit::event::PointerSource::Touch { .. });
                        if let winit::event::PointerSource::Touch { finger_id, .. } = source {
                            let id = finger_id.into_raw() as u64;
                            self.input.active_touches.insert(id, (position.x, position.y));
                        }

                        if is_touch {
                            if let Some((_, sx, sy)) = self.input.map_touch_start {
                                let dx = position.x - sx;
                                let dy = position.y - sy;
                                if dx * dx + dy * dy > 400.0 {
                                    self.input.map_touch_start = None;
                                }
                            }
                        }

                        if self.input.active_touches.len() >= 2 {
                            self.input.dragging = false; // Cancel map drag while pinching
                            let mut it = self.input.active_touches.values();
                            let p1 = *it.next().unwrap();
                            let p2 = *it.next().unwrap();
                            let dx = p1.0 - p2.0;
                            let dy = p1.1 - p2.1;
                            let distance = (dx * dx + dy * dy).sqrt();

                            if let Some(last_dist) = self.input.last_pinch_distance {
                                let delta = distance - last_dist;
                                let old_zoom = self.input.camera_zoom;
                                self.input.camera_zoom *= 1.0 + (delta as f32 * 0.005);
                                let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h);
                                self.input.camera_zoom = self.input.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);

                                let pinch_cx = (p1.0 + p2.0) / 2.0;
                                let pinch_cy = (p1.1 + p2.1) / 2.0;
                                let map_x = (pinch_cx as f32 - self.input.camera_x) / old_zoom;
                                let map_y = (pinch_cy as f32 - self.input.camera_y) / old_zoom;
                                self.input.camera_x = pinch_cx as f32 - map_x * self.input.camera_zoom;
                                self.input.camera_y = pinch_cy as f32 - map_y * self.input.camera_zoom;
                            }
                            self.input.last_pinch_distance = Some(distance);
                        } else {
                            if self.input.dragging && (!is_touch || !self.ui.egui_ctx.egui_wants_pointer_input()) {
                                let dx = position.x - self.input.last_mouse_x;
                                let dy = position.y - self.input.last_mouse_y;
                                self.input.camera_x += dx as f32;
                                self.input.camera_y += dy as f32;
                            }
                        }
                        self.input.last_mouse_x = position.x;
                        self.input.last_mouse_y = position.y;
                        self.ui.raw_input.events.push(egui::Event::PointerMoved(Pos2::new(self.input.last_mouse_x as f32, self.input.last_mouse_y as f32)));
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        let scroll = match delta {
                            MouseScrollDelta::LineDelta(x, y) => {
                                if y.abs() >= x.abs() {
                                    y
                                } else {
                                    x
                                }
                            }
                            MouseScrollDelta::PixelDelta(pos) => {
                                let x = pos.x as f32 / 50.0;
                                let y = pos.y as f32 / 50.0;
                                if y.abs() >= x.abs() {
                                    y
                                } else {
                                    x
                                }
                            }
                        };
                        let old_zoom = self.input.camera_zoom;
                        self.input.camera_zoom *= 1.0 + scroll * 0.15;
                        let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h);
                        self.input.camera_zoom = self.input.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);

                        let factor = self.input.camera_zoom / old_zoom;
                        self.input.camera_x = self.input.last_mouse_x as f32 - factor * (self.input.last_mouse_x as f32 - self.input.camera_x);
                        self.input.camera_y = self.input.last_mouse_y as f32 - factor * (self.input.last_mouse_y as f32 - self.input.camera_y);
                    }

            _ => {}
        }
    }
}
