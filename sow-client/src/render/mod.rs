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
            sow_core::register_game_assets(&self.ui.egui_ctx);
        });

        #[cfg(target_arch = "wasm32")]
        if let Some(win) = self.gfx.window.as_ref() {
            crate::viewport::sync_wasm_window(self, win.as_ref());
        }

        let wanted = self
            .gfx
            .window
            .as_ref()
            .map(|w| crate::viewport::Viewport::measure(w.as_ref()));

        if let Some(ref vp) = wanted {
            if vp.wants_reconfigure(self) {
                self.apply_surface_resize(vp.physical, false);
            }
        }

        let draw_world = self.should_draw_world();

        if let Some(win) = self.gfx.window.as_ref() {
            win.pre_present_notify();
        }
        let Some(frame) = self.gfx.surface.as_mut().map(|s| s.acquire_frame()) else {
            return;
        };

        let configured = self.gfx.configured_physical;
        let window_ahead = wanted.as_ref().is_some_and(|vp| {
            vp.physical.width != configured.width || vp.physical.height != configured.height
        });
        if window_ahead {
            if let Some(vp) = wanted {
                self.apply_surface_resize(vp.physical, false);
            }
            if let Some(win) = self.gfx.window.as_ref() {
                win.request_redraw();
            }
            return;
        }

        let sf = wanted.as_ref().map(|v| v.scale_factor).unwrap_or(1.0);
        crate::viewport::Viewport::from_configured(self, sf).sync_to_app(self);
        crate::viewport::scale_pointer_events(&mut self.ui.raw_input, sf);

        if self.gfx.pending_session_cleanup {
            self.gfx.pending_session_cleanup = false;
            if self.ui.app.phase == ClientPhase::MainMenu {
                if let Some(render_ctx) = self.gfx.render_ctx.take() {
                    if let Some(sp) = self.gfx.prev_sync_point.take() {
                        let _ = render_ctx.context.wait_for(&sp, !0);
                    }
                    self.gfx.render_ctx = Some(render_ctx);
                    self.cleanup_game_session_stub();
                }
            }
        }

        if let Some(ref mut s) = self.gfx.surface {
            let mut render_ctx = match self.gfx.render_ctx.take() {
                Some(ctx) => ctx,
                None => return,
            };

            if let Some(sp) = self.gfx.prev_sync_point.take() {
                let _ = render_ctx.context.wait_for(&sp, !0);
            }

            render_ctx.command_encoder.start();
            render_ctx.command_encoder.init_texture(frame.texture());

            let mut map_drawn = false;
            if let Some(ref mut mr) = self.gfx.map_renderer {
                // Upload map state on first frame or after each tick (runs during splash for loader step 2).
                if self.gfx.needs_first_upload {
                    render_ctx.command_encoder.init_texture(mr.terrain_texture);
                    render_ctx.command_encoder.init_texture(mr.owner_texture);
                    self.gfx.needs_first_upload = false;
                    mr.upload_terrain(&mut render_ctx.command_encoder);
                }

                if draw_world {
                    // --- Layer 4: Track and Spawn Detonations ---
                    let mut new_detonations = Vec::new();
                    if let Some(snap) = &self.sim.current_snapshot {
                        for (id, prev_proj) in &self.ui.last_projectiles {
                            if !snap.projectiles.iter().any(|p| p.id == *id) {
                                let at_end = prev_proj.path_cursor
                                    + (prev_proj.steps_per_tick as usize)
                                    >= prev_proj.path_len;
                                if at_end {
                                    let dst_x = (prev_proj.dst_tile % self.sim.map_w) as f32;
                                    let dst_y = (prev_proj.dst_tile / self.sim.map_w) as f32;
                                    new_detonations.push((dst_x, dst_y, prev_proj.kind));
                                }
                            }
                        }
                    }

                    // Spawns and tracks active fallout zones
                    let current_time = web_time::Instant::now();
                    for (dx, dy, kind) in new_detonations {
                        if let sow_core::game::ProjectileKind::Nuke { level } = kind {
                            sow_audio::play_nuke_impact_sound(
                                level,
                                sow_audio::SpatialSoundParams {
                                    wx: dx + 0.5,
                                    wy: dy + 0.5,
                                    camera_x: self.input.camera_x,
                                    camera_y: self.input.camera_y,
                                    camera_zoom: self.input.camera_zoom,
                                    screen_w: self.input.screen_w,
                                    screen_h: self.input.screen_h,
                                },
                            );

                            let fallout_radius = 30.0 + (level.saturating_sub(1) as f32) * 22.5;
                            self.ui.fallout_zones.push(crate::app::FalloutZone {
                                x: dx,
                                y: dy,
                                radius: fallout_radius,
                                start_time: current_time,
                            });
                        }
                    }

                    // Detect new nuke launches and set client-side silo cooldowns
                    if let Some(snap) = &self.sim.current_snapshot {
                        let current_tick = snap.tick;
                        const SILO_COOLDOWN_TICKS: u64 = 90;

                        for proj in &snap.projectiles {
                            if matches!(proj.kind, sow_core::game::ProjectileKind::Nuke { .. })
                                && !self.ui.last_projectiles.contains_key(&proj.id)
                            {
                                let src_x = (proj.src_tile % self.sim.map_w) as f32 + 0.5;
                                let src_y = (proj.src_tile / self.sim.map_w) as f32 + 0.5;
                                sow_audio::play_nuke_launch_sound(sow_audio::SpatialSoundParams {
                                    wx: src_x,
                                    wy: src_y,
                                    camera_x: self.input.camera_x,
                                    camera_y: self.input.camera_y,
                                    camera_zoom: self.input.camera_zoom,
                                    screen_w: self.input.screen_w,
                                    screen_h: self.input.screen_h,
                                });

                                // New nuke — find source building by src_tile
                                if let Some(b) = snap.buildings.iter().find(|b| {
                                    b.kind == sow_core::game::BuildingKind::City
                                        && b.tile_idx == proj.src_tile
                                }) {
                                    self.ui
                                        .silo_cooldowns
                                        .insert(b.id, current_tick + SILO_COOLDOWN_TICKS);
                                }
                            }
                        }

                        // Prune expired cooldowns
                        self.ui
                            .silo_cooldowns
                            .retain(|_, expires| *expires > current_tick);
                    }

                    // Sync last_projectiles
                    if let Some(snap) = &self.sim.current_snapshot {
                        self.ui.last_projectiles.clear();
                        for proj in &snap.projectiles {
                            self.ui.last_projectiles.insert(
                                proj.id,
                                crate::app::TrackedProjectile::from_snapshot(proj),
                            );
                        }
                    }

                    let mut border_thickness = 0.5f32;
                    let mut border_darkness = 0.35f32;
                    let mut shore_thickness = 1.0f32;
                    let mut shore_darkness = 1.0f32;
                    let mut territory_opacity = 1.0f32;
                    let mut blend_mode = 0.0f32;
                    let mut sub_voxel_scale = 1.0f32;
                    let mut conquest_duration = 2.5f32;

                    self.ui.egui_ctx.data_mut(|d| {
                        border_thickness = *d
                            .get_temp_mut_or_insert_with(egui::Id::new("dev_thickness"), || 0.5f32);
                        border_darkness = *d
                            .get_temp_mut_or_insert_with(egui::Id::new("dev_darkness"), || 0.35f32);
                        shore_thickness = *d.get_temp_mut_or_insert_with(
                            egui::Id::new("dev_shore_thickness"),
                            || 1.0f32,
                        );
                        shore_darkness = *d.get_temp_mut_or_insert_with(
                            egui::Id::new("dev_shore_darkness"),
                            || 1.0f32,
                        );
                        territory_opacity = *d.get_temp_mut_or_insert_with(
                            egui::Id::new("dev_territory_opacity"),
                            || 1.0f32,
                        );
                        blend_mode = *d
                            .get_temp_mut_or_insert_with(egui::Id::new("dev_blend_mode"), || {
                                0.0f32
                            });
                        sub_voxel_scale = *d.get_temp_mut_or_insert_with(
                            egui::Id::new("dev_sub_voxel_scale"),
                            || 1.0f32,
                        );
                        conquest_duration = *d.get_temp_mut_or_insert_with(
                            egui::Id::new("dev_conquest_duration"),
                            || 2.5f32,
                        );
                    });

                    let dirty = self
                        .sim
                        .current_snapshot
                        .as_ref()
                        .map(|s| s.dirty_tiles.as_slice())
                        .unwrap_or(&[]);

                    mr.update(
                        &mut render_ctx.command_encoder,
                        &render_ctx.context,
                        dirty,
                        conquest_duration,
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

                        let (col, row) =
                            crate::render::world::movers::world_to_tile(world_x, world_y);

                        hover_hex = [col as f32, row as f32];
                        hover_building_kind = match kind {
                            sow_core::game::BuildingKind::City => 1.0,
                            sow_core::game::BuildingKind::Bunker => 2.0,
                            sow_core::game::BuildingKind::Factory => 3.0,
                            sow_core::game::BuildingKind::Port => 4.0,
                        };

                        if let Some(snap) = &self.sim.current_snapshot {
                            let my_id = self.sim.my_player_id.unwrap_or(0);
                            let stack_tile = crate::input::find_stack_target_tile(
                                kind,
                                col,
                                row,
                                self.sim.map_w,
                                my_id,
                                &snap.buildings,
                            );

                            let mut list = Vec::new();
                            for b in &snap.buildings {
                                if stack_tile == Some(b.tile_idx) {
                                    continue;
                                }

                                let bx = (b.tile_idx % self.sim.map_w) as i32;
                                let by = (b.tile_idx / self.sim.map_w) as i32;

                                let mut radius = None;
                                for rule in kind.spacing_rules() {
                                    if b.kind == rule.target_kind {
                                        radius = Some(rule.min_distance as f32);
                                        break;
                                    }
                                }

                                if let Some(r_val) = radius {
                                    let dx = (bx - col).abs();
                                    let dy = (by - row).abs();
                                    let dist = dx.max(dy);

                                    list.push((bx, by, r_val, dist));
                                }
                            }

                            list.sort_unstable_by_key(|item| item.3);
                            for (i, item) in list.iter().take(32).enumerate() {
                                nobuild_slots[i] = [item.0 as f32, item.1 as f32, item.2, 1.0f32];
                            }
                        }
                    }

                    let current_time = web_time::Instant::now();
                    let mut fallout_slots = [[0.0f32; 4]; 8];
                    {
                        let mut slot = 0usize;
                        self.ui.fallout_zones.retain(|fz| {
                            let elapsed = current_time.duration_since(fz.start_time).as_secs_f32();
                            let duration = 7.0;
                            if elapsed >= duration {
                                return false;
                            }
                            if slot < 8 {
                                let alpha_p = (1.0 - elapsed / duration).max(0.0);
                                fallout_slots[slot] = [fz.x, fz.y, fz.radius, alpha_p];
                                slot += 1;
                            }
                            true
                        });
                    }

                    let globals = MapGlobals {
                        camera_pos: [self.input.camera_x, self.input.camera_y],
                        zoom: self.input.camera_zoom,
                        time: self.time.start_time.elapsed().as_secs_f32() % 1000.0,
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
                        territory_opacity,
                        fallout_slots,
                        nobuild_slots,
                        sub_voxel_scale,
                        blend_mode,
                        _pad3: 0.0,
                        _pad4: 0.0,
                    };
                    let colors_struct = sow_render::PlayerColors {
                        colors: player_colors,
                    };
                    mr.draw(
                        &mut render_ctx.command_encoder,
                        frame.texture_view(),
                        globals,
                        colors_struct,
                    );
                    map_drawn = true;

                    // ── GPU-instanced movers (boats, nukes, SAM) ─────────────
                    if self.gfx.mover_renderer.is_none() {
                        let surface_format = s.info().format;
                        self.gfx.mover_renderer = Some(sow_render::MoverRenderer::new(
                            &render_ctx.context,
                            surface_format,
                        ));
                        if let Some(ref mr_mover) = self.gfx.mover_renderer {
                            mr_mover
                                .upload_atlas(&mut render_ctx.command_encoder, &render_ctx.context);
                        }
                    }
                    if let (Some(ref mut mover_r), Some(ref snap)) =
                        (&mut self.gfx.mover_renderer, &self.sim.current_snapshot)
                    {
                        let now = web_time::Instant::now();
                        let alpha = crate::render::world::movers::interp_alpha(&self.time, now);
                        let pack = crate::render::world::movers::MoverPackParams {
                            camera_x: self.input.camera_x,
                            camera_y: self.input.camera_y,
                            camera_zoom: self.input.camera_zoom,
                            screen_w: self.input.screen_w,
                            screen_h: self.input.screen_h,
                            alpha,
                            selected_warships: &self.input.selected_warships,
                        };
                        crate::render::world::movers::update_and_pack(
                            &mut self.ui.mover_scene,
                            snap,
                            self.sim.map_w,
                            mover_r,
                            pack,
                        );
                        let mover_globals = sow_render::MoverGlobals {
                            camera_pos: [self.input.camera_x, self.input.camera_y],
                            zoom: self.input.camera_zoom,
                            sprite_count: 0,
                            screen_size: [self.input.screen_w, self.input.screen_h],
                            trail_count: 0,
                            _pad: 0.0,
                        };
                        mover_r.draw(
                            &mut render_ctx.command_encoder,
                            frame.texture_view(),
                            mover_globals,
                            &render_ctx.context,
                        );
                    }
                }
            }

            if !map_drawn {
                let pass = render_ctx.command_encoder.render(
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

            // Restore the context so UI-action handlers (e.g. opening the map
            // editor) can take ownership of it during `run_ui` below. The
            // command encoder stays mid-frame on the same object across the
            // put/re-take, so UI painting continues recording into it.
            self.gfx.render_ctx = Some(render_ctx);

            // ── UI UPDATE ───────────────────────────────────────
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
            // Frame-based decay smoothed out UI; sim snapshots keep it strictly synced.
            if let Some(ref mut secs) = self.ui.app.hud_state.spawn_timer_secs {
                if self.net.client.is_some() {
                    *secs = (*secs - self.ui.raw_input.predicted_dt).max(0.0);
                }
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

            // Re-take the context for UI painting. If a UI action (map editor)
            // took ownership during `run_ui`, the editor now drives rendering;
            // drop this frame and let the editor take over next tick.
            let Some(mut render_ctx) = self.gfx.render_ctx.take() else {
                return;
            };

            #[cfg(target_arch = "wasm32")]
            {
                use sow_ui::app::ClientPhase;
                if !self.web_loader_hidden && self.ui.app.phase != ClientPhase::Splash {
                    crate::loader::hide_web_loader();
                    self.web_loader_hidden = true;
                }
            }

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

            crate::platform_output::handle_egui_platform_output(&egui_output.platform_output);

            self.ui.raw_input.events.clear();

            // ── DRAWING UI ──────────────────────────────────────────
            if let Some(ref mut gp) = self.gfx.gui_painter {
                let screen_desc = blade_egui::ScreenDescriptor {
                    physical_size: (self.input.screen_w as u32, self.input.screen_h as u32),
                    scale_factor: sf,
                };
                let paint_jobs = self.ui.egui_ctx.tessellate(egui_output.shapes, sf);
                gp.update_textures(
                    &mut render_ctx.command_encoder,
                    &egui_output.textures_delta,
                    &render_ctx.context,
                );

                let mut pass = render_ctx.command_encoder.render(
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

                gp.paint(&mut pass, &paint_jobs, &screen_desc, &render_ctx.context);
                drop(pass);
            }
            if let Some(ref mut gp) = self.gfx.gui_painter {
                gp.sync(&render_ctx.context);
            }
            render_ctx.command_encoder.present(frame);
            let sync_point = render_ctx.context.submit(&mut render_ctx.command_encoder);

            if let Some(ref mut gp) = self.gfx.gui_painter {
                gp.after_submit(&sync_point);
            }

            self.gfx.prev_sync_point = Some(sync_point);
            self.gfx.render_ctx = Some(render_ctx);
        }
    }
}

impl SowApp {
    pub fn check_surface(&mut self) {
        if !self.ensure_render_ctx() {
            return;
        }
        if self.gfx.surface.is_none() {
            if let Some(ref win) = self.gfx.window {
                #[cfg(target_arch = "wasm32")]
                let (pw, ph) = crate::web_canvas::physical_viewport_size();
                #[cfg(target_arch = "wasm32")]
                let sz = winit::dpi::PhysicalSize::new(pw.max(1), ph.max(1));
                #[cfg(not(target_arch = "wasm32"))]
                let sz = win.surface_size();

                let Some(render_ctx) = self.gfx.render_ctx.take() else {
                    return;
                };

                #[cfg(target_arch = "wasm32")]
                crate::web_canvas::set_canvas_backing_store_size(sz.width, sz.height);

                match render_ctx.create_surface(win, sz.width, sz.height) {
                    Ok(s) => {
                        self.gfx.configured_physical = sz;
                        // ponytail: query device_pixel_ratio directly as winit scale_factor is 1.0 initially
                        #[cfg(target_arch = "wasm32")]
                        let sf = web_sys::window()
                            .map(|window| window.device_pixel_ratio() as f32)
                            .unwrap_or(1.0);
                        #[cfg(not(target_arch = "wasm32"))]
                        let sf = win.scale_factor() as f32;

                        let vp = crate::viewport::Viewport::from_configured(self, sf);
                        vp.sync_to_app(self);
                        let zmax =
                            camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h);
                        self.input.camera_zoom =
                            self.input.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);
                        let format = s.info().format;

                        if let Some(sp) = self.gfx.prev_sync_point.take() {
                            let _ = render_ctx.context.wait_for(&sp, !0);
                        }
                        if let Some(mut old_mr) = self.gfx.map_renderer.take() {
                            let old_terrain = old_mr.terrain.clone();
                            old_mr.destroy(&render_ctx);
                            self.gfx.map_renderer = Some(sow_render::MapRenderer::new(
                                &render_ctx.context,
                                self.sim.map_w,
                                self.sim.map_h,
                                format,
                                &old_terrain,
                            ));
                            self.gfx.needs_first_upload = true;
                        }
                        if let Some(mut old_mover) = self.gfx.mover_renderer.take() {
                            old_mover.destroy(&render_ctx);
                        }
                        self.gfx.mover_renderer =
                            Some(sow_render::MoverRenderer::new(&render_ctx.context, format));
                        if let Some(mut old_gp) = self.gfx.gui_painter.take() {
                            old_gp.destroy(&render_ctx.context);
                        }

                        self.gfx.gui_painter =
                            Some(blade_egui::GuiPainter::new(s.info(), &render_ctx.context));
                        self.gfx.surface = Some(s);
                        self.gfx.render_ctx = Some(render_ctx);

                        self.ui.egui_ctx = egui::Context::default();
                        sow_ui::ui::theme::apply_theme(&self.ui.egui_ctx);
                        log::info!("Successfully created surface on retry.");
                    }
                    Err(e) => {
                        self.gfx.render_ctx = Some(render_ctx);
                        log::warn!("Surface creation failed: {:?}", e);
                    }
                }
            }
        }
    }
}
