use crate::app::SowApp;
use crate::{camera_zoom_upper_bound, CAMERA_MIN_ZOOM};
use blade_graphics as gpu;
use egui::{Pos2, Rect, Vec2};
use sow_ui::app::ClientPhase;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};

impl SowApp {
    pub fn handle_window_event(
        &mut self,
        event_loop: &dyn winit::event_loop::ActiveEventLoop,
        event: WindowEvent,
    ) {
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
                    self.gfx
                        .render_ctx
                        .context
                        .destroy_command_encoder(&mut self.gfx.render_ctx.command_encoder);
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
                        let display_sync = if cfg!(any(target_os = "android", target_os = "ios")) {
                            gpu::DisplaySync::Block
                        } else {
                            gpu::DisplaySync::Tear
                        };
                        self.gfx.render_ctx.context.reconfigure_surface(
                            s,
                            gpu::SurfaceConfig {
                                size: gpu::Extent {
                                    width: physical_size.width,
                                    height: physical_size.height,
                                    depth: 1,
                                },
                                usage: gpu::TextureUsage::TARGET,
                                display_sync,
                                color_space: gpu::ColorSpace::Srgb,
                                ..Default::default()
                            },
                        );
                    }
                    self.input.screen_w = physical_size.width as f32;
                    self.input.screen_h = physical_size.height as f32;
                    let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h);
                    self.input.camera_zoom = self.input.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);
                    self.ui.raw_input.screen_rect = Some(Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(self.input.screen_w, self.input.screen_h),
                    ));
                    if let Some(win) = self.gfx.window.as_ref() {
                        win.request_redraw();
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if pressed
                    && !self.ui.egui_ctx.egui_wants_keyboard_input()
                    && self.ui.app.phase == ClientPhase::Playing
                    && self.ui.app.hud_state.sync_state.is_none()
                {
                    if let winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyB) =
                        event.physical_key
                    {
                        let world_x = (self.input.last_mouse_x as f32 - self.input.camera_x)
                            / self.input.camera_zoom;
                        let world_y = (self.input.last_mouse_y as f32 - self.input.camera_y)
                            / self.input.camera_zoom;
                        let col = world_x.floor() as i32;
                        let row = world_y.floor() as i32;
                        if col >= 0
                            && row >= 0
                            && col < self.sim.map_w as i32
                            && row < self.sim.map_h as i32
                        {
                            let idx = (row * self.sim.map_w as i32 + col) as usize;

                            let troops = Some(
                                self.ui.app.hud_state.troops
                                    * (self.ui.app.hud_state.attack_ratio as f64),
                            );
                            let intent = sow_core::protocol::GameplayIntent::LaunchFleet {
                                target_tile: idx as u32,
                                troops,
                            };

                            let owner = self.gfx.map_renderer.as_ref().map(|mr| mr.owners[idx]).unwrap_or(0);
                            let my_id = self.sim.my_player_id.unwrap_or(0);
                            let is_betrayer = self.sim.current_snapshot.as_ref()
                                .and_then(|s| s.players.iter().find(|p| p.id == owner))
                                .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
                                .unwrap_or(false);
                            let is_allied = self.sim.current_snapshot.as_ref()
                                .and_then(|s| s.players.iter().find(|p| p.id == my_id))
                                .map(|p| p.alliances.contains(&owner) && !is_betrayer)
                                .unwrap_or(false);

                            if owner != 0 && owner != my_id && is_allied {
                                let mx = self.input.last_mouse_x;
                                let my = self.input.last_mouse_y;
                                self.open_context_menu_at(mx, my);
                            } else {
                                if let Some(c) = self.net.client.as_ref() {
                                    if let Ok(json) = bincode::serialize(
                                        &sow_core::protocol::ClientMessage::Gameplay {
                                            intent: intent.clone(),
                                        },
                                    ) {
                                        c.send(json);
                                    }
                                } else {
                                    self.sim.offline_intents.push(intent);
                                }
                            }
                        }
                    }

                    // Building hotkeys 1-6, Nuke hotkeys 7-9 (OpenFrontIO-style)
                    if let winit::keyboard::PhysicalKey::Code(code) = event.physical_key {
                        let building = match code {
                            winit::keyboard::KeyCode::Digit1 | winit::keyboard::KeyCode::Numpad1 => Some(sow_core::game::BuildingKind::City),
                            winit::keyboard::KeyCode::Digit2 | winit::keyboard::KeyCode::Numpad2 => Some(sow_core::game::BuildingKind::Factory),
                            winit::keyboard::KeyCode::Digit3 | winit::keyboard::KeyCode::Numpad3 => Some(sow_core::game::BuildingKind::Port),
                            winit::keyboard::KeyCode::Digit4 | winit::keyboard::KeyCode::Numpad4 => Some(sow_core::game::BuildingKind::DefensePost),
                            winit::keyboard::KeyCode::Digit5 | winit::keyboard::KeyCode::Numpad5 => Some(sow_core::game::BuildingKind::SamLauncher),
                            winit::keyboard::KeyCode::Digit6 | winit::keyboard::KeyCode::Numpad6 => Some(sow_core::game::BuildingKind::MissileSilo),
                            _ => None,
                        };
                        if let Some(kind) = building {
                            if self.ui.app.hud_state.selected_building_kind == Some(kind) {
                                self.ui.app.hud_state.selected_building_kind = None;
                            } else {
                                self.ui.app.hud_state.selected_building_kind = Some(kind);
                                self.ui.app.hud_state.selected_nuke_kind = None;
                            }
                        }

                        let nuke = match code {
                            winit::keyboard::KeyCode::Digit7 | winit::keyboard::KeyCode::Numpad7 => Some(sow_core::game::NukeKind::AtomBomb),
                            winit::keyboard::KeyCode::Digit8 | winit::keyboard::KeyCode::Numpad8 => Some(sow_core::game::NukeKind::HydrogenBomb),
                            winit::keyboard::KeyCode::Digit9 | winit::keyboard::KeyCode::Numpad9 => Some(sow_core::game::NukeKind::MIRV),
                            _ => None,
                        };
                        if let Some(kind) = nuke {
                            if self.ui.app.hud_state.selected_nuke_kind == Some(kind) {
                                self.ui.app.hud_state.selected_nuke_kind = None;
                            } else {
                                self.ui.app.hud_state.selected_nuke_kind = Some(kind);
                                self.ui.app.hud_state.selected_building_kind = None;
                            }
                        }
                    }
                }

                if pressed {
                    if let winit::keyboard::Key::Character(text) = &event.logical_key {
                        let key_str = text.as_str();
                        let egui_key = match key_str {
                            "c" | "C" => Some(egui::Key::C),
                            "v" | "V" => Some(egui::Key::V),
                            "x" | "X" => Some(egui::Key::X),
                            "a" | "A" => Some(egui::Key::A),
                            "z" | "Z" => Some(egui::Key::Z),
                            _ => None,
                        };
                        if let Some(key) = egui_key {
                            self.ui.raw_input.events.push(egui::Event::Key {
                                key,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: self.ui.raw_input.modifiers,
                            });
                        }
                        self.ui
                            .raw_input
                            .events
                            .push(egui::Event::Text(text.to_string()));
                    } else if let winit::keyboard::Key::Named(named) = &event.logical_key {
                        if *named == winit::keyboard::NamedKey::Backspace {
                            self.ui.raw_input.events.push(egui::Event::Key {
                                key: egui::Key::Backspace,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: self.ui.raw_input.modifiers,
                            });
                        } else if *named == winit::keyboard::NamedKey::Escape {
                            self.ui.app.hud_state.selected_building_kind = None;
                            self.ui.app.hud_state.selected_nuke_kind = None;
                            self.ui.raw_input.events.push(egui::Event::Key {
                                key: egui::Key::Escape,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: self.ui.raw_input.modifiers,
                            });
                        }
                    }
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.ui.raw_input.modifiers.alt = modifiers.state().alt_key();
                self.ui.raw_input.modifiers.ctrl = modifiers.state().control_key();
                self.ui.raw_input.modifiers.shift = modifiers.state().shift_key();
                self.ui.raw_input.modifiers.mac_cmd = modifiers.state().meta_key();
                self.ui.raw_input.modifiers.command =
                    self.ui.raw_input.modifiers.ctrl || self.ui.raw_input.modifiers.mac_cmd;
            }
            WindowEvent::Ime(ime) => {
                use winit::event::Ime;
                match ime {
                    Ime::Enabled | Ime::Disabled | Ime::DeleteSurrounding { .. } => {}
                    Ime::Preedit(text, _) => {
                        self.ui
                            .raw_input
                            .events
                            .push(egui::Event::Ime(egui::ImeEvent::Preedit(text.clone())));
                    }
                    Ime::Commit(text) => {
                        self.ui
                            .raw_input
                            .events
                            .push(egui::Event::Ime(egui::ImeEvent::Commit(text.clone())));
                    }
                }
            }
            WindowEvent::PointerButton {
                state: btn_state,
                button,
                position,
                primary,
                ..
            } => {
                let pressed = btn_state == ElementState::Pressed;
                if primary {
                    self.input.last_mouse_x = position.x;
                    self.input.last_mouse_y = position.y;
                }

                let is_primary_action = match button {
                    winit::event::ButtonSource::Mouse(b) => b == MouseButton::Left,
                    winit::event::ButtonSource::Touch { .. } => primary,
                    _ => false,
                };
                let is_secondary = match button {
                    winit::event::ButtonSource::Mouse(b) => b == MouseButton::Right,
                    _ => false,
                };
                let is_touch = matches!(button, winit::event::ButtonSource::Touch { .. });

                if let winit::event::ButtonSource::Touch { finger_id, .. } = button {
                    let id = finger_id.into_raw() as u64;
                    if pressed {
                        self.input
                            .active_touches
                            .insert(id, (position.x, position.y));
                    } else {
                        self.input.active_touches.remove(&id);
                        if self.input.active_touches.len() < 2 {
                            self.input.last_pinch_state = None;
                        }
                    }
                }

                let wants_pointer = self.ui.egui_ctx.egui_wants_pointer_input();
                let in_game = self.ui.app.phase == ClientPhase::Playing
                    && self.ui.app.hud_state.sync_state.is_none();

                if is_primary_action {
                    if pressed {
                        if !wants_pointer {
                            self.input.dragging = true;
                        }
                        // Start hold-to-attack tracking
                        if !wants_pointer && in_game {
                            self.input.map_touch_start =
                                Some((web_time::Instant::now(), position.x, position.y));
                            self.try_begin_hold_attack(position.x, position.y, is_touch);
                        }
                    } else {
                        self.input.dragging = false;
                        // On release: if it was a quick tap, handle click actions
                        if !wants_pointer && in_game {
                            let was_quick = self
                                .input
                                .map_touch_start
                                .map(|(t, _, _)| t.elapsed().as_millis() < 300)
                                .unwrap_or(false);
                            let no_drift = self
                                .input
                                .map_touch_start
                                .map(|(_, sx, sy)| {
                                    let dx = position.x - sx;
                                    let dy = position.y - sy;
                                    dx * dx + dy * dy <= 400.0
                                })
                                .unwrap_or(false);

                            if was_quick && no_drift {
                                let is_spawning = self
                                    .sim
                                    .current_snapshot
                                    .as_ref()
                                    .map(|s| {
                                        matches!(
                                            s.phase,
                                            sow_core::game::GamePhase::Spawning { .. }
                                        )
                                    })
                                    .unwrap_or(false);
                                let (sx, sy) = self
                                    .input
                                    .map_touch_start
                                    .map(|(_, x, y)| (x, y))
                                    .unwrap_or((position.x, position.y));
                                let is_building = self.ui.app.hud_state.selected_building_kind.is_some();
                                if is_touch && !is_spawning && !is_building {
                                    // Tap on mobile → open context menu
                                    self.open_context_menu_at(sx, sy);
                                } else {
                                    // Quick click on desktop or tap during spawn/build → one-shot action
                                    self.handle_map_click(sx, sy);
                                }
                            }
                        }
                        self.input.hold_attack_target = None;
                        self.input.hold_attack_accum = 0.0;
                        self.input.map_touch_start = None;
                    }
                }

                // Right-click on desktop → cancel placement mode or move warships or open context menu
                if is_secondary && !pressed && !wants_pointer && in_game {
                    if self.ui.app.hud_state.selected_building_kind.is_some() {
                        self.ui.app.hud_state.selected_building_kind = None;
                    } else if self.ui.app.hud_state.selected_nuke_kind.is_some() {
                        self.ui.app.hud_state.selected_nuke_kind = None;
                    } else if !self.input.selected_warships.is_empty() {
                        let world_x = (position.x as f32 - self.input.camera_x) / self.input.camera_zoom;
                        let world_y = (position.y as f32 - self.input.camera_y) / self.input.camera_zoom;
                        let col = world_x.floor() as i32;
                        let row = world_y.floor() as i32;
                        if col >= 0 && row >= 0 && col < self.sim.map_w as i32 && row < self.sim.map_h as i32 {
                            let target_tile = (row * self.sim.map_w as i32 + col) as u32;
                            let intent = sow_core::protocol::GameplayIntent::MoveWarships {
                                unit_ids: self.input.selected_warships.clone(),
                                target_tile,
                            };
                            self.send_intent(intent);
                        }
                    } else {
                        self.open_context_menu_at(position.x, position.y);
                    }
                }

                if primary {
                    self.ui.raw_input.events.push(egui::Event::PointerButton {
                        pos: Pos2::new(
                            self.input.last_mouse_x as f32,
                            self.input.last_mouse_y as f32,
                        ),
                        button: match button {
                            winit::event::ButtonSource::Mouse(MouseButton::Right) => {
                                egui::PointerButton::Secondary
                            }
                            winit::event::ButtonSource::Mouse(MouseButton::Middle) => {
                                egui::PointerButton::Middle
                            }
                            _ => egui::PointerButton::Primary,
                        },
                        pressed,
                        modifiers: Default::default(),
                    });
                }
            }
            WindowEvent::PointerMoved {
                source, position, primary, ..
            } => {
                let is_touch = matches!(source, winit::event::PointerSource::Touch { .. });
                if let winit::event::PointerSource::Touch { finger_id, .. } = source {
                    let id = finger_id.into_raw() as u64;
                    self.input
                        .active_touches
                        .insert(id, (position.x, position.y));
                }

                if is_touch {
                    if let Some((_, sx, sy)) = self.input.map_touch_start {
                        let dx = position.x - sx;
                        let dy = position.y - sy;
                        if dx * dx + dy * dy > 400.0 {
                            self.input.map_touch_start = None;
                            self.input.hold_attack_target = None;
                            self.input.hold_attack_accum = 0.0;
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
                    let pinch_cx = (p1.0 + p2.0) / 2.0;
                    let pinch_cy = (p1.1 + p2.1) / 2.0;

                    if let Some((last_dist, last_cx, last_cy)) = self.input.last_pinch_state {
                        let delta_dist = distance - last_dist;
                        let delta_x = pinch_cx - last_cx;
                        let delta_y = pinch_cy - last_cy;
                        
                        self.input.camera_x += delta_x as f32;
                        self.input.camera_y += delta_y as f32;

                        self.process_camera_zoom(
                            1.0 + (delta_dist as f32 * 0.005),
                            pinch_cx as f32,
                            pinch_cy as f32,
                        );
                    }
                    self.input.last_pinch_state = Some((distance, pinch_cx, pinch_cy));
                } else if primary {
                    if self.input.dragging
                        && (!is_touch || !self.ui.egui_ctx.egui_wants_pointer_input())
                    {
                        let dx = position.x - self.input.last_mouse_x;
                        let dy = position.y - self.input.last_mouse_y;
                        self.input.camera_x += dx as f32;
                        self.input.camera_y += dy as f32;
                    }
                }
                
                if primary {
                    self.input.last_mouse_x = position.x;
                    self.input.last_mouse_y = position.y;
                    self.ui
                        .raw_input
                        .events
                        .push(egui::Event::PointerMoved(Pos2::new(
                            self.input.last_mouse_x as f32,
                            self.input.last_mouse_y as f32,
                        )));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !self.input.active_touches.is_empty() {
                    return;
                }
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
                self.process_camera_zoom(
                    1.0 + scroll * 0.15,
                    self.input.last_mouse_x as f32,
                    self.input.last_mouse_y as f32,
                );
            }

            _ => {}
        }
    }

    fn try_begin_hold_attack(&mut self, x: f64, y: f64, is_touch: bool) {
        let phase = self
            .sim
            .current_snapshot
            .as_ref()
            .map(|s| &s.phase)
            .unwrap_or(&sow_core::game::GamePhase::Lobby);
        if matches!(phase, sow_core::game::GamePhase::Spawning { .. }) {
            return;
        }

        let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
        let world_y = (y as f32 - self.input.camera_y) / self.input.camera_zoom;
        let col = world_x.floor() as i32;
        let row = world_y.floor() as i32;
        if col < 0 || row < 0 || col >= self.sim.map_w as i32 || row >= self.sim.map_h as i32 {
            return;
        }
        let idx = (row * self.sim.map_w as i32 + col) as usize;
        let owner = self
            .gfx
            .map_renderer
            .as_ref()
            .map(|mr| mr.owners[idx])
            .unwrap_or(0);
        let terrain_byte = self
            .gfx
            .map_renderer
            .as_ref()
            .map(|mr| mr.terrain[idx])
            .unwrap_or(0);
        let is_land = (terrain_byte & 0x80) != 0;
        let my_id = self.sim.my_player_id.unwrap_or(0);

        if is_land && owner != my_id {
            let is_betrayer = self.sim.current_snapshot.as_ref()
                .and_then(|s| s.players.iter().find(|p| p.id == owner))
                .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
                .unwrap_or(false);
            let is_allied = self.sim.current_snapshot.as_ref()
                .and_then(|s| s.players.iter().find(|p| p.id == my_id))
                .map(|p| p.alliances.contains(&owner) && !is_betrayer)
                .unwrap_or(false);

            let troops = self.ui.app.hud_state.troops * (self.ui.app.hud_state.attack_ratio as f64);
            let attack = sow_core::protocol::AttackIntent {
                target_owner: owner,
                troops: Some(troops),
            };
            let intent = sow_core::protocol::GameplayIntent::Attack(attack);

            if is_allied {
                // Intercept and open context menu instead
                self.open_context_menu_at(x, y);
            } else {
                if !is_touch {
                    // Desktop: fire immediately
                    self.send_intent(intent);
                    self.input.hold_attack_target = Some((owner, web_time::Instant::now(), x, y, true));
                } else {
                    // Mobile: wait for hold to distinguish from tap (context menu)
                    self.input.hold_attack_target =
                        Some((owner, web_time::Instant::now(), x, y, false));
                }
            }
            self.input.hold_attack_accum = 0.0;
        }
    }

    pub(crate) fn open_context_menu_at(&mut self, x: f64, y: f64) {
        let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
        let world_y = (y as f32 - self.input.camera_y) / self.input.camera_zoom;
        let col = world_x.floor() as i32;
        let row = world_y.floor() as i32;
        if col >= 0 && row >= 0 && col < self.sim.map_w as i32 && row < self.sim.map_h as i32 {
            let idx = (row * self.sim.map_w as i32 + col) as u32;
            
            // Clear any prior menu state first to avoid animation caching issues
            self.input.map_context_menu = None;
            self.input.map_context_menu_active = None;
            self.input.context_menu_timer = 0.0;
            self.input.context_menu_open_time = Some(web_time::Instant::now());
            self.input.map_context_menu_session += 1;
            
            self.input.map_context_menu = Some((x as f32, y as f32, idx));
        }
    }

    fn handle_map_click(&mut self, x: f64, y: f64) {
        let phase = self
            .sim
            .current_snapshot
            .as_ref()
            .map(|s| &s.phase)
            .unwrap_or(&sow_core::game::GamePhase::Lobby);

        let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
        let world_y = (y as f32 - self.input.camera_y) / self.input.camera_zoom;
        let col = world_x.floor() as i32;
        let row = world_y.floor() as i32;
        if col < 0 || row < 0 || col >= self.sim.map_w as i32 || row >= self.sim.map_h as i32 {
            return;
        }

        if matches!(phase, sow_core::game::GamePhase::Spawning { .. }) {
            let intent = sow_core::protocol::GameplayIntent::Spawn {
                x: col as u32,
                y: row as u32,
            };
            self.send_intent(intent);
        } else if let Some(nuke_kind) = self.ui.app.hud_state.selected_nuke_kind {
            let tile_idx = (row * self.sim.map_w as i32 + col) as u32;
            let intent = sow_core::protocol::GameplayIntent::LaunchNuke {
                kind: nuke_kind,
                target_tile: tile_idx,
            };
            self.send_intent(intent);
            self.ui.app.hud_state.selected_nuke_kind = None;
        } else if let Some(kind) = self.ui.app.hud_state.selected_building_kind {
            let map_w = self.sim.map_w;
            let map_h = self.sim.map_h;
            let owners = self.gfx.map_renderer.as_ref().map(|mr| mr.owners.as_slice()).unwrap_or(&[]);
            let terrain = self.gfx.map_renderer.as_ref().map(|mr| mr.terrain.as_slice()).unwrap_or(&[]);
            let my_id = self.sim.my_player_id.unwrap_or(0);
            let buildings = self.sim.current_snapshot.as_ref().map(|s| s.buildings.as_slice()).unwrap_or(&[]);

            // Check if there is a valid upgrade target within Manhattan distance STRUCTURE_MIN_DIST of (col, row)
            let mut upgrade_target = None;
            if kind.upgradable() {
                let min_dist = sow_core::building::STRUCTURE_MIN_DIST;
                let mut best_dist = 999;
                for b in buildings {
                    if b.owner_id == my_id && b.kind == kind && !b.under_construction {
                        let bx = (b.tile_idx % map_w) as i32;
                        let by = (b.tile_idx / map_w) as i32;
                        let d = (col - bx).abs() + (row - by).abs(); // Manhattan distance
                        if d <= min_dist {
                            if d < best_dist || (d == best_dist && upgrade_target.map_or(true, |old_id| b.id < old_id)) {
                                best_dist = d;
                                upgrade_target = Some(b.id);
                            }
                        }
                    }
                }
            }

            let cost = {
                let i = sow_core::game::BuildingKind::ALL.iter().position(|&k| k == kind).unwrap_or(0);
                self.ui.app.hud_state.building_costs[i]
            };

            if let Some(target_id) = upgrade_target {
                if self.ui.app.hud_state.gold < cost {
                    self.ui.app.hud_state.show_error = Some(format!("Not enough Gold! You need {}.", cost));
                } else {
                    let intent = sow_core::protocol::GameplayIntent::UpgradeStructure {
                        building_id: target_id,
                    };
                    self.send_intent(intent);
                }
                self.ui.app.hud_state.selected_building_kind = None;
                return;
            }

            let snapped_res = resolve_building_placement_tile(
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

            let cost = {
                let i = sow_core::game::BuildingKind::ALL.iter().position(|&k| k == kind).unwrap_or(0);
                self.ui.app.hud_state.building_costs[i]
            };

            let mut valid = true;
            let mut err_msg = String::new();

            if self.ui.app.hud_state.gold < cost {
                valid = false;
                err_msg = format!("Need {} Gold!", cost);
            } else {
                match snapped_res {
                    Ok(_) => {}
                    Err(msg) => {
                        valid = false;
                        err_msg = msg.to_string();
                    }
                }
            }

            if !valid {
                self.ui.app.hud_state.show_error = Some(err_msg);
                self.ui.app.hud_state.selected_building_kind = None;
                return;
            }

            let intent = sow_core::protocol::GameplayIntent::BuildStructure {
                kind,
                target_tile: snapped_res.unwrap(),
            };
            self.send_intent(intent);
            self.ui.app.hud_state.selected_building_kind = None;
        } else {
            // Check if we clicked on a Warship we own
            let mut clicked_warships = Vec::new();
            if let Some(snap) = &self.sim.current_snapshot {
                let my_pid = self.sim.my_player_id.unwrap_or(0);
                for f in &snap.fleets {
                    if f.unit_type == sow_core::game::UnitType::Warship && f.owner_id == my_pid {
                        let wx = (f.current_tile % self.sim.map_w) as f32;
                        let wy = (f.current_tile / self.sim.map_w) as f32;
                        // Click tolerance (half a tile)
                        if (wx + 0.5 - world_x).abs() < 0.5 && (wy + 0.5 - world_y).abs() < 0.5 {
                            clicked_warships.push(f.id);
                        }
                    }
                }
            }
            if !clicked_warships.is_empty() {
                self.input.selected_warships = clicked_warships;
            } else {
                self.input.selected_warships.clear();
            }
        }
    }

    pub(crate) fn send_intent(&mut self, intent: sow_core::protocol::GameplayIntent) {
        if let Some(c) = self.net.client.as_ref() {
            let msg = sow_core::protocol::ClientMessage::Gameplay {
                intent: intent.clone(),
            };
            if let Ok(json) = bincode::serialize(&msg) {
                c.send(json);
            }
        } else {
            self.sim.offline_intents.push(intent);
        }
    }

    pub(crate) fn process_camera_zoom(&mut self, zoom_factor: f32, cx: f32, cy: f32) {
        let old_zoom = self.input.camera_zoom;
        self.input.camera_zoom *= zoom_factor;
        let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h);
        self.input.camera_zoom = self.input.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);

        let map_x = (cx - self.input.camera_x) / old_zoom;
        let map_y = (cy - self.input.camera_y) / old_zoom;
        self.input.camera_x = cx - map_x * self.input.camera_zoom;
        self.input.camera_y = cy - map_y * self.input.camera_zoom;
    }
}

