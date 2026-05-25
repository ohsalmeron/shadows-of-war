use crate::{camera_zoom_upper_bound, CAMERA_MIN_ZOOM};
use sow_render::MapGlobals;

use sow_ui::app::ClientPhase;
use web_time::Instant;

use crate::app::SowApp;
use blade_graphics as gpu;

pub mod interact;
pub mod world;

impl SowApp {
    pub fn render_frame(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        static REGISTER_ONCE: std::sync::Once = std::sync::Once::new();
        REGISTER_ONCE.call_once(|| {
            sow_core::register_game_assets!(self.ui.egui_ctx, "../../../sow-client/assets/");
        });

        if self.map_editor.is_some() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(win) = self.gfx.window.as_ref() {
            let web_win = web_sys::window().unwrap();
            let w = web_win.inner_width().unwrap().as_f64().unwrap();
            let h = web_win.inner_height().unwrap().as_f64().unwrap();

            // Use the logical size and sf to calculate current physical size
            let sf = win.scale_factor();
            let expected_w = (w * sf) as u32;
            let expected_h = (h * sf) as u32;

            if expected_w.abs_diff(self.input.screen_w as u32) > 1
                || expected_h.abs_diff(self.input.screen_h as u32) > 1
            {
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
            self.gfx
                .render_ctx
                .command_encoder
                .init_texture(frame.texture());

            let mut map_drawn = false;
            if let Some(ref mut mr) = self.gfx.map_renderer {
                // Upload map state on first frame or after each tick
                if self.gfx.needs_first_upload {
                    self.gfx
                        .render_ctx
                        .command_encoder
                        .init_texture(mr.terrain_texture);
                    self.gfx
                        .render_ctx
                        .command_encoder
                        .init_texture(mr.owner_texture);
                    self.gfx.needs_first_upload = false;
                    mr.upload_terrain(&mut self.gfx.render_ctx.command_encoder);
                }

                // Perform CPU-side update of the map
                let dirty = self
                    .sim
                    .current_snapshot
                    .as_ref()
                    .map(|s| s.dirty_tiles.as_slice())
                    .unwrap_or(&[]);
                mr.update(
                    &mut self.gfx.render_ctx.command_encoder,
                    &self.gfx.render_ctx.context,
                    dirty,
                );
                for dt in dirty {
                    let i = dt.index as usize;
                    if i < self.sim.tile_upgrades.len() {
                        self.sim.tile_upgrades[i] = dt.upgrade_level;
                    }
                }
                if let Some(snap) = &mut self.sim.current_snapshot {
                    snap.dirty_tiles.clear();
                }
                let mut border_thickness = 1.0f32;
                let mut border_darkness = 0.35f32;
                let mut shore_thickness = 1.0f32;
                let mut shore_darkness = 1.0f32;

                self.ui.egui_ctx.data_mut(|d| {
                    border_thickness =
                        *d.get_temp_mut_or_insert_with(egui::Id::new("dev_thickness"), || 1.0f32);
                    border_darkness =
                        *d.get_temp_mut_or_insert_with(egui::Id::new("dev_darkness"), || 0.35f32);
                    shore_thickness = *d
                        .get_temp_mut_or_insert_with(egui::Id::new("dev_shore_thickness"), || {
                            1.0f32
                        });
                    shore_darkness = *d
                        .get_temp_mut_or_insert_with(egui::Id::new("dev_shore_darkness"), || {
                            1.0f32
                        });
                });

                let mut player_colors = [[0.5, 0.5, 0.5, 1.0]; 256];
                if let Some(snap) = &self.sim.current_snapshot {
                    for p in &snap.players {
                        if (p.id as usize) < 256 {
                            player_colors[p.id as usize] =
                                [p.color[0], p.color[1], p.color[2], 1.0];
                        }
                    }
                }

                let mut threat_slots = [[0.0f32; 4]; 8];
                if let Some(snap) = &self.sim.current_snapshot {
                    let my_id = self.sim.my_player_id.unwrap_or(0);
                    let mut slot = 0usize;
                    for attack in &snap.attacks {
                        if slot >= 8 {
                            break;
                        }
                        let is_mine = my_id != 0 && attack.owner_id == my_id;
                        let targets_me = my_id != 0 && attack.target_owner == my_id;
                        if !(is_mine || targets_me) || attack.troops <= 0.0 {
                            continue;
                        }
                        if attack.front_cx == 0.0 && attack.front_cy == 0.0 {
                            continue;
                        }
                        let radius = (attack.troops as f32 / std::f32::consts::PI)
                            .sqrt()
                            .clamp(1.0, 200.0)
                            * 2.5;
                        threat_slots[slot] = [
                            attack.front_cx,
                            attack.front_cy,
                            radius,
                            attack.target_owner as f32 * 1024.0 + attack.owner_id as f32,
                        ];
                        slot += 1;
                    }
                }

                let mut hover_hex = [0.0f32, 0.0f32];
                let mut hover_building_kind = 0.0f32;
                let mut nobuild_slots = [[0.0f32; 4]; 32];

                if let Some(kind) = self.ui.app.hud_state.selected_building_kind {
                    let mx = self.input.last_mouse_x as f32;
                    let my = self.input.last_mouse_y as f32;

                    let world_x = (mx - self.input.camera_x) / self.input.camera_zoom;
                    let world_y = (my - self.input.camera_y) / self.input.camera_zoom;

                    let q_f = world_x - world_y * 0.577_350_26_f32;
                    let r_f = world_y * 1.154_700_5_f32;
                    let s_f = -q_f - r_f;

                    let mut rq = q_f.round();
                    let mut rr = r_f.round();
                    let rs = s_f.round();

                    let q_diff = (rq - q_f).abs();
                    let r_diff = (rr - r_f).abs();
                    let s_diff = (rs - s_f).abs();

                    if q_diff > r_diff && q_diff > s_diff {
                        rq = -rr - rs;
                    } else if r_diff > s_diff {
                        rr = -rq - rs;
                    }

                    let col = rq as i32 + (rr as i32 - (rr as i32 & 1)) / 2;
                    let row = rr as i32;

                    hover_hex = [col as f32, row as f32];
                    hover_building_kind = match kind {
                        sow_core::game::BuildingKind::City => 1.0,
                        sow_core::game::BuildingKind::Bunker => 2.0,
                    };

                    if let Some(snap) = &self.sim.current_snapshot {
                        let mut list = Vec::new();
                        for b in &snap.buildings {
                            let bx = (b.tile_idx % self.sim.map_w) as i32;
                            let by = (b.tile_idx / self.sim.map_w) as i32;

                            let radius = match kind {
                                sow_core::game::BuildingKind::City => {
                                    if b.kind == sow_core::game::BuildingKind::City {
                                        Some(12.0f32)
                                    } else {
                                        None
                                    }
                                }
                                sow_core::game::BuildingKind::Bunker => {
                                    if b.kind == sow_core::game::BuildingKind::City {
                                        Some(6.0f32)
                                    } else if b.kind == sow_core::game::BuildingKind::Bunker {
                                        Some(4.0f32)
                                    } else {
                                        None
                                    }
                                }
                            };

                            if let Some(r_val) = radius {
                                let q1 = col - (row - (row & 1)) / 2;
                                let r1 = row;
                                let q2 = bx - (by - (by & 1)) / 2;
                                let r2 = by;
                                let dq = q2 - q1;
                                let dr = r2 - r1;
                                let hex_dist = (dq.abs() + dr.abs() + (dq + dr).abs()) / 2;

                                list.push((bx, by, r_val, hex_dist));
                            }
                        }

                        list.sort_unstable_by_key(|item| item.3);
                        for (i, item) in list.iter().take(32).enumerate() {
                            nobuild_slots[i] = [item.0 as f32, item.1 as f32, item.2, 1.0f32];
                        }
                    }
                }

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
                    threat_slots,
                    effect_shockwave: 1.0,
                    effect_breathe: 1.0,
                    effect_energy_flow: 1.0,
                    my_player_id: self.sim.my_player_id.unwrap_or(0) as f32,
                    hover_hex,
                    hover_building_kind,
                    _pad1: 0.0,
                    nobuild_slots,
                };
                let colors_struct = sow_render::PlayerColors {
                    colors: player_colors,
                };
                mr.draw(
                    &mut self.gfx.render_ctx.command_encoder,
                    frame.texture_view(),
                    globals,
                    colors_struct,
                );
                map_drawn = true;
            }

            if !map_drawn {
                let pass = self.gfx.render_ctx.command_encoder.render(
                    "clear_pass",
                    gpu::RenderTargetSet {
                        colors: &[gpu::RenderTarget {
                            view: frame.texture_view(),
                            init_op: gpu::InitOp::Clear(gpu::TextureColor::OpaqueBlack),
                            finish_op: gpu::FinishOp::Store,
                        }],
                        depth_stencil: None,
                    },
                );
                drop(pass);
            }

            // ── UI UPDATE ───────────────────────────────────────
            let mut sf = self
                .gfx
                .window
                .as_ref()
                .map_or(1.0, |w| w.scale_factor() as f32);
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
                egui::Vec2::new(self.input.screen_w / sf, self.input.screen_h / sf),
            ));

