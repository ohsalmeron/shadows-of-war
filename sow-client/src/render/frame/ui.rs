use crate::app::SowApp;
use blade_graphics as gpu;
use sow_ui::app::ClientPhase;
use web_time::Instant;

impl SowApp {
    pub(super) fn render_frame_ui_and_present(&mut self, sf: f32, frame: blade_graphics::Frame) {
        // ── UI UPDATE ───────────────────────────────────────
        let frame_now = Instant::now();
        let dt = frame_now
            .duration_since(self.time.last_frame_time)
            .as_secs_f32();
        self.time.last_frame_time = frame_now;
        self.ui.raw_input.predicted_dt = dt.min(0.1);

        // Smooth Zoom Lerp
        let diff = self.input.target_zoom - self.input.camera_zoom;
        if diff.abs() > 0.0001 {
            let lerp_factor = (1.0 - f32::exp(-12.0 * dt)).clamp(0.0, 1.0);
            let new_zoom = self.input.camera_zoom + diff * lerp_factor;

            let cx = self.input.last_mouse_x as f32;
            let cy = self.input.last_mouse_y as f32;

            let old_zoom = self.input.camera_zoom;
            self.input.camera_zoom = new_zoom;

            let map_x = (cx - self.input.camera_x) / old_zoom;
            let map_y = (cy - self.input.camera_y) / old_zoom;
            self.input.camera_x = cx - map_x * self.input.camera_zoom;
            self.input.camera_y = cy - map_y * self.input.camera_zoom;

            if let Some(win) = self.gfx.window.as_ref() {
                win.request_redraw();
            }
        }

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
            sync.time_remaining = (sync.time_remaining - self.ui.raw_input.predicted_dt).max(0.0);
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
            let insets = ctx.input(|i| i.safe_area_insets());
            self.ui.app.hud_state.safe_area_top = insets.0.top;
            self.ui.app.hud_state.safe_area_bottom = insets.0.bottom;

            if self.ui.app.phase == sow_ui::app::ClientPhase::Playing {
                self.render_world_overlays(ctx, sf);
                self.render_tutorial_ui(ctx);
            }

            self.calculate_fps_and_ping();

            if self.ui.app.phase == ClientPhase::Playing {
                self.handle_map_interactions(ctx);
                self.render_endgame_ui(ctx);
                self.render_leaderboard(ctx);
                self.render_player_hover_panel(ctx);
            }

            self.render_dev_panels(ctx);
            sow_ui::ui::theme::publish_lobby_modal_embed(
                ctx,
                crate::store_portals::is_lobby_modal_embed(),
            );
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

                    let request_data = winit::window::ImeRequestData::default().with_cursor_area(
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
                        if let Some(req) = winit::window::ImeEnableRequest::new(caps, request_data)
                        {
                            let _ = win.request_ime_update(winit::window::ImeRequest::Enable(req));
                        }
                    } else {
                        let _ =
                            win.request_ime_update(winit::window::ImeRequest::Update(request_data));
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

    pub(crate) fn render_player_hover_panel(&self, ctx: &egui::Context) {
        // Ponytail: YAGNI - skip entirely on mobile / small screens
        if ctx.input(|i| i.viewport_rect().width()) < 720.0 {
            return;
        }

        // Get hovered tile index from mouse position
        let mx = self.input.last_mouse_x as f32;
        let my = self.input.last_mouse_y as f32;
        let world_x = (mx - self.input.camera_x) / self.input.camera_zoom;
        let world_y = (my - self.input.camera_y) / self.input.camera_zoom;
        let (h_col, h_row) = crate::render::world::movers::world_to_tile(world_x, world_y);

        let hovered_tile_idx = if h_col >= 0
            && h_row >= 0
            && h_col < self.sim.map_w as i32
            && h_row < self.sim.map_h as i32
        {
            Some((h_row * self.sim.map_w as i32 + h_col) as u32)
        } else {
            None
        };

        let Some(idx) = hovered_tile_idx else {
            return;
        };

        let hovered_owner = self
            .gfx
            .map_renderer
            .as_ref()
            .and_then(|mr| mr.owners.get(idx as usize).copied())
            .unwrap_or(0);

        if hovered_owner == 0 {
            return;
        }

        let Some(snap) = &self.sim.current_snapshot else {
            return;
        };

        // ponytail: cached in Arc with pre-formatted labels to avoid any allocations/formatting in the render loop
        #[derive(Debug)]
        struct CachedHover {
            owner_id: u16,
            tick: u64,
            flag_emoji: String,
            name: String,
            player_color: egui::Color32,
            type_label: String,
            leader_civ_label: String,
            troops_label: String,
            gold_label: String,
            tiles_label: String,
            city_label: Option<String>,
            bunker_label: Option<String>,
            factory_label: Option<String>,
            port_label: Option<String>,
        }

        let cached = ctx.data(|d| {
            d.get_temp::<std::sync::Arc<CachedHover>>(egui::Id::new("player_hover_cache"))
        });

        let info = if let Some(c) =
            cached.filter(|c| c.owner_id == hovered_owner && c.tick == snap.tick)
        {
            c
        } else {
            let Some(player) = snap.players.iter().find(|p| p.id == hovered_owner) else {
                return;
            };

            // Count their buildings
            let mut city_count = 0;
            let mut bunker_count = 0;
            let mut factory_count = 0;
            let mut port_count = 0;
            for b in &snap.buildings {
                if b.owner_id == hovered_owner {
                    match b.kind {
                        sow_core::game::BuildingKind::City => city_count += 1,
                        sow_core::game::BuildingKind::Bunker => bunker_count += 1,
                        sow_core::game::BuildingKind::Factory => factory_count += 1,
                        sow_core::game::BuildingKind::Port => port_count += 1,
                    }
                }
            }

            let type_str = match player.player_type {
                sow_core::player::PlayerType::Human => "Human",
                sow_core::player::PlayerType::Bot => "AI Bot",
                sow_core::player::PlayerType::Nation => "AI Nation",
            };

            let player_color = egui::Color32::from_rgb(
                (player.color[0] * 255.0) as u8,
                (player.color[1] * 255.0) as u8,
                (player.color[2] * 255.0) as u8,
            );

            let flag_emoji = player.active_emoji.as_deref().unwrap_or("🏳️").to_string();
            let type_label = format!("({})", type_str);
            let leader_civ_label = format!(
                "Leader: {} | Civilization: {}",
                player.leader.name(),
                player.civilization.name()
            );
            let troops_label = format!("🛡️ {:.0}/{:.0}", player.troops, player.max_troops);
            let gold_label = format!("🪙 {:.0}", player.gold);
            let tiles_label = format!("🏳️ {} tiles", player.tile_count);
            let city_label = if city_count > 0 {
                Some(format!("🏛️ x{}", city_count))
            } else {
                None
            };
            let bunker_label = if bunker_count > 0 {
                Some(format!("🛡️ x{}", bunker_count))
            } else {
                None
            };
            let factory_label = if factory_count > 0 {
                Some(format!("🏭 x{}", factory_count))
            } else {
                None
            };
            let port_label = if port_count > 0 {
                Some(format!("⚓ x{}", port_count))
            } else {
                None
            };

            let new_c = std::sync::Arc::new(CachedHover {
                owner_id: hovered_owner,
                tick: snap.tick,
                flag_emoji,
                name: player.name.clone(),
                player_color,
                type_label,
                leader_civ_label,
                troops_label,
                gold_label,
                tiles_label,
                city_label,
                bunker_label,
                factory_label,
                port_label,
            });

            ctx.data_mut(|d| d.insert_temp(egui::Id::new("player_hover_cache"), new_c.clone()));
            new_c
        };

        let safe_area_top = ctx.input(|i| i.safe_area_insets().0.top);

        egui::Window::new("Player Hover Info")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(
                egui::Align2::CENTER_TOP,
                egui::vec2(0.0, 12.0 + safe_area_top),
            )
            .frame(
                egui::Frame::window(&ctx.global_style())
                    .fill(egui::Color32::from_black_alpha(150))
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        sow_ui::ui::theme::palette::field_border(),
                    ))
                    .corner_radius(12)
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 10,
                        bottom: 10,
                    }),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Avatar/Emoji representation or Spirit Animal if any
                    let (icon_rect, _) =
                        ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
                    if !sow_ui::widgets::try_paint_emoji(
                        ui.painter(),
                        &info.flag_emoji,
                        icon_rect,
                        egui::Color32::WHITE,
                    ) {
                        ui.label(egui::RichText::new(&info.flag_emoji).size(24.0));
                    }

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&info.name)
                                    .size(16.0)
                                    .strong()
                                    .color(info.player_color),
                            );

                            ui.label(
                                egui::RichText::new(&info.type_label)
                                    .size(11.0)
                                    .color(egui::Color32::from_gray(140)),
                            );
                        });

                        ui.label(
                            egui::RichText::new(&info.leader_civ_label)
                                .size(12.0)
                                .color(egui::Color32::from_gray(180)),
                        );
                    });

                    ui.add(egui::Separator::default().vertical());

                    // Stats Grid
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            sow_ui::widgets::emoji_label(
                                ui,
                                &info.troops_label,
                                egui::FontId::proportional(13.0),
                                egui::Color32::from_rgb(34, 211, 238),
                            );
                            ui.add_space(8.0);
                            sow_ui::widgets::emoji_label(
                                ui,
                                &info.gold_label,
                                egui::FontId::proportional(13.0),
                                egui::Color32::from_rgb(250, 204, 21),
                            );
                            ui.add_space(8.0);
                            sow_ui::widgets::emoji_label(
                                ui,
                                &info.tiles_label,
                                egui::FontId::proportional(13.0),
                                egui::Color32::from_gray(210),
                            );
                        });

                        ui.horizontal(|ui| {
                            if let Some(ref l) = info.city_label {
                                sow_ui::widgets::emoji_label(
                                    ui,
                                    l,
                                    egui::FontId::proportional(12.0),
                                    egui::Color32::from_gray(210),
                                );
                            }
                            if let Some(ref l) = info.bunker_label {
                                sow_ui::widgets::emoji_label(
                                    ui,
                                    l,
                                    egui::FontId::proportional(12.0),
                                    egui::Color32::from_gray(210),
                                );
                            }
                            if let Some(ref l) = info.factory_label {
                                sow_ui::widgets::emoji_label(
                                    ui,
                                    l,
                                    egui::FontId::proportional(12.0),
                                    egui::Color32::from_gray(210),
                                );
                            }
                            if let Some(ref l) = info.port_label {
                                sow_ui::widgets::emoji_label(
                                    ui,
                                    l,
                                    egui::FontId::proportional(12.0),
                                    egui::Color32::from_gray(210),
                                );
                            }
                            if info.city_label.is_none()
                                && info.bunker_label.is_none()
                                && info.factory_label.is_none()
                                && info.port_label.is_none()
                            {
                                ui.label(
                                    egui::RichText::new("No structures placed")
                                        .size(11.0)
                                        .italics()
                                        .color(egui::Color32::from_gray(130)),
                                );
                            }
                        });
                    });
                });
            });
    }
}
