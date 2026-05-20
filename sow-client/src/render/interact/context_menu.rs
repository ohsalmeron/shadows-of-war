use crate::app::SowApp;
use egui::{Color32, Stroke};

impl SowApp {
    pub(crate) fn draw_context_menu(&mut self, ctx: &egui::Context) {
        if let Some((mx, my, tile_idx)) = self.input.map_context_menu_active {
            let my_id = self.sim.my_player_id.unwrap_or(1);
            let owner_id = self.gfx.map_renderer.as_ref()
                .map(|mr| mr.owners[tile_idx as usize])
                .unwrap_or(0);

            let is_own_territory = owner_id == my_id;
            let is_friendly = owner_id != 0 && owner_id != my_id;

            let owner_snapshot = self.sim.current_snapshot.as_ref()
                .and_then(|s| s.players.iter().find(|p| p.id == owner_id));

            let my_snapshot = self.sim.current_snapshot.as_ref()
                .and_then(|s| s.players.iter().find(|p| p.id == my_id));

            let is_allied = if let Some(owner) = owner_snapshot {
                owner.alliances.contains(&my_id)
            } else {
                false
            };

            let has_alliance_request = my_snapshot
                .map(|p| p.alliance_requests.contains(&owner_id))
                .unwrap_or(false);
                
            let has_proposed_alliance = owner_snapshot
                .map(|p| p.alliance_requests.contains(&my_id))
                .unwrap_or(false);

            let is_spawning = self.sim.current_snapshot.as_ref()
                .map(|s| matches!(s.phase, sow_core::game::GamePhase::Spawning { .. }))
                .unwrap_or(false);

            let col = tile_idx % self.sim.map_w;
            let row = tile_idx / self.sim.map_w;

            // Egui memory key ids for popovers
            let build_active_id = egui::Id::new("radial_build_active");
            let radial_build_active: bool = ctx.data(|d| d.get_temp(build_active_id).unwrap_or(false));

            // Animation scaling
            let is_open_target = self.input.map_context_menu.is_some();
            let animation_id = egui::Id::new(("radial_menu_scale", tile_idx, self.input.map_context_menu_session));
            let duration = if is_open_target { 0.22 } else { 0.12 };
            let progress = ctx.animate_bool_with_time(animation_id, is_open_target, duration);

            if progress <= 0.0 && !is_open_target {
                self.input.map_context_menu_active = None;
            }

            // Disney overshoot curve
            let spring_scale = if is_open_target {
                1.0 - (progress * 7.5).cos() * (-3.5 * progress).exp()
            } else {
                progress
            };
            let scale = spring_scale.clamp(0.0, 1.25);

            let center = egui::pos2(mx, my);
            let pointer_pos = ctx.input(|i| i.pointer.interact_pos());

            let r_center = 36.0 * scale;
            let inner_r = 40.0 * scale;
            let outer_r = 95.0 * scale;

            // Helper to get active zone for a given mouse/touch coordinate
            let get_zone_at = |pos: egui::Pos2| -> (bool, Option<usize>) {
                let dist = pos.distance(center);
                if dist <= r_center {
                    (true, None)
                } else if dist > inner_r && dist <= outer_r + 15.0 && scale > 0.2 {
                    let angle = (pos.y - center.y).atan2(pos.x - center.x);
                    let pi = std::f32::consts::PI;
                    if angle >= -3.0 * pi / 4.0 && angle < -pi / 4.0 {
                        (false, Some(0)) // Top Emojis
                    } else if angle >= -pi / 4.0 && angle < pi / 4.0 {
                        (false, Some(1)) // Right Boat
                    } else if angle >= pi / 4.0 && angle < 3.0 * pi / 4.0 {
                        (false, Some(2)) // Bottom Alliances
                    } else {
                        (false, Some(3)) // Left Build
                    }
                } else {
                    (false, None)
                }
            };

            let (hovered_center, hovered_sector) = pointer_pos.map(get_zone_at).unwrap_or((false, None));

            egui::Area::new(egui::Id::new("map_context_menu_area"))
                .fixed_pos(center - egui::vec2(150.0, 150.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    // Intercept hover/drag BEFORE borrowing painter
                    let (_rect, _response) = ui.allocate_exact_size(egui::vec2(300.0, 300.0), egui::Sense::click_and_drag());
                    let painter = ui.painter();

                    let get_wedge_points = |inner_r: f32, outer_r: f32, a_start: f32, a_end: f32| -> Vec<egui::Pos2> {
                        let mut pts = Vec::new();
                        let steps = 16;
                        for i in 0..=steps {
                            let t = i as f32 / steps as f32;
                            let angle = a_start + (a_end - a_start) * t;
                            pts.push(center + egui::vec2(angle.cos() * outer_r, angle.sin() * outer_r));
                        }
                        for i in 0..=steps {
                            let t = i as f32 / steps as f32;
                            let angle = a_end - (a_end - a_start) * t;
                            pts.push(center + egui::vec2(angle.cos() * inner_r, angle.sin() * inner_r));
                        }
                        pts
                    };

                    let pi = std::f32::consts::PI;
                    let gap = 0.04;

                    // 0. Top Wedge (Emojis)
                    let t_start = -3.0 * pi / 4.0 + gap;
                    let t_end = -pi / 4.0 - gap;
                    let t_color = Color32::from_rgb(251, 191, 36);
                    let is_t_hovered = hovered_sector == Some(0);
                    let t_pts = get_wedge_points(inner_r, outer_r, t_start, t_end);
                    painter.add(egui::Shape::convex_polygon(
                        t_pts,
                        if is_t_hovered { t_color.linear_multiply(0.25) } else { Color32::from_rgba_unmultiplied(15, 23, 42, (185.0 * progress) as u8) },
                        Stroke::new(1.5 * scale, if is_t_hovered { t_color } else { t_color.linear_multiply(0.4) })
                    ));

                    // 1. Right Wedge (Boat)
                    let r_start = -pi / 4.0 + gap;
                    let r_end = pi / 4.0 - gap;
                    let r_color = Color32::from_rgb(42, 130, 201);
                    let is_r_hovered = hovered_sector == Some(1);
                    let r_pts = get_wedge_points(inner_r, outer_r, r_start, r_end);
                    painter.add(egui::Shape::convex_polygon(
                        r_pts,
                        if is_r_hovered { r_color.linear_multiply(0.25) } else { Color32::from_rgba_unmultiplied(15, 23, 42, (185.0 * progress) as u8) },
                        Stroke::new(1.5 * scale, if is_r_hovered { r_color } else { r_color.linear_multiply(0.4) })
                    ));

                    // 2. Bottom Wedge (Alliances)
                    let b_start = pi / 4.0 + gap;
                    let b_end = 3.0 * pi / 4.0 - gap;
                    let b_color = if is_allied { 
                        Color32::from_rgb(239, 68, 68) 
                    } else if has_alliance_request { 
                        Color32::from_rgb(74, 222, 128) 
                    } else if has_proposed_alliance { 
                        Color32::from_rgb(251, 191, 36) 
                    } else { 
                        Color32::from_rgb(74, 222, 128) 
                    };
                    let is_b_hovered = hovered_sector == Some(2);
                    let b_pts = get_wedge_points(inner_r, outer_r, b_start, b_end);
                    painter.add(egui::Shape::convex_polygon(
                        b_pts,
                        if is_b_hovered { b_color.linear_multiply(0.25) } else { Color32::from_rgba_unmultiplied(15, 23, 42, (185.0 * progress) as u8) },
                        Stroke::new(1.5 * scale, if is_b_hovered { b_color } else { b_color.linear_multiply(0.4) })
                    ));

                    // 3. Left Wedge (Build)
                    let l_start = 3.0 * pi / 4.0 + gap;
                    let l_end = 5.0 * pi / 4.0 - gap;
                    let l_color = Color32::from_rgb(34, 211, 238);
                    let is_l_hovered = hovered_sector == Some(3);
                    let l_pts = get_wedge_points(inner_r, outer_r, l_start, l_end);
                    let l_fill = if !is_own_territory {
                        Color32::from_rgba_unmultiplied(30, 30, 30, (80.0 * progress) as u8)
                    } else if is_l_hovered {
                        l_color.linear_multiply(0.25)
                    } else {
                        Color32::from_rgba_unmultiplied(15, 23, 42, (185.0 * progress) as u8)
                    };
                    let l_stroke = Stroke::new(
                        1.5 * scale,
                        if !is_own_territory { l_color.linear_multiply(0.15) } else if is_l_hovered { l_color } else { l_color.linear_multiply(0.4) }
                    );
                    painter.add(egui::Shape::convex_polygon(l_pts, l_fill, l_stroke));

                    // Text & icons for wedges
                    let draw_text = |angle: f32, icon: &str, label: &str| {
                        if scale > 0.05 {
                            let r_i = (inner_r + outer_r) / 2.0 - 5.0 * scale;
                            let r_t = (inner_r + outer_r) / 2.0 + 9.0 * scale;
                            let p_i = center + egui::vec2(angle.cos() * r_i, angle.sin() * r_i);
                            let p_t = center + egui::vec2(angle.cos() * r_t, angle.sin() * r_t);
                            let alpha = (255.0 * progress.clamp(0.0, 1.0)) as u8;

                            painter.text(p_i, egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional((20.0 * scale).max(1.0)), Color32::from_rgba_unmultiplied(255, 255, 255, alpha));
                            painter.text(p_t, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional((8.5 * scale).max(1.0)), Color32::from_rgba_unmultiplied(230, 230, 230, alpha));
                        }
                    };

                    draw_text(-pi / 2.0, "😀", "EMOJIS");
                    draw_text(0.0, "⛵", "BOAT");
                    draw_text(pi / 2.0, "🤝", if is_allied { "BREAK ALLY" } else if has_alliance_request { "ACCEPT ALLY" } else if has_proposed_alliance { "PENDING..." } else { "ALLIANCE" });
                    draw_text(pi, "🔧", if is_own_territory { "BUILD" } else { "LOCKED" });

                    // Center Circle Button
                    let c_color = if hovered_center {
                        if is_spawning {
                            Color32::from_rgb(74, 222, 128).linear_multiply(0.25)
                        } else {
                            Color32::from_rgb(239, 68, 68).linear_multiply(0.25)
                        }
                    } else {
                        Color32::from_rgba_unmultiplied(15, 23, 42, (200.0 * progress) as u8)
                    };
                    let c_stroke_glow = if is_spawning { Color32::from_rgb(74, 222, 128) } else { Color32::from_rgb(239, 68, 68) };
                    let c_stroke = Stroke::new(2.0 * scale, if hovered_center { c_stroke_glow } else { c_stroke_glow.linear_multiply(0.4) });
                    painter.circle(center, r_center, c_color, c_stroke);

                    if scale > 0.05 {
                        let alpha = (255.0 * progress.clamp(0.0, 1.0)) as u8;
                        painter.text(
                            center - egui::vec2(0.0, 5.0 * scale),
                            egui::Align2::CENTER_CENTER,
                            if is_spawning { "★" } else { "⚔" },
                            egui::FontId::proportional((22.0 * scale).max(1.0)),
                            Color32::from_rgba_unmultiplied(255, 255, 255, alpha)
                        );
                        painter.text(
                            center + egui::vec2(0.0, 12.0 * scale),
                            egui::Align2::CENTER_CENTER,
                            if is_spawning { "SPAWN" } else if is_allied { "BREAK & ATTACK" } else { "ATTACK" },
                            egui::FontId::proportional((7.5 * scale).max(1.0)),
                            Color32::from_rgba_unmultiplied(240, 240, 240, alpha)
                        );
                    }

                    // Click Actions (primary/left only — ignore right-clicks that opened the menu)
                    let opened_duration = self.input.context_menu_open_time
                        .map(|t| t.elapsed().as_secs_f32())
                        .unwrap_or(0.0);

                    if is_open_target && scale > 0.3 && opened_duration > 0.1 && ui.input(|i| i.pointer.primary_clicked()) {
                        if let Some(click_pos) = ui.input(|i| i.pointer.press_origin().or(i.pointer.interact_pos())) {
                            let (clicked_center, clicked_sector) = get_zone_at(click_pos);
                            if clicked_center {
                                // Clicked Center (Spawn / Attack)
                                if is_spawning {
                                    self.send_intent(sow_core::protocol::GameplayIntent::Spawn { x: col, y: row });
                                } else {
                                    if is_allied {
                                        self.send_intent(sow_core::protocol::GameplayIntent::BreakAlliance { target_player: owner_id });
                                    }
                                    let troops = self.ui.app.hud_state.troops * (self.ui.app.hud_state.attack_ratio as f64);
                                    if troops > 0.0 {
                                        self.send_intent(sow_core::protocol::GameplayIntent::Attack(sow_core::protocol::AttackIntent {
                                            target_owner: owner_id,
                                            troops: Some(troops),
                                        }));
                                    }
                                }
                                ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                self.input.map_context_menu = None;
                            } else if let Some(sector) = clicked_sector {
                                if sector == 0 {
                                    // Top Wedge (Emojis) - Toggle bottom right panel
                                    self.ui.app.hud_state.show_emoji_panel = !self.ui.app.hud_state.show_emoji_panel;
                                    ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                    self.input.map_context_menu = None;
                                } else if sector == 1 {
                                    // Right Wedge (Boat) - Launch fleet
                                    let troops = Some(self.ui.app.hud_state.troops * (self.ui.app.hud_state.attack_ratio as f64));
                                    self.send_intent(sow_core::protocol::GameplayIntent::LaunchFleet { target_tile: tile_idx, troops });
                                    ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                    self.input.map_context_menu = None;
                                } else if sector == 2 {
                                    // Bottom Wedge (Alliances)
                                    if is_friendly {
                                        if is_allied {
                                            self.send_intent(sow_core::protocol::GameplayIntent::BreakAlliance { target_player: owner_id });
                                        } else if has_alliance_request {
                                            self.send_intent(sow_core::protocol::GameplayIntent::AcceptAlliance { target_player: owner_id });
                                        } else {
                                            self.send_intent(sow_core::protocol::GameplayIntent::ProposeAlliance { target_player: owner_id });
                                        }
                                    }
                                    ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                    self.input.map_context_menu = None;
                                } else if sector == 3 {
                                    // Left Wedge (Build)
                                    if is_own_territory {
                                        ctx.data_mut(|d| d.insert_temp(build_active_id, !radial_build_active));
                                    }
                                }
                            } else {
                                // Clicked outside, close
                                ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                self.input.map_context_menu = None;
                            }
                        }
                    }

                    // Render Build sub-popover directly to the left
                    if radial_build_active && is_own_territory {
                        egui::Area::new(egui::Id::new("radial_build_popover"))
                            .fixed_pos(center - egui::vec2(outer_r + 120.0, 75.0))
                            .order(egui::Order::Tooltip)
                            .show(ctx, |ui| {
                                egui::Frame::menu(&ctx.global_style())
                                    .fill(sow_ui::ui::theme::panel_bg())
                                    .stroke(egui::Stroke::new(1.5_f32, Color32::from_rgb(34, 211, 238)))
                                    .corner_radius(12)
                                    .inner_margin(8)
                                    .show(ui, |ui| {
                                        ui.vertical(|ui| {
                                            ui.label(egui::RichText::new("BUILD").strong().color(Color32::from_rgb(34, 211, 238)).size(11.0));
                                            ui.add_space(4.0);
                                            for &kind in &sow_core::game::BuildingKind::ALL {
                                                let btn = egui::Button::new(egui::RichText::new(kind.as_str()).size(12.0))
                                                    .fill(Color32::TRANSPARENT);
                                                if ui.add(btn).clicked() {
                                                    self.send_intent(sow_core::protocol::GameplayIntent::BuildStructure {
                                                        kind,
                                                        target_tile: tile_idx,
                                                    });
                                                    ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                                    self.input.map_context_menu = None;
                                                }
                                            }
                                        });
                                    });
                            });
                    }
                });

            // Auto-close on Primary left-click outside only — never on Secondary/right-click
            // Also require a grace period so the opening right-click doesn't immediately dismiss
            let opened_duration = self.input.context_menu_open_time
                .map(|t| t.elapsed().as_secs_f32())
                .unwrap_or(0.0);

            if is_open_target && progress > 0.15 && opened_duration > 0.1 && ctx.input(|i| i.pointer.primary_clicked()) {
                if let Some(pos) = ctx.input(|i| i.pointer.press_origin().or(i.pointer.interact_pos())) {
                    if pos.distance(center) > 115.0 * scale.max(0.3) {
                        ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                        self.input.map_context_menu = None;
                    }
                }
            }

            // 3-second inactivity auto-close (except if hovering popovers or menu)
            if self.ui.egui_ctx.egui_wants_pointer_input() {
                self.input.context_menu_timer = 0.0;
            } else {
                self.input.context_menu_timer += ctx.input(|i| i.predicted_dt);
                if self.input.context_menu_timer >= 3.0 {
                    ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                    self.input.map_context_menu = None;
                }
            }
        }
    }
}