            let mut safe_area_top = 0.0;
            let mut safe_area_bottom = 0.0;
            let mut safe_area_left = 0.0;
            let mut safe_area_right = 0.0;

            if cfg!(target_os = "android") || cfg!(target_os = "ios") {
                if let Some(win) = self.gfx.window.as_ref() {
                    let insets = win.safe_area();
                    safe_area_top = (insets.top as f32 / sf).round();
                    safe_area_bottom = (insets.bottom as f32 / sf).round();
                    safe_area_left = (insets.left as f32 / sf).round();
                    safe_area_right = (insets.right as f32 / sf).round();
                }

                self.ui.raw_input.safe_area_insets = Some(egui::SafeAreaInsets(
                    egui::Margin {
                        top: safe_area_top.min(127.0) as i8,
                        bottom: safe_area_bottom.min(127.0) as i8,
                        left: safe_area_left.min(127.0) as i8,
                        right: safe_area_right.min(127.0) as i8,
                    }
                    .into(),
                ));
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
            let dt = frame_now
                .duration_since(self.time.last_frame_time)
                .as_secs_f32();
            self.time.last_frame_time = frame_now;
            self.ui.raw_input.predicted_dt = dt.min(0.1);

            if self.ui.app.main_menu_state.is_waiting
                && self.ui.app.main_menu_state.wait_timer_secs > 0.0
            {
                self.ui.app.main_menu_state.wait_timer_secs =
                    (self.ui.app.main_menu_state.wait_timer_secs - self.ui.raw_input.predicted_dt)
                        .max(0.0);
            }
            if let Some(ref mut secs) = self.ui.app.hud_state.spawn_timer_secs {
                *secs = (*secs - self.ui.raw_input.predicted_dt).max(0.0);
            }
            if let Some(ref mut sync) = self.ui.app.hud_state.sync_state {
                sync.time_remaining =
                    (sync.time_remaining - self.ui.raw_input.predicted_dt).max(0.0);
            }
            let mut local_cancel_intents: Vec<sow_core::protocol::GameplayIntent> = Vec::new();
            if self.ui.app.phase == ClientPhase::Playing {
                self.sync_hud_combat_state();
            }

            #[cfg(target_arch = "wasm32")]
            self.ime_bridge
                .drain_pending_into(&mut self.ui.raw_input.events);

            let egui_ctx = self.ui.egui_ctx.clone();
            let egui_output = egui_ctx.run_ui(self.ui.raw_input.clone(), |ctx| {
                if (cfg!(target_os = "android") || cfg!(target_os = "ios"))
                    && self.ui.app.phase == sow_ui::app::ClientPhase::MainMenu
                {
                    let config = crate::config::ClientVisualConfig::default();
                    let screen_rect = ctx.content_rect();
                    let painter = ctx.layer_painter(egui::LayerId::new(
                        egui::Order::Foreground,
                        egui::Id::new("safe_area_bars"),
                    ));

                    let top_c = config.top_bar_color;
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            screen_rect.min,
                            egui::pos2(screen_rect.max.x, screen_rect.min.y + safe_area_top),
                        ),
                        0.0,
                        egui::Color32::from_rgba_premultiplied(
                            top_c[0], top_c[1], top_c[2], top_c[3],
                        ),
                    );

                    let bot_c = config.bottom_bar_color;
                    painter.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(screen_rect.min.x, screen_rect.max.y - safe_area_bottom),
                            screen_rect.max,
                        ),
                        0.0,
                        egui::Color32::from_rgba_premultiplied(
                            bot_c[0], bot_c[1], bot_c[2], bot_c[3],
                        ),
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

                self.render_dev_panels(ctx);
                let ui_action = self.ui.app.draw(ctx, &mut local_cancel_intents);

                if self.ui.update_available {
                    egui::Window::new("Update Available")
                        .collapsible(false)
                        .resizable(false)
                        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                        .show(ctx, |ui| {
                            ui.heading("A new version is available!");
                            ui.add_space(10.0);
                            ui.label(
                                "The server has been updated. Please refresh to continue playing.",
                            );
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

                self.process_ui_actions(ctx, ui_action);
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

            // Offline tick generation moved to sim.rs

            #[cfg(not(target_arch = "wasm32"))]
            if let Some(win) = self.gfx.window.as_ref() {
                let ime_opt = egui_output.platform_output.ime;
                let allow_ime = ime_opt.is_some();

                if let Some(ime_out) = ime_opt {
                    let ppp = egui_output.pixels_per_point;
                    let ime_rect_px = ppp * ime_out.rect;
                    let had_input_events = !self.ui.raw_input.events.is_empty();
                    let toggling = self.input.ime_allowed_state != allow_ime;

                    if toggling
                        || self.input.ime_cursor_rect_px != Some(ime_rect_px)
                        || had_input_events
                    {
                        self.input.ime_allowed_state = true;
                        self.input.ime_cursor_rect_px = Some(ime_rect_px);

                        let request_data = winit::window::ImeRequestData::default()
                            .with_cursor_area(
                                winit::dpi::PhysicalPosition::new(
                                    ime_rect_px.min.x.round() as i32,
                                    ime_rect_px.min.y.round() as i32,
                                )
                                .into(),
                                winit::dpi::PhysicalSize::new(
                                    ime_rect_px.width().round().max(1.0) as u32,
                                    ime_rect_px.height().round().max(1.0) as u32,
                                )
                                .into(),
                            );

                        if toggling {
                            let caps = winit::window::ImeCapabilities::new().with_cursor_area();
                            if let Some(req) =
                                winit::window::ImeEnableRequest::new(caps, request_data)
                            {
                                let _ =
                                    win.request_ime_update(winit::window::ImeRequest::Enable(req));
                            }
                        } else {
                            let _ = win.request_ime_update(winit::window::ImeRequest::Update(
                                request_data,
                            ));
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

                let mut pass = self.gfx.render_ctx.command_encoder.render(
                    "ui_pass",
                    gpu::RenderTargetSet {
                        colors: &[gpu::RenderTarget {
                            view: frame.texture_view(),
                            init_op: gpu::InitOp::Load,
                            finish_op: gpu::FinishOp::Store,
                        }],
                        depth_stencil: None,
                    },
                );

                gp.paint(
                    &mut pass,
                    &paint_jobs,
                    &screen_desc,
                    &self.gfx.render_ctx.context,
                );
                drop(pass);
            }
            if let Some(ref mut gp) = self.gfx.gui_painter {
                gp.sync(&self.gfx.render_ctx.context);
            }
            self.gfx.render_ctx.command_encoder.present(frame);
            let sync_point = self
                .gfx
                .render_ctx
                .context
                .submit(&mut self.gfx.render_ctx.command_encoder);

            if let Some(ref mut gp) = self.gfx.gui_painter {
                gp.after_submit(&sync_point, &self.gfx.render_ctx.context);
            }

            self.gfx.prev_sync_point = Some(sync_point);
        }
    }
}

impl SowApp {
    pub fn check_surface(&mut self) {
        if self.gfx.surface.is_none() {
            if let Some(ref win) = self.gfx.window {
                let sz = win.surface_size();
                match self
                    .gfx
                    .render_ctx
                    .create_surface(win, sz.width.max(1), sz.height.max(1))
                {
                    Ok(s) => {
                        self.input.screen_w = sz.width as f32;
                        self.input.screen_h = sz.height as f32;
                        let zmax =
                            camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h);
                        self.input.camera_zoom =
                            self.input.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);
                        self.ui.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::Vec2::new(self.input.screen_w, self.input.screen_h),
                        ));
                        let format = s.info().format;

                        if let Some(sp) = self.gfx.prev_sync_point.take() {
                            let _ = self.gfx.render_ctx.context.wait_for(&sp, !0);
                        }
                        if let Some(mut old_mr) = self.gfx.map_renderer.take() {
                            let old_terrain = old_mr.terrain.clone();
                            old_mr.destroy(&self.gfx.render_ctx);
                            self.gfx.map_renderer = Some(sow_render::MapRenderer::new(
                                &self.gfx.render_ctx.context,
                                self.sim.map_w,
                                self.sim.map_h,
                                format,
                                &old_terrain,
                            ));
                            self.gfx.needs_first_upload = true;
                        }
                        if let Some(mut old_gp) = self.gfx.gui_painter.take() {
                            old_gp.destroy(&self.gfx.render_ctx.context);
                        }

                        self.gfx.gui_painter = Some(blade_egui::GuiPainter::new(
                            s.info(),
                            &self.gfx.render_ctx.context,
                        ));
                        self.gfx.surface = Some(s);

                        self.ui.egui_ctx = egui::Context::default();
                        egui_extras::install_image_loaders(&self.ui.egui_ctx);
                        sow_ui::ui::theme::apply_theme(&self.ui.egui_ctx);
                        log::info!("Successfully created surface on retry.");
                    }
                    Err(e) => {
                        log::warn!("Surface creation failed: {:?}", e);
                    }
                }
            }
        }
    }
}