pub fn resolve_building_placement_tile(
    kind: sow_core::game::BuildingKind,
    click_x: i32,
    click_y: i32,
    map_w: u32,
    map_h: u32,
    owners: &[u16],
    terrain: &[u8],
    my_id: u16,
    buildings: &[sow_core::protocol::BuildingSnapshot],
) -> Result<u32, &'static str> {
    let min_dist = sow_core::building::STRUCTURE_MIN_DIST;
    let min_dist_sq = min_dist * min_dist;
    
    // Snapping search radius: extremely forgiving (Poka Yoke) on mobile
    let pokayoke_dist = match kind {
        sow_core::game::BuildingKind::Port => 35, // extremely forgiving shoreline search for Port
        _ => 25, // forgiving search for other structures
    };
    let pokayoke_dist_sq = pokayoke_dist * pokayoke_dist;
    let max_search_dist = pokayoke_dist + min_dist;
    let max_search_dist_sq = max_search_dist * max_search_dist;

    // Filter buildings to those close to the click target to optimize distance checks
    let nearby_buildings: Vec<_> = buildings
        .iter()
        .filter(|b| {
            let bx = (b.tile_idx % map_w) as i32;
            let by = (b.tile_idx / map_w) as i32;
            let bdx = click_x - bx;
            let bdy = click_y - by;
            (bdx * bdx + bdy * bdy) < max_search_dist_sq
        })
        .collect();

    // Diagnostic flags to identify exact failure reasons
    let mut found_any_owned = false;
    let mut found_any_land = false;
    let mut found_any_far_enough = false;
    let mut found_any_shoreline = false;

    // 1. Gather valid land structure tiles within pokayoke_dist of click target
    let mut valid_land_tiles = Vec::new();
    for dy in -pokayoke_dist..=pokayoke_dist {
        for dx in -pokayoke_dist..=pokayoke_dist {
            let tx = click_x + dx;
            let ty = click_y + dy;
            if tx < 0 || tx >= map_w as i32 || ty < 0 || ty >= map_h as i32 {
                continue;
            }
            if (dx * dx + dy * dy) >= pokayoke_dist_sq { // Euclidean distance limit
                continue;
            }
            let tile_idx = (ty * map_w as i32 + tx) as u32;
            
            // Check ownership
            if owners.get(tile_idx as usize).copied().unwrap_or(0) != my_id {
                continue;
            }
            found_any_owned = true;
            
            // Check land (bit 7: is_land)
            let tile_terrain = terrain.get(tile_idx as usize).copied().unwrap_or(0);
            let is_land = (tile_terrain & 0x80) != 0;
            if !is_land {
                continue;
            }
            found_any_land = true;
            
            // Check minimum distance from existing buildings
            let mut too_close = false;
            for b in &nearby_buildings {
                let bx = (b.tile_idx % map_w) as i32;
                let by = (b.tile_idx / map_w) as i32;
                let bdx = tx - bx;
                let bdy = ty - by;
                if (bdx * bdx + bdy * bdy) < min_dist_sq {
                    too_close = true;
                    break;
                }
            }
            if too_close {
                continue;
            }
            found_any_far_enough = true;
            
            valid_land_tiles.push((tx, ty, tile_idx));
        }
    }
    
    if valid_land_tiles.is_empty() {
        if !found_any_owned {
            return Err("Target area must be inside your owned territory!");
        }
        if !found_any_land {
            return Err("Structures can only be built on land territory!");
        }
        if !found_any_far_enough {
            return Err("Too close to another structure! Minimum spacing is 8 tiles.");
        }
        return Err("No space nearby!");
    }
    
    match kind {
        sow_core::game::BuildingKind::Port => {
            let mut candidates = Vec::new();
            for &(tx, ty, tile_idx) in &valid_land_tiles {
                let tile_terrain = terrain.get(tile_idx as usize).copied().unwrap_or(0);
                let is_shoreline = (tile_terrain & 0x40) != 0;
                if is_shoreline {
                    found_any_shoreline = true;
                    let dist = (tx - click_x).abs() + (ty - click_y).abs();
                    candidates.push((tx, ty, tile_idx, dist));
                }
            }
            
            if candidates.is_empty() {
                if !found_any_shoreline {
                    return Err("No shoreline here! Ports must be placed on coastal tiles next to water.");
                }
                return Err("No valid shoreline found for Port!");
            }
            
            // Sort candidates by Manhattan distance, then by tile index
            candidates.sort_by(|a, b| {
                a.3.cmp(&b.3).then_with(|| a.2.cmp(&b.2))
            });
            Ok(candidates.first().map(|&(_, _, idx, _)| idx).unwrap())
        }
        _ => {
            // For other structures, find the closest valid land tile to click target by Euclidean distance
            valid_land_tiles.sort_by(|a, b| {
                let da = (a.0 - click_x) * (a.0 - click_x) + (a.1 - click_y) * (a.1 - click_y);
                let db = (b.0 - click_x) * (b.0 - click_x) + (b.1 - click_y) * (b.1 - click_y);
                da.cmp(&db).then_with(|| a.2.cmp(&b.2))
            });
            Ok(valid_land_tiles.first().map(|&(_, _, idx)| idx).unwrap())
        }
    }
}

