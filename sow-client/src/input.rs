use crate::app::SowApp;
use crate::render::world::movers::world_to_tile;
use crate::{camera_zoom_upper_bound, CAMERA_MIN_ZOOM};
use blade_graphics as gpu;
use egui::{Pos2, Vec2};
use sow_ui::app::ClientPhase;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};

impl SowApp {
    fn mouse_to_tile(&self, x: f64, y: f64) -> Option<(i32, i32)> {
        let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
        let world_y = (y as f32 - self.input.camera_y) / self.input.camera_zoom;

        let (col, row) = world_to_tile(world_x, world_y);

        if col >= 0 && row >= 0 && col < self.sim.map_w as i32 && row < self.sim.map_h as i32 {
            Some((col, row))
        } else {
            None
        }
    }

    pub(crate) fn apply_surface_resize(
        &mut self,
        physical_size: winit::dpi::PhysicalSize<u32>,
        force_reconfigure: bool,
    ) {
        if physical_size.width == 0 || physical_size.height == 0 {
            return;
        }
        let sf = self
            .gfx
            .window
            .as_ref()
            .map_or(1.0, |w| w.scale_factor() as f32)
            .max(0.01);
        let vp = crate::viewport::Viewport {
            physical: physical_size,
            scale_factor: sf,
            logical: Vec2::new(
                physical_size.width as f32 / sf,
                physical_size.height as f32 / sf,
            ),
        };
        let needs_reconfigure = vp.wants_reconfigure(self) || force_reconfigure;

        let recreate_surface = needs_reconfigure
            && cfg!(any(target_os = "android", target_os = "ios"))
            && self.gfx.surface.is_some()
            && vp.orientation_flipped(self);

        if recreate_surface {
            if let Some(render_ctx) = self.gfx.render_ctx.as_mut() {
                if let Some(sp) = self.gfx.prev_sync_point.take() {
                    let _ = render_ctx.context.wait_for(&sp, !0);
                }
                if let Some(mut s) = self.gfx.surface.take() {
                    render_ctx.context.destroy_surface(&mut s);
                }
            }
        } else if needs_reconfigure {
            if let Some(render_ctx) = self.gfx.render_ctx.as_mut() {
                if let Some(sp) = self.gfx.prev_sync_point.take() {
                    let _ = render_ctx.context.wait_for(&sp, !0);
                }
                if let Some(ref mut s) = self.gfx.surface {
                    let display_sync = if cfg!(any(target_os = "android", target_os = "ios")) {
                        gpu::DisplaySync::Block
                    } else {
                        gpu::DisplaySync::Tear
                    };
                    render_ctx.context.reconfigure_surface(
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
            }
        }

        if needs_reconfigure || recreate_surface {
            self.gfx.configured_physical = physical_size;
        }

        crate::viewport::Viewport::from_configured(self, sf).sync_to_app(self);
        let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h);
        self.input.camera_zoom = self.input.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);

        if recreate_surface {
            self.check_surface();
        }
        if needs_reconfigure {
            if let Some(win) = self.gfx.window.as_ref() {
                win.request_redraw();
            }
        }
    }

