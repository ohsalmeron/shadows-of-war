use crate::app::SowApp;
use crate::{camera_zoom_upper_bound, CAMERA_MIN_ZOOM};
use egui::Pos2;
use sow_ui_kit::ClientPhase;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};

impl SowApp {
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

                if !self.ui.egui_ctx.egui_wants_keyboard_input()
                    && self.ui.app.phase == ClientPhase::Playing
                {
                    if let winit::keyboard::PhysicalKey::Code(code) = event.physical_key {
                        match code {
                            winit::keyboard::KeyCode::KeyW | winit::keyboard::KeyCode::ArrowUp => {
                                self.input.key_pan_up = pressed;
                            }
                            winit::keyboard::KeyCode::KeyS
                            | winit::keyboard::KeyCode::ArrowDown => {
                                self.input.key_pan_down = pressed;
                            }
                            winit::keyboard::KeyCode::KeyA
                            | winit::keyboard::KeyCode::ArrowLeft => {
                                self.input.key_pan_left = pressed;
                            }
                            winit::keyboard::KeyCode::KeyD
                            | winit::keyboard::KeyCode::ArrowRight => {
                                self.input.key_pan_right = pressed;
                            }
                            _ => {}
                        }
                    }
                } else {
                    self.input.key_pan_up = false;
                    self.input.key_pan_down = false;
                    self.input.key_pan_left = false;
                    self.input.key_pan_right = false;
                }

                if pressed
                    && !self.ui.egui_ctx.egui_wants_keyboard_input()
                    && self.ui.app.phase == ClientPhase::Playing
                    && self.ui.app.hud_state.sync_state.is_none()
                {
                    if let winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyB) =
                        event.physical_key
                    {
                        if !self.ui.egui_ctx.egui_wants_pointer_input() {
                            if let Some((col, row)) =
                                self.mouse_to_tile(self.input.last_mouse_x, self.input.last_mouse_y)
                            {
                                let idx = (row * self.sim.map_w as i32 + col) as usize;

                                let owner = self
                                    .gfx
                                    .map_renderer
                                    .as_ref()
                                    .map(|mr| mr.owners[idx])
                                    .unwrap_or(0);
                                let my_id = self.sim.my_player_id.unwrap_or(0);

                                let owner_snapshot = self
                                    .sim
                                    .current_snapshot
                                    .as_ref()
                                    .and_then(|s| s.players.iter().find(|p| p.id == owner));
                                let my_snapshot = self
                                    .sim
                                    .current_snapshot
                                    .as_ref()
                                    .and_then(|s| s.players.iter().find(|p| p.id == my_id));

                                let is_teammate = if let Some(owner) = owner_snapshot {
                                    if let Some(my_snap) = my_snapshot {
                                        my_snap.team.is_some() && my_snap.team == owner.team
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                };

                                let is_betrayer = owner_snapshot
                                    .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
                                    .unwrap_or(false);
                                let is_allied = my_snapshot
                                    .map(|p| p.alliances.contains(&owner) && !is_betrayer)
                                    .unwrap_or(false);

                                if owner != 0 && owner != my_id && is_allied {
                                    let lang = self.ui.app.settings_state.language;
                                    let msg =
                                        sow_i18n::get(lang).hud.err_break_alliance_boat.clone();
                                    let mx = self.input.last_mouse_x;
                                    let my = self.input.last_mouse_y;
                                    let world_x =
                                        (mx as f32 - self.input.camera_x) / self.input.camera_zoom;
                                    let offset_my = my as f32 - 60.0;
                                    let world_y =
                                        (offset_my - self.input.camera_y) / self.input.camera_zoom;
                                    self.ui.floating_notices.push(crate::app::FloatingNotice {
                                        text: msg,
                                        world_x,
                                        world_y,
                                        start_time: web_time::Instant::now(),
                                        duration: web_time::Duration::from_millis(2000),
                                        color: egui::Color32::from_rgb(248, 113, 113),
                                    });
                                    self.open_context_menu_at(mx, my);
                                } else if !is_teammate && owner != my_id {
                                    let troops = Some(
                                        self.ui.app.hud_state.troops
                                            * (self.ui.app.hud_state.attack_ratio as f64),
                                    );
                                    self.send_intent(
                                        sow_core::protocol::GameplayIntent::LaunchFleet {
                                            target_tile: idx as u32,
                                            troops,
                                        },
                                    );
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
                        if (code == winit::keyboard::KeyCode::KeyQ
                            || code == winit::keyboard::KeyCode::Escape)
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
                            if in_game && self.ui.app.hud_state.selected_building_kind.is_some() {
                                // Do not drag map while drawing structures
                            } else {
                                self.input.dragging = true;
                            }
                        }
                        // Start hold tracking
                        if !wants_pointer && in_game {
                            self.input.map_touch_start =
                                Some((web_time::Instant::now(), position.x, position.y));
                            if self.ui.app.hud_state.selected_building_kind.is_some() {
                                self.handle_map_click(position.x, position.y);
                                self.input.hold_build_active = true;
                                self.input.hold_build_accum = 0.0;
                            } else {
                                self.try_begin_hold_attack(position.x, position.y, is_touch);
                            }
                        }
                    } else {
                        self.input.dragging = false;
                        // On release: if it was a quick tap, handle click actions
                        if !wants_pointer && in_game {
                            if self.input.hold_build_active {
                                self.input.hold_build_active = false;
                                self.input.hold_build_accum = 0.0;
                            } else {
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
                                    let is_nuke =
                                        self.ui.app.hud_state.selected_nuke_kind.is_some();
                                    if is_touch && !is_spawning && !is_building && !is_nuke {
                                        // Tap on mobile → open context menu
                                        self.open_context_menu_at(sx, sy);
                                    } else {
                                        // Quick click on desktop or tap during spawn/build/nuke → one-shot action
                                        self.handle_map_click(sx, sy);
                                    }
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
                            // ponytail: query device_pixel_ratio directly on WASM for scroll delta scaling
                            #[cfg(target_arch = "wasm32")]
                            let sf = web_sys::window()
                                .map(|window| window.device_pixel_ratio() as f32)
                                .unwrap_or(1.0);
                            #[cfg(not(target_arch = "wasm32"))]
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
                    let zoom_factor = 1.0 + scroll * 0.15;
                    self.input.target_zoom *= zoom_factor;
                    let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h);
                    self.input.target_zoom = self.input.target_zoom.clamp(CAMERA_MIN_ZOOM, zmax);
                    if let Some(win) = self.gfx.window.as_ref() {
                        win.request_redraw();
                    }
                }
            }

            _ => {}
        }
    }
}
