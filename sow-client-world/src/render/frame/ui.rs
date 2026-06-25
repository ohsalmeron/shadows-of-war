use crate::app::SowApp;
use blade_graphics as gpu;
use sow_ui_kit::ClientPhase;
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

        // Keyboard map panning (WASD and Arrows)
        if self.ui.app.phase == ClientPhase::Playing
            && !self.ui.egui_ctx.egui_wants_keyboard_input()
        {
            let dx = self.input.key_pan_left as i32 as f32 - self.input.key_pan_right as i32 as f32;
            let dy = self.input.key_pan_up as i32 as f32 - self.input.key_pan_down as i32 as f32;
            if dx != 0.0 || dy != 0.0 {
                let pan = 1000.0 * dt;
                self.input.camera_x += dx * pan;
                self.input.camera_y += dy * pan;
                if let Some(win) = self.gfx.window.as_ref() {
                    win.request_redraw();
                }
            }
        }

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
        if let Some(ref mut tr) = self.gfx.text_renderer {
            tr.begin_frame();
        }
        if let Some(ref mut sr) = self.gfx.structure_renderer {
            sr.begin_frame();
        }
        let egui_output = egui_ctx.run_ui(self.ui.raw_input.clone(), |ctx| {
            let insets = ctx.input(|i| i.safe_area_insets());
            self.ui.app.hud_state.safe_area_top = insets.0.top;
            self.ui.app.hud_state.safe_area_bottom = insets.0.bottom;

            if self.ui.app.phase == sow_ui_kit::ClientPhase::Playing {
                self.render_world_overlays(ctx, sf);
                self.render_tutorial_ui(ctx);
            }

            self.calculate_fps_and_ping();
            self.render_stats_overlay(ctx);

            if self.ui.app.phase == ClientPhase::Playing {
                self.handle_map_interactions(ctx);
                self.render_endgame_ui(ctx);
                self.render_leaderboard(ctx);
                self.render_player_hover_panel(ctx);
                self.render_placement_cancel_button(ctx);
            }

            #[cfg(any(feature = "dev", debug_assertions))]
            self.render_dev_panels(ctx);
            sow_ui_kit::theme::publish_lobby_modal_embed(
                ctx,
                crate::store_portals::is_lobby_modal_embed(),
            );
            self.ui.app.hud_state.is_tutorial = self.ui.tutorial_active;
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

        if let Some(ref mut sr) = self.gfx.structure_renderer {
            sr.draw(
                &mut render_ctx.command_encoder,
                frame.texture_view(),
                [self.input.camera_x, self.input.camera_y],
                self.input.camera_zoom,
                [self.input.screen_w, self.input.screen_h],
                &render_ctx.context,
            );
        }

        if let Some(ref mut tr) = self.gfx.text_renderer {
            tr.draw(
                &mut render_ctx.command_encoder,
                frame.texture_view(),
                [self.input.screen_w, self.input.screen_h],
                &render_ctx.context,
            );
        }

        #[cfg(target_arch = "wasm32")]
        {
            use sow_ui_kit::ClientPhase;
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
            name: String,
            player_type: sow_core::player::PlayerType,
            player_color_raw: [f32; 3],
            player_color_vibrant: egui::Color32,
            leader: sow_core::player::Leader,
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

            let vibrant_color =
                crate::hud::nameplate::ensure_readable_nameplate_color(player.color);
            let troops_label = format!(
                "🛡️ {}/{}",
                sow_ui_kit::utils::format_number(player.troops),
                sow_ui_kit::utils::format_number(player.max_troops)
            );
            let gold_label = format!("🪙 {}", sow_ui_kit::utils::format_number(player.gold));
            let tiles_label = format!(
                "🌽 {}",
                sow_ui_kit::utils::format_number(player.tile_count as f64)
            );
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
                name: player.name.clone(),
                player_type: player.player_type,
                player_color_raw: player.color,
                player_color_vibrant: vibrant_color,
                leader: player.leader,
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
                        sow_ui_kit::theme::palette::field_border(),
                    ))
                    .corner_radius(12)
                    .inner_margin(egui::Margin {
                        left: 14,
                        right: 14,
                        top: 8,
                        bottom: 8,
                    }),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Avatar/Emoji representation or Spirit Animal if any
                    let (avatar_rect, _) =
                        ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                    crate::hud::avatar::draw_player_avatar(
                        ui.painter(),
                        avatar_rect.center(),
                        10.0, // radius
                        info.owner_id,
                        &info.name,
                        info.player_type,
                        info.player_color_raw,
                        &info.leader,
                        &self.ui.app.asset_loader,
                    );

                    ui.label(
                        egui::RichText::new(&info.name)
                            .size(14.0)
                            .strong()
                            .color(info.player_color_vibrant),
                    );

                    ui.add(egui::Separator::default().vertical());

                    // Stats
                    sow_ui_kit::widgets::emoji_label(
                        ui,
                        &info.troops_label,
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_rgb(34, 211, 238),
                    );
                    ui.add_space(6.0);
                    sow_ui_kit::widgets::emoji_label(
                        ui,
                        &info.gold_label,
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_rgb(250, 204, 21),
                    );
                    ui.add_space(6.0);
                    sow_ui_kit::widgets::emoji_label(
                        ui,
                        &info.tiles_label,
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_gray(210),
                    );

                    // Structures (if any)
                    if info.city_label.is_some()
                        || info.bunker_label.is_some()
                        || info.factory_label.is_some()
                        || info.port_label.is_some()
                    {
                        ui.add(egui::Separator::default().vertical());

                        if let Some(ref l) = info.city_label {
                            sow_ui_kit::widgets::emoji_label(
                                ui,
                                l,
                                egui::FontId::proportional(12.0),
                                egui::Color32::from_gray(210),
                            );
                            ui.add_space(4.0);
                        }
                        if let Some(ref l) = info.bunker_label {
                            sow_ui_kit::widgets::emoji_label(
                                ui,
                                l,
                                egui::FontId::proportional(12.0),
                                egui::Color32::from_gray(210),
                            );
                            ui.add_space(4.0);
                        }
                        if let Some(ref l) = info.factory_label {
                            sow_ui_kit::widgets::emoji_label(
                                ui,
                                l,
                                egui::FontId::proportional(12.0),
                                egui::Color32::from_gray(210),
                            );
                            ui.add_space(4.0);
                        }
                        if let Some(ref l) = info.port_label {
                            sow_ui_kit::widgets::emoji_label(
                                ui,
                                l,
                                egui::FontId::proportional(12.0),
                                egui::Color32::from_gray(210),
                            );
                        }
                    }
                });
            });
    }

    pub(crate) fn render_placement_cancel_button(&mut self, ctx: &egui::Context) {
        let has_building = self.ui.app.hud_state.selected_building_kind.is_some();
        if !has_building && self.ui.app.hud_state.selected_nuke_kind.is_none() {
            return;
        }

        let bottom_panel_rect =
            ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("hud_bottom_panel_rect")));

        let safe_area_bottom = self.ui.app.hud_state.safe_area_bottom;
        let y_pos = if let Some(rect) = bottom_panel_rect {
            rect.top() - 12.0
        } else {
            ctx.input(|i| i.content_rect()).bottom() - 140.0 - safe_area_bottom
        };

        egui::Area::new(egui::Id::new("placement_cancel_area"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(
                ctx.input(|i| i.content_rect()).center().x,
                y_pos,
            ))
            .pivot(egui::Align2::CENTER_BOTTOM)
            .show(ctx, |ui| {
                let frame = egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(20, 10, 15, 220))
                    .stroke(egui::Stroke::new(
                        1.5_f32,
                        egui::Color32::from_rgb(239, 68, 68),
                    ))
                    .corner_radius(16)
                    .inner_margin(egui::Margin::symmetric(16, 8));

                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;

                        let text = if let Some(kind) = self.ui.app.hud_state.selected_building_kind
                        {
                            let emoji = match kind {
                                sow_core::game::BuildingKind::City => "🏛️",
                                sow_core::game::BuildingKind::Bunker => "🛡️",
                                sow_core::game::BuildingKind::Factory => "🏭",
                                _ => "⚓",
                            };
                            format!("Placing {emoji}... ")
                        } else {
                            "Placing Nuke... ".to_string()
                        };

                        ui.label(
                            egui::RichText::new(text)
                                .size(13.0)
                                .color(egui::Color32::from_rgb(220, 220, 220)),
                        );

                        let cancel_btn = sow_ui_kit::widgets::ThemeButton::new("❌ Cancel")
                            .style(sow_ui_kit::widgets::ThemeButtonStyle::Danger)
                            .min_size(egui::vec2(80.0, 28.0))
                            .text_size(12.0)
                            .corner_radius(10);

                        if ui.add(cancel_btn).clicked() {
                            self.ui.app.hud_state.selected_building_kind = None;
                            self.ui.app.hud_state.selected_nuke_kind = None;
                        }
                    });
                });
            });
    }
}
