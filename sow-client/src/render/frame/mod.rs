use crate::app::SowApp;
use crate::render::gpu::MapGlobals;
use blade_graphics as gpu;
use sow_ui_kit::ClientPhase;

mod ui;

impl SowApp {
    pub fn render_frame(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        sow_ui_kit::register_game_assets(&self.ui.egui_ctx);

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
        let spatial_audio = self.spatial_audio_ctx();

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
                    if let Some(ref tr) = self.gfx.text_renderer {
                        tr.init_textures(&mut render_ctx.command_encoder);
                        tr.upload_atlas(&mut render_ctx.command_encoder, &render_ctx.context);
                    }
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
                                spatial_audio.params(dx + 0.5, dy + 0.5),
                            );

                            let blast_radius = sow_core::game::nuke_inner_radius(level) as f32;
                            self.ui.fallout_zones.push(crate::app::FalloutZone {
                                x: dx,
                                y: dy,
                                radius: blast_radius,
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
                                sow_audio::play_nuke_launch_sound(
                                    spatial_audio.params(src_x, src_y),
                                );

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

                    let dev = sow_ui_kit::theme::dev_config::DevConfig::get();
                    let border_thickness = dev.thickness;
                    let border_darkness = dev.darkness;
                    let shore_thickness = dev.shore_thickness;
                    let shore_darkness = dev.shore_darkness;
                    let territory_opacity = dev.territory_opacity;
                    let blend_mode = dev.blend_mode;
                    let conquest_duration = dev.conquest_duration;

                    let dirty = self
                        .sim
                        .current_snapshot
                        .as_ref()
                        .map(|s| s.dirty_tiles.as_slice())
                        .unwrap_or(&[]);

                    thread_local! {
                        static LAST_FOG_OF_WAR_TOGGLE: std::cell::Cell<Option<bool>> = std::cell::Cell::new(None);
                    }
                    let mut force_fog_upload = self.sim.force_fog_upload;
                    self.sim.force_fog_upload = false;
                    let toggle_changed = LAST_FOG_OF_WAR_TOGGLE.with(|cell| {
                        let prev = cell.get();
                        if prev != Some(dev.fog_of_war) {
                            cell.set(Some(dev.fog_of_war));
                            true
                        } else {
                            false
                        }
                    });
                    if toggle_changed {
                        force_fog_upload = true;
                    }

                    mr.update(
                        &mut render_ctx.command_encoder,
                        &render_ctx.context,
                        dirty,
                        conquest_duration,
                        &self.sim.fog_explored,
                        &self.sim.fog_visible,
                        force_fog_upload,
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
                                // Team players render with the planar team color;
                                // everyone else keeps their personal color.
                                let rgb = p
                                    .team
                                    .map_or(p.color, sow_core::player::team_territory_rgb);
                                player_colors[p.id as usize] = [rgb[0], rgb[1], rgb[2], 1.0];
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
                            let norm = (attack.troops as f32 / 100_000.0).clamp(0.0, 1.5);
                            let intensity = norm.powi(2);
                            let radius = 5.0 + intensity * 75.0;
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
                            let duration = 5.0;
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

                    // ── Attack Border Flash ──
                    let (attack_flash_target, attack_flash_t) = {
                        let mut player_intensities: std::collections::HashMap<u16, f32> =
                            std::collections::HashMap::new();
                        self.ui.border_flashes.retain(|flash| {
                            let elapsed =
                                current_time.duration_since(flash.start_time).as_secs_f32();
                            if let Some(t) = crate::app::easeout_flash(elapsed) {
                                let entry =
                                    player_intensities.entry(flash.player_id).or_insert(0.0);
                                *entry += t * flash.max_intensity;
                                true
                            } else {
                                false
                            }
                        });

                        player_intensities
                            .into_iter()
                            .max_by(|a, b| {
                                a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .map(|(id, t)| (id as f32, t.min(1.5)))
                            .unwrap_or((0.0, 0.0))
                    };

                    // ── Viewport Alert Vignette ──
                    let currently_under_attack = if let Some(snap) = &self.sim.current_snapshot {
                        let my_id = self.sim.my_player_id.unwrap_or(0);
                        my_id != 0
                            && snap
                                .attacks
                                .iter()
                                .any(|a| a.target_owner == my_id && a.troops > 0.0)
                    } else {
                        false
                    };

                    if currently_under_attack {
                        self.ui
                            .trigger_viewport_alert(crate::app::ViewportAlertKind::UnderAttack);
                    } else if let Some(ref current) = self.ui.viewport_alert {
                        if current.kind == crate::app::ViewportAlertKind::UnderAttack {
                            self.ui.viewport_alert = None;
                        }
                    }

                    let (alert_color, alert_intensity) = if let Some(ref alert) =
                        self.ui.viewport_alert
                    {
                        let kind = alert.kind;
                        let elapsed = current_time.duration_since(alert.start_time).as_secs_f32();
                        let persistent = kind == crate::app::ViewportAlertKind::UnderAttack
                            || kind == crate::app::ViewportAlertKind::Victory
                            || kind == crate::app::ViewportAlertKind::Defeat;

                        let intensity = if persistent {
                            (elapsed / crate::app::FLASH_DURATION).min(1.0)
                        } else {
                            crate::app::easeout_flash(elapsed).unwrap_or(0.0)
                        };

                        let color = match kind {
                            crate::app::ViewportAlertKind::UnderAttack => [1.0, 0.05, 0.05, 0.55],
                            crate::app::ViewportAlertKind::Victory => [0.1, 0.9, 0.25, 0.45],
                            crate::app::ViewportAlertKind::Defeat => [0.25, 0.25, 0.3, 0.5],
                            crate::app::ViewportAlertKind::ConquerPlayer => [0.95, 0.8, 0.0, 0.5],
                            crate::app::ViewportAlertKind::AllianceRequest => [0.0, 0.8, 0.8, 0.4],
                            crate::app::ViewportAlertKind::Betrayal => [0.6, 0.0, 0.8, 0.65],
                        };
                        (color, intensity)
                    } else {
                        ([0.0f32; 4], 0.0f32)
                    };
                    // Clear expired one-shot alerts
                    if alert_intensity <= 0.0 {
                        self.ui.viewport_alert = None;
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
                        effect_shockwave: if dev.vfx_conquer { 1.0 } else { 0.0 },
                        effect_breathe: if dev.vfx_border_breathe { 1.0 } else { 0.0 },
                        effect_energy_flow: if dev.vfx_energy_flow { 1.0 } else { 0.0 },
                        my_player_id: self.sim.my_player_id.unwrap_or(0) as f32,
                        hover_hex,
                        hover_building_kind,
                        territory_opacity,
                        fallout_slots,
                        nobuild_slots,
                        blend_mode,
                        effect_heartbeat: if dev.vfx_heartbeat { 1.0 } else { 0.0 },
                        effect_war_fog: if dev.vfx_war_fog { 1.0 } else { 0.0 },
                        effect_fallout: if dev.vfx_fallout { 1.0 } else { 0.0 },
                        effect_golden_hour: if dev.vfx_ambient_grade { 1.0 } else { 0.0 },
                        effect_holo_grid: if dev.vfx_holo_grid { 1.0 } else { 0.0 },
                        attack_flash_target,
                        attack_flash_t,
                        alert_intensity,
                        fog_of_war: if dev.fog_of_war { 1.0 } else { 0.0 },
                        _pad1: 0.0,
                        _pad2: 0.0,
                        alert_color,
                    };
                    let colors_struct = crate::render::gpu::PlayerColors {
                        colors: player_colors,
                    };
                    mr.draw(
                        &mut render_ctx.command_encoder,
                        frame.texture_view(),
                        globals,
                        colors_struct,
                    );
                    map_drawn = true;

                    if self.gfx.text_renderer.is_none() {
                        let surface_format = s.info().format;
                        let tr = crate::render::gpu::TextRenderer::new(
                            &render_ctx.context,
                            surface_format,
                        );
                        tr.init_textures(&mut render_ctx.command_encoder);
                        tr.upload_atlas(&mut render_ctx.command_encoder, &render_ctx.context);
                        self.gfx.text_renderer = Some(tr);
                    }

                    // ── GPU-instanced movers (boats, nukes, SAM) ─────────────
                    if dev.vfx_mover_trails {
                        if self.gfx.mover_renderer.is_none() {
                            let surface_format = s.info().format;
                            self.gfx.mover_renderer = Some(crate::render::gpu::MoverRenderer::new(
                                &render_ctx.context,
                                surface_format,
                            ));
                            if let Some(ref mr_mover) = self.gfx.mover_renderer {
                                mr_mover.upload_atlas(
                                    &mut render_ctx.command_encoder,
                                    &render_ctx.context,
                                );
                            }
                        }
                        if let (Some(mover_r), Some(snap)) =
                            (&mut self.gfx.mover_renderer, &self.sim.current_snapshot)
                        {
                            let now = web_time::Instant::now();
                            let alpha = crate::render::world::movers::interp_alpha(&self.time, now);
                            let linear_alpha = self.time.interp.linear_alpha(now);
                            let pack = crate::render::world::movers::MoverPackParams {
                                camera_x: self.input.camera_x,
                                camera_y: self.input.camera_y,
                                camera_zoom: self.input.camera_zoom,
                                screen_w: self.input.screen_w,
                                screen_h: self.input.screen_h,
                                alpha,
                                linear_alpha,
                                selected_warships: &self.input.selected_warships,
                            };
                            crate::render::world::movers::update_and_pack(
                                &mut self.ui.mover_scene,
                                snap,
                                self.sim.map_w,
                                mover_r,
                                pack,
                                dev.fog_of_war,
                                self.sim.my_player_id.unwrap_or(0),
                                &self.sim.fog_visible,
                            );
                            let mover_globals = crate::render::gpu::MoverGlobals {
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

            self.render_frame_ui_and_present(sf, frame);
        }
    }
}