    pub fn handle_window_event(
        &mut self,
        event_loop: &dyn winit::event_loop::ActiveEventLoop,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                if let Some(render_ctx) = self.gfx.render_ctx.as_mut() {
                    if let Some(sp) = self.gfx.prev_sync_point.take() {
                        let _ = render_ctx.context.wait_for(&sp, !0);
                    }
                    if let Some(mut s) = self.gfx.surface.take() {
                        if let Some(mut gp) = self.gfx.gui_painter.take() {
                            gp.destroy(&render_ctx.context);
                        }
                        if let Some(mut mr) = self.gfx.map_renderer.take() {
                            mr.destroy(render_ctx);
                        }
                        render_ctx.reset_command_encoder();
                        render_ctx.context.destroy_surface(&mut s);
                    }
                }
                event_loop.exit()
            }
            WindowEvent::SurfaceResized(physical_size) => {
                self.apply_surface_resize(physical_size, false);
                if let Some(win) = self.gfx.window.as_ref() {
                    win.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let vp = self
                    .gfx
                    .window
                    .as_ref()
                    .map(|win| crate::viewport::Viewport::measure(win.as_ref()));
                if let Some(vp) = vp {
                    if vp.wants_reconfigure(self) {
                        self.apply_surface_resize(vp.physical, false);
                    } else {
                        crate::viewport::Viewport::from_configured(self, vp.scale_factor)
                            .sync_to_app(self);
                    }
                }
                if let Some(win) = self.gfx.window.as_ref() {
                    win.request_redraw();
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
                        if let Some((col, row)) =
                            self.mouse_to_tile(self.input.last_mouse_x, self.input.last_mouse_y)
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

                            let owner = self
                                .gfx
                                .map_renderer
                                .as_ref()
                                .map(|mr| mr.owners[idx])
                                .unwrap_or(0);
                            let my_id = self.sim.my_player_id.unwrap_or(0);
                            let is_betrayer = self
                                .sim
                                .current_snapshot
                                .as_ref()
                                .and_then(|s| s.players.iter().find(|p| p.id == owner))
                                .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
                                .unwrap_or(false);
                            let is_allied = self
                                .sim
                                .current_snapshot
                                .as_ref()
                                .and_then(|s| s.players.iter().find(|p| p.id == my_id))
                                .map(|p| p.alliances.contains(&owner) && !is_betrayer)
                                .unwrap_or(false);

                            if owner != 0 && owner != my_id && is_allied {
                                let lang = self.ui.app.settings_state.language;
                                self.ui.app.hud_state.show_error =
                                    Some(sow_i18n::get(lang).hud.err_break_alliance_boat.clone());
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

                    // Building hotkeys 1-6, Nuke hotkeys 8-0 (Redesigned)
                    if let winit::keyboard::PhysicalKey::Code(code) = event.physical_key {
                        let building = match code {
                            winit::keyboard::KeyCode::Digit1
                            | winit::keyboard::KeyCode::Numpad1 => {
                                Some(sow_core::game::BuildingKind::City)
                            }
                            winit::keyboard::KeyCode::Digit2
                            | winit::keyboard::KeyCode::Numpad2 => {
                                Some(sow_core::game::BuildingKind::Factory)
                            }
                            winit::keyboard::KeyCode::Digit3
                            | winit::keyboard::KeyCode::Numpad3 => {
                                Some(sow_core::game::BuildingKind::Port)
                            }
                            winit::keyboard::KeyCode::Digit4
                            | winit::keyboard::KeyCode::Numpad4 => {
                                Some(sow_core::game::BuildingKind::Bunker)
                            }
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
                            winit::keyboard::KeyCode::Digit0
                            | winit::keyboard::KeyCode::Numpad0 => {
                                Some(sow_core::game::NukeKind::AtomBomb)
                            }
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

                        // Deselect building/nuke tools (Escape exits browser fullscreen on portals).
                        if code == winit::keyboard::KeyCode::KeyQ
                            && (self.ui.app.hud_state.selected_building_kind.is_some()
                                || self.ui.app.hud_state.selected_nuke_kind.is_some())
                        {
                            self.ui.app.hud_state.selected_building_kind = None;
                            self.ui.app.hud_state.selected_nuke_kind = None;
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
                        } else if *named == winit::keyboard::NamedKey::Escape
                            && self.ui.app.phase != ClientPhase::Playing
                        {
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
                                let is_building =
                                    self.ui.app.hud_state.selected_building_kind.is_some();
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
                        let world_x =
                            (position.x as f32 - self.input.camera_x) / self.input.camera_zoom;
                        let world_y =
                            (position.y as f32 - self.input.camera_y) / self.input.camera_zoom;
                        let col = world_x.floor() as i32;
                        let row = world_y.floor() as i32;
                        if col >= 0
                            && row >= 0
                            && col < self.sim.map_w as i32
                            && row < self.sim.map_h as i32
                        {
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
                source,
                position,
                primary,
                ..
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
                } else if primary
                    && self.input.dragging
                    && (!is_touch || !self.ui.egui_ctx.egui_wants_pointer_input())
                {
                    let dx = position.x - self.input.last_mouse_x;
                    let dy = position.y - self.input.last_mouse_y;
                    self.input.camera_x += dx as f32;
                    self.input.camera_y += dy as f32;
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

                let wants_pointer = self.ui.egui_ctx.egui_wants_pointer_input();
                let in_game = self.ui.app.phase == ClientPhase::Playing;

                if wants_pointer || !in_game {
                    let (unit, vec_delta) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => {
                            (egui::MouseWheelUnit::Line, egui::vec2(x, y))
                        }
                        MouseScrollDelta::PixelDelta(pos) => {
                            let sf = self
                                .gfx
                                .window
                                .as_ref()
                                .map_or(1.0, |w| w.scale_factor() as f32);
                            (
                                egui::MouseWheelUnit::Point,
                                egui::vec2(pos.x as f32 / sf, pos.y as f32 / sf),
                            )
                        }
                    };

                    self.ui.raw_input.events.push(egui::Event::MouseWheel {
                        unit,
                        delta: vec_delta,
                        phase: egui::TouchPhase::Move,
                        modifiers: self.ui.raw_input.modifiers,
                    });
                } else {
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

        let (col, row) = match self.mouse_to_tile(x, y) {
            Some(res) => res,
            None => return,
        };
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
            let is_betrayer = self
                .sim
                .current_snapshot
                .as_ref()
                .and_then(|s| s.players.iter().find(|p| p.id == owner))
                .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
                .unwrap_or(false);
            let is_allied = self
                .sim
                .current_snapshot
                .as_ref()
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
                // Do not attack nor open menu on press; handled on release (click) instead
                return;
            } else {
                if !is_touch {
                    // Desktop: fire immediately
                    self.send_intent(intent);
                    self.input.hold_attack_target =
                        Some((owner, web_time::Instant::now(), x, y, true));
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
        if let Some((col, row)) = self.mouse_to_tile(x, y) {
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

        let (col, row) = match self.mouse_to_tile(x, y) {
            Some(res) => res,
            None => return,
        };

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
            if let Some(snap) = &self.sim.current_snapshot {
                let my_id = self.sim.my_player_id.unwrap_or(0);
                let owners = self
                    .gfx
                    .map_renderer
                    .as_ref()
                    .map(|mr| mr.owners.as_slice())
                    .unwrap_or(&[]);
                let terrain = self
                    .gfx
                    .map_renderer
                    .as_ref()
                    .map(|mr| mr.terrain.as_slice())
                    .unwrap_or(&[]);

                let target_res = resolve_build_target_tile(
                    kind,
                    col,
                    row,
                    self.sim.map_w,
                    self.sim.map_h,
                    owners,
                    terrain,
                    my_id,
                    &snap.buildings,
                );

                let cost = {
                    let i = sow_core::game::BuildingKind::ALL
                        .iter()
                        .position(|&k| k == kind)
                        .unwrap_or(0);
                    self.ui.app.hud_state.building_costs[i]
                };

                let mut valid = true;
                let mut err_msg = String::new();

                if self.ui.app.hud_state.gold < cost {
                    valid = false;
                    let lang = self.ui.app.settings_state.language;
                    err_msg = sow_i18n::get(lang)
                        .hud
                        .err_need_gold
                        .replace("{}", &sow_ui::utils::format_number(cost));
                } else {
                    match target_res {
                        Ok(_) => {}
                        Err(msg) => {
                            valid = false;
                            err_msg = msg.to_string();
                        }
                    }
                }

                if !valid {
                    self.ui.app.hud_state.show_error = Some(err_msg);
                } else {
                    let target_tile = target_res.unwrap();
                    let intent =
                        sow_core::protocol::GameplayIntent::BuildStructure { kind, target_tile };
                    self.send_intent(intent);
                }
            }
        } else {
            // Check if we clicked on a Warship we own
            let mut clicked_warships = Vec::new();
            if let Some(snap) = &self.sim.current_snapshot {
                let my_pid = self.sim.my_player_id.unwrap_or(0);
                let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
                let world_y = (y as f32 - self.input.camera_y) / self.input.camera_zoom;
                for f in &snap.fleets {
                    if f.unit_type == sow_core::game::UnitType::Warship && f.owner_id == my_pid {
                        let col = (f.current_tile % self.sim.map_w) as f32;
                        let row = (f.current_tile / self.sim.map_w) as f32;
                        let wx = col + 0.5;
                        let wy = row + 0.5;
                        // Click tolerance (half a tile)
                        if (wx - world_x).abs() < 0.5 && (wy - world_y).abs() < 0.5 {
                            clicked_warships.push(f.id);
                        }
                    }
                }
            }
            if !clicked_warships.is_empty() {
                self.input.selected_warships = clicked_warships;
            } else {
                self.input.selected_warships.clear();

                // If not selecting warships, check if we clicked on allied territory to open context menu on release
                let idx = (row * self.sim.map_w as i32 + col) as usize;
                let owner = self
                    .gfx
                    .map_renderer
                    .as_ref()
                    .map(|mr| mr.owners[idx])
                    .unwrap_or(0);
                let my_id = self.sim.my_player_id.unwrap_or(0);
                let is_betrayer = self
                    .sim
                    .current_snapshot
                    .as_ref()
                    .and_then(|s| s.players.iter().find(|p| p.id == owner))
                    .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
                    .unwrap_or(false);
                let is_allied = self
                    .sim
                    .current_snapshot
                    .as_ref()
                    .and_then(|s| s.players.iter().find(|p| p.id == my_id))
                    .map(|p| p.alliances.contains(&owner) && !is_betrayer)
                    .unwrap_or(false);

                if owner != 0 && owner != my_id && is_allied {
                    self.open_context_menu_at(x, y);
                }
            }
        }
    }

    pub(crate) fn send_intent(&mut self, intent: sow_core::protocol::GameplayIntent) {
        match &intent {
            sow_core::protocol::GameplayIntent::LaunchFleet { target_tile, .. }
            | sow_core::protocol::GameplayIntent::MoveWarships { target_tile, .. } => {
                let wx = (*target_tile % self.sim.map_w) as f32 + 0.5;
                let wy = (*target_tile / self.sim.map_w) as f32 + 0.5;
                self.ui.click_markers.push(crate::app::ClickMarker {
                    world_x: wx,
                    world_y: wy,
                    start_time: web_time::Instant::now(),
                });
            }
            _ => {}
        }

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

/// Closest same-kind building within stack range of the click (matches server logic).
pub fn find_stack_target_tile(
    kind: sow_core::game::BuildingKind,
    click_x: i32,
    click_y: i32,
    map_w: u32,
    my_id: u16,
    buildings: &[sow_core::protocol::BuildingSnapshot],
) -> Option<u32> {
    let stack_dist = sow_core::building::placement::STRUCTURE_MIN_DIST;
    let mut best: Option<(i32, u64, u32)> = None;
    for b in buildings {
        if b.owner_id != my_id || b.kind != kind {
            continue;
        }
        let bx = (b.tile_idx % map_w) as i32;
        let by = (b.tile_idx / map_w) as i32;
        let d = (click_x - bx).abs() + (click_y - by).abs();
        if d > stack_dist {
            continue;
        }
        let cand = (d, b.id, b.tile_idx);
        match best {
            None => best = Some(cand),
            Some((bd, bid, _)) => {
                if d < bd || (d == bd && b.id < bid) {
                    best = Some(cand);
                }
            }
        }
    }
    best.map(|(_, _, tile)| tile)
}

#[allow(clippy::too_many_arguments)]
pub fn resolve_build_target_tile(
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
    if let Some(tile) = find_stack_target_tile(kind, click_x, click_y, map_w, my_id, buildings) {
        return Ok(tile);
    }
    resolve_building_placement_tile(
        kind, click_x, click_y, map_w, map_h, owners, terrain, my_id, buildings,
    )
}

#[allow(clippy::too_many_arguments)]
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
    let pokayoke_dist = 25;
    let pokayoke_dist_sq = pokayoke_dist * pokayoke_dist;

    let mut found_any_owned = false;
    let mut found_any_land = false;
    let mut found_any_far_enough = false;

    let mut valid_land_tiles = Vec::new();
    for dy in -pokayoke_dist..=pokayoke_dist {
        for dx in -pokayoke_dist..=pokayoke_dist {
            let tx = click_x + dx;
            let ty = click_y + dy;
            if tx < 0 || tx >= map_w as i32 || ty < 0 || ty >= map_h as i32 {
                continue;
            }
            if (dx * dx + dy * dy) >= pokayoke_dist_sq {
                continue;
            }
            let tile_idx = (ty * map_w as i32 + tx) as u32;

            if owners.get(tile_idx as usize).copied().unwrap_or(0) != my_id {
                continue;
            }
            found_any_owned = true;

            let tile_terrain = terrain.get(tile_idx as usize).copied().unwrap_or(0);
            let is_land = (tile_terrain & 0x80) != 0;
            if !is_land {
                continue;
            }
            found_any_land = true;

            let mut too_close = false;
            for rule in kind.spacing_rules() {
                let min_d = rule.min_distance;
                let min_d_sq = min_d * min_d;
                for b in buildings {
                    if b.kind == rule.target_kind {
                        let bx = (b.tile_idx % map_w) as i32;
                        let by = (b.tile_idx / map_w) as i32;
                        let bdx = tx - bx;
                        let bdy = ty - by;
                        if (bdx * bdx + bdy * bdy) < min_d_sq {
                            too_close = true;
                            break;
                        }
                    }
                }
                if too_close {
                    break;
                }
            }

            if too_close {
                continue;
            }
            found_any_far_enough = true;

            if kind == sow_core::game::BuildingKind::Port {
                let mut near_water = false;
                for wdy in -2..=2 {
                    for wdx in -2..=2 {
                        let nx = tx + wdx;
                        let ny = ty + wdy;
                        if nx >= 0 && nx < map_w as i32 && ny >= 0 && ny < map_h as i32 {
                            let n_idx = (ny * map_w as i32 + nx) as usize;
                            let n_terr = terrain.get(n_idx).copied().unwrap_or(0);
                            let n_is_land = (n_terr & 0x80) != 0;
                            if !n_is_land {
                                near_water = true;
                                break;
                            }
                        }
                    }
                    if near_water {
                        break;
                    }
                }
                if !near_water {
                    continue;
                }
            }

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
            if kind == sow_core::game::BuildingKind::City {
                return Err("Too close to another City! Minimum spacing is 6 tiles.");
            } else {
                return Err("Too close to another structure! Spacing rules: City requires 6, other structures require 4.");
            }
        }
        return Err("No space nearby!");
    }

    valid_land_tiles.sort_unstable_by(|a, b| {
        let da = (a.0 - click_x) * (a.0 - click_x) + (a.1 - click_y) * (a.1 - click_y);
        let db = (b.0 - click_x) * (b.0 - click_x) + (b.1 - click_y) * (b.1 - click_y);
        da.cmp(&db).then_with(|| a.2.cmp(&b.2))
    });
    Ok(valid_land_tiles.first().map(|&(_, _, idx)| idx).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sow_core::game::BuildingKind;
    use sow_core::protocol::BuildingSnapshot;

    fn land_terrain() -> Vec<u8> {
        vec![0x80; 32 * 32]
    }

    fn owned_map(w: u32, h: u32, owner: u16) -> Vec<u16> {
        vec![owner; (w * h) as usize]
    }

    fn city_snapshot(id: u64, owner: u16, tile_idx: u32) -> BuildingSnapshot {
        BuildingSnapshot {
            id,
            owner_id: owner,
            tile_idx,
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: sow_core::building::CityModules::default(),
        }
    }

    #[test]
    fn click_on_city_resolves_to_city_tile() {
        let map_w = 32u32;
        let map_h = 32u32;
        let my_id = 1u16;
        let city_tile = 10 * map_w + 10;
        let buildings = vec![city_snapshot(1, my_id, city_tile)];
        let owners = owned_map(map_w, map_h, my_id);
        let terrain = land_terrain();

        let resolved = resolve_build_target_tile(
            BuildingKind::City,
            10,
            10,
            map_w,
            map_h,
            &owners,
            &terrain,
            my_id,
            &buildings,
        )
        .expect("click on city should stack");

        assert_eq!(resolved, city_tile);
    }

    #[test]
    fn click_far_from_city_snaps_to_spawn_tile() {
        let map_w = 32u32;
        let map_h = 32u32;
        let my_id = 1u16;
        let city_tile = 5 * map_w + 5;
        let buildings = vec![city_snapshot(1, my_id, city_tile)];
        let owners = owned_map(map_w, map_h, my_id);
        let terrain = land_terrain();

        let resolved = resolve_build_target_tile(
            BuildingKind::City,
            20,
            20,
            map_w,
            map_h,
            &owners,
            &terrain,
            my_id,
            &buildings,
        )
        .expect("click far from city should find spawn tile");

        assert_ne!(resolved, city_tile);
    }
}
