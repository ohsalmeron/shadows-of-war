use crate::app::SowApp;
use egui::{Color32, Stroke};

impl SowApp {
    pub(crate) fn draw_context_menu(&mut self, ctx: &egui::Context) {
        if let Some((mx, my, tile_idx)) = self.input.map_context_menu_active {
            let my_id = self.sim.my_player_id.unwrap_or(1);
            let owner_id = self
                .gfx
                .map_renderer
                .as_ref()
                .map(|mr| mr.owners[tile_idx as usize])
                .unwrap_or(0);

            let is_own_territory = owner_id == my_id;
            let has_completed_port = self
                .sim
                .current_snapshot
                .as_ref()
                .map(|s| {
                    s.buildings.iter().any(|b| {
                        b.tile_idx == tile_idx
                            && b.kind == sow_core::game::BuildingKind::City
                            && b.modules.port > 0
                            && !b.under_construction
                    })
                })
                .unwrap_or(false);
            let is_friendly = owner_id != 0 && owner_id != my_id;

            let owner_snapshot = self
                .sim
                .current_snapshot
                .as_ref()
                .and_then(|s| s.players.iter().find(|p| p.id == owner_id));

            let target_name = owner_snapshot
                .map(|p| {
                    if p.name.is_empty() {
                        if p.id >= 200 {
                            format!("Bot {}", p.id)
                        } else {
                            format!("Player {}", p.id)
                        }
                    } else {
                        p.name.clone()
                    }
                })
                .unwrap_or_else(|| format!("Player {}", owner_id));

            let my_snapshot = self
                .sim
                .current_snapshot
                .as_ref()
                .and_then(|s| s.players.iter().find(|p| p.id == my_id));

            let is_betrayer = owner_snapshot
                .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
                .unwrap_or(false);

            let is_teammate = if let Some(owner) = owner_snapshot {
                if let Some(my_snap) = my_snapshot {
                    my_snap.team.is_some() && my_snap.team == owner.team
                } else {
                    false
                }
            } else {
                false
            };

            let is_allied = if let Some(owner) = owner_snapshot {
                (owner.alliances.contains(&my_id) && !is_betrayer) || is_teammate
            } else {
                false
            };

            let mut alliance_timer = 0;
            if is_allied {
                if let Some(my_snap) = my_snapshot {
                    alliance_timer = my_snap
                        .alliance_timers
                        .get(&owner_id)
                        .copied()
                        .unwrap_or(2400);
                }
            }
            let is_in_renewal_window = is_allied && alliance_timer <= 300;

            let has_alliance_request = my_snapshot
                .map(|p| p.alliance_requests.contains(&owner_id))
                .unwrap_or(false);

            let has_proposed_alliance = owner_snapshot
                .map(|p| p.alliance_requests.contains(&my_id))
                .unwrap_or(false);

            let is_spawning = self
                .sim
                .current_snapshot
                .as_ref()
                .map(|s| matches!(s.phase, sow_core::game::GamePhase::Spawning { .. }))
                .unwrap_or(false);

            let col = tile_idx % self.sim.map_w;
            let row = tile_idx / self.sim.map_w;

            // Egui memory key ids for popovers
            let build_active_id = egui::Id::new("radial_build_active");
            let radial_build_active: bool =
                ctx.data(|d| d.get_temp(build_active_id).unwrap_or(false));

            let missile_active_id = egui::Id::new("radial_missile_active");
            let radial_missile_active: bool =
                ctx.data(|d| d.get_temp(missile_active_id).unwrap_or(false));

            // Animation scaling
            let is_open_target = self.input.map_context_menu.is_some();
            let animation_id = egui::Id::new((
                "radial_menu_scale",
                tile_idx,
                self.input.map_context_menu_session,
            ));
            let duration = if is_open_target { 0.22 } else { 0.12 };
            let progress = ctx.animate_bool_with_time(animation_id, is_open_target, duration);

            if progress <= 0.0 && !is_open_target {
                self.input.map_context_menu_active = None;
            }

            // Disney overshoot curve (pop-in and bouncy pop-out)
            let spring_scale = if is_open_target {
                let t = progress;
                if t >= 1.0 {
                    1.0
                } else {
                    1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
                }
            } else {
                let t = 1.0 - progress;
                if t < 0.25 {
                    1.0 + (t * 6.0).sin() * 0.15
                } else {
                    progress * 1.15
                }
            };
            let scale = spring_scale.clamp(0.0, 1.25);
            let screen = ctx.content_rect();
            let compact = screen.width() < 768.0 || screen.width() < screen.height() * 1.25;
            let sf = ctx.pixels_per_point();
            let r_padding = 110.0 * scale;
            let clamped_x = (mx / sf).clamp(r_padding, screen.width() - r_padding);
            let clamped_y = (my / sf).clamp(r_padding, screen.height() - r_padding);
            let center = egui::pos2(clamped_x, clamped_y);
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
                        (false, Some(3)) // Left Build / Missile
                    }
                } else {
                    (false, None)
                }
            };

            let (hovered_center, hovered_sector) =
                pointer_pos.map(get_zone_at).unwrap_or((false, None));

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

                    // Sector hover animations
                    let is_t_hovered = hovered_sector == Some(0);
                    let t_hover_t = ctx.animate_bool_with_time(egui::Id::new(("radial_hover", tile_idx, 0)), is_t_hovered, 0.15);

                    let is_r_hovered = hovered_sector == Some(1) && !is_teammate;
                    let r_hover_t = ctx.animate_bool_with_time(egui::Id::new(("radial_hover", tile_idx, 1)), is_r_hovered, 0.15);

                    let is_b_hovered = hovered_sector == Some(2) && !is_teammate;
                    let b_hover_t = ctx.animate_bool_with_time(egui::Id::new(("radial_hover", tile_idx, 2)), is_b_hovered, 0.15);

                    let is_l_hovered = hovered_sector == Some(3) && !is_teammate;
                    let l_hover_t = ctx.animate_bool_with_time(egui::Id::new(("radial_hover", tile_idx, 3)), is_l_hovered, 0.15);

                    // Dynamic wedge outer radii for hover-burst
                    let t_outer = outer_r + 12.0 * t_hover_t * scale;
                    let r_outer = outer_r + 12.0 * r_hover_t * scale;
                    let b_outer = outer_r + 12.0 * b_hover_t * scale;
                    let l_outer = outer_r + 12.0 * l_hover_t * scale;

                    // 0. Top Wedge (Emojis)
                    let t_start = -3.0 * pi / 4.0 + gap;
                    let t_end = -pi / 4.0 - gap;
                    let t_color = Color32::from_rgb(251, 191, 36);
                    let t_pts = get_wedge_points(inner_r, t_outer, t_start, t_end);
                    let t_fill = if is_t_hovered { t_color.linear_multiply(0.20 + 0.15 * t_hover_t) } else { Color32::from_rgba_unmultiplied(15, 23, 42, (185.0 * progress) as u8) };
                    let t_stroke = Stroke::new((1.5 + 1.0 * t_hover_t) * scale, if is_t_hovered { t_color } else { t_color.linear_multiply(0.4 + 0.3 * t_hover_t) });
                    painter.add(egui::Shape::convex_polygon(t_pts, t_fill, t_stroke));

                    // 1. Right Wedge (Boat)
                    let r_start = -pi / 4.0 + gap;
                    let r_end = pi / 4.0 - gap;
                    let r_color = if is_teammate { Color32::from_rgb(100, 116, 139) } else { Color32::from_rgb(42, 130, 201) };
                    let r_pts = get_wedge_points(inner_r, r_outer, r_start, r_end);
                    let r_fill = if is_r_hovered { r_color.linear_multiply(0.20 + 0.15 * r_hover_t) } else { Color32::from_rgba_unmultiplied(15, 23, 42, (185.0 * progress) as u8) };
                    let r_stroke = Stroke::new((1.5 + 1.0 * r_hover_t) * scale, if is_r_hovered { r_color } else { r_color.linear_multiply(0.4 + 0.3 * r_hover_t) });
                    painter.add(egui::Shape::convex_polygon(r_pts, r_fill, r_stroke));

                    // 2. Bottom Wedge (Alliances)
                    let b_start = pi / 4.0 + gap;
                    let b_end = 3.0 * pi / 4.0 - gap;
                    let b_color = if is_in_renewal_window {
                        if has_alliance_request {
                            Color32::from_rgb(74, 222, 128)
                        } else if has_proposed_alliance {
                            Color32::from_rgb(251, 191, 36)
                        } else {
                            Color32::from_rgb(251, 146, 60)
                        }
                    } else if is_teammate {
                        Color32::from_rgb(100, 116, 139)
                    } else if is_allied {
                        Color32::from_rgb(239, 68, 68)
                    } else if has_alliance_request {
                        Color32::from_rgb(74, 222, 128)
                    } else if has_proposed_alliance {
                        Color32::from_rgb(251, 191, 36)
                    } else {
                        Color32::from_rgb(74, 222, 128)
                    };
                    let b_pts = get_wedge_points(inner_r, b_outer, b_start, b_end);
                    let b_fill = if is_b_hovered { b_color.linear_multiply(0.20 + 0.15 * b_hover_t) } else { Color32::from_rgba_unmultiplied(15, 23, 42, (185.0 * progress) as u8) };
                    let b_stroke = Stroke::new((1.5 + 1.0 * b_hover_t) * scale, if is_b_hovered { b_color } else { b_color.linear_multiply(0.4 + 0.3 * b_hover_t) });
                    painter.add(egui::Shape::convex_polygon(b_pts, b_fill, b_stroke));

                    // 3. Left Wedge (Build / Missile)
                    let l_start = 3.0 * pi / 4.0 + gap;
                    let l_end = 5.0 * pi / 4.0 - gap;
                    let l_color = if is_own_territory {
                        Color32::from_rgb(34, 211, 238) // Cyan for build
                    } else if is_teammate {
                        Color32::from_rgb(100, 116, 139) // Slate gray for teammate
                    } else {
                        Color32::from_rgb(239, 68, 68)  // Red for missile/offensive
                    };
                    let l_pts = get_wedge_points(inner_r, l_outer, l_start, l_end);
                    let l_fill = if is_l_hovered { l_color.linear_multiply(0.20 + 0.15 * l_hover_t) } else { Color32::from_rgba_unmultiplied(15, 23, 42, (185.0 * progress) as u8) };
                    let l_stroke = Stroke::new((1.5 + 1.0 * l_hover_t) * scale, if is_l_hovered { l_color } else { l_color.linear_multiply(0.4 + 0.3 * l_hover_t) });
                    painter.add(egui::Shape::convex_polygon(l_pts, l_fill, l_stroke));

                    let draw_big_icon = |angle: f32, icon: &str, hover_t: f32, disabled: bool| {
                        if scale > 0.05 {
                            let current_outer = outer_r + 12.0 * hover_t * scale;
                            let r_i = (inner_r + current_outer) / 2.0;
                            let p_i = center + egui::vec2(angle.cos() * r_i, angle.sin() * r_i);
                            let alpha = (255.0 * progress.clamp(0.0, 1.0)) as u8;
                            let draw_alpha = if disabled { alpha / 2 } else { alpha };
                            let size_val = (34.0 + 10.0 * hover_t) * scale;
                            let icon_rect =
                                egui::Rect::from_center_size(p_i, egui::vec2(size_val, size_val));
                            let tint = Color32::from_rgba_unmultiplied(255, 255, 255, draw_alpha);
                            if !sow_ui::widgets::try_paint_emoji(painter, icon, icon_rect, tint) {
                                painter.text(
                                    p_i,
                                    egui::Align2::CENTER_CENTER,
                                    icon,
                                    egui::FontId::proportional(size_val),
                                    tint,
                                );
                            }
                        }
                    };

                    let is_top_disabled = !is_friendly;
                    draw_big_icon(-pi / 2.0, "⚖️", t_hover_t, is_top_disabled);
                    draw_big_icon(0.0, "⛵", r_hover_t, is_teammate);
                    draw_big_icon(pi / 2.0, "🤝", b_hover_t, has_proposed_alliance || is_teammate);

                    let left_icon = if is_own_territory {
                        if has_completed_port { "⚓" } else { "🔧" }
                    } else {
                        "🚀"
                    };
                    draw_big_icon(pi, left_icon, l_hover_t, is_teammate);

                    // Center Circle Button
                    let hovered_center_actual = hovered_center && !is_teammate;
                    let c_hover_t = ctx.animate_bool_with_time(egui::Id::new(("radial_hover_center", tile_idx)), hovered_center_actual, 0.15);
                    let c_radius = r_center + 6.0 * c_hover_t * scale;
                    let c_color = if hovered_center_actual {
                        if is_spawning {
                            Color32::from_rgb(74, 222, 128).linear_multiply(0.20 + 0.15 * c_hover_t)
                        } else {
                            Color32::from_rgb(239, 68, 68).linear_multiply(0.20 + 0.15 * c_hover_t)
                        }
                    } else {
                        Color32::from_rgba_unmultiplied(15, 23, 42, (200.0 * progress) as u8)
                    };
                    let c_stroke_glow = if is_spawning { Color32::from_rgb(74, 222, 128) } else { Color32::from_rgb(239, 68, 68) };
                    let c_stroke = Stroke::new((2.0 + 1.0 * c_hover_t) * scale, if hovered_center_actual { c_stroke_glow } else { c_stroke_glow.linear_multiply(0.4 + 0.3 * c_hover_t) });
                    painter.circle(center, c_radius, c_color, c_stroke);

                    if scale > 0.05 {
                        let alpha = (255.0 * progress.clamp(0.0, 1.0)) as u8;
                        let text_size = (24.0 + 8.0 * c_hover_t) * scale;
                        let tint = Color32::from_rgba_unmultiplied(255, 255, 255, if is_teammate { alpha / 2 } else { alpha });
                        let icon_rect = egui::Rect::from_center_size(center, egui::vec2(text_size, text_size));
                        if !sow_ui::widgets::try_paint_emoji(painter, "⚔", icon_rect, tint) {
                            painter.text(
                                center,
                                egui::Align2::CENTER_CENTER,
                                "⚔",
                                egui::FontId::proportional(text_size),
                                tint,
                            );
                        }
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
                                } else if !is_teammate {
                                    let troops = self.ui.app.hud_state.troops * (self.ui.app.hud_state.attack_ratio as f64);
                                    if troops > 0.0 {
                                        let intent = sow_core::protocol::GameplayIntent::Attack(sow_core::protocol::AttackIntent {
                                            target_owner: owner_id,
                                            troops: Some(troops),
                                        });
                                        if is_allied {
                                            self.ui.app.hud_state.show_betrayal_warning = Some((owner_id, intent));
                                        } else {
                                            self.send_intent(intent);
                                        }
                                    }
                                }
                                ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                ctx.data_mut(|d| d.insert_temp(missile_active_id, false));
                                self.input.map_context_menu = None;
                            } else if let Some(sector) = clicked_sector {
                                if sector == 0 {
                                    if is_friendly {
                                        if is_allied {
                                            // Default to Request tab with 10% of ally's resources
                                            let (ally_gold, ally_troops) = self.sim.current_snapshot.as_ref()
                                                .and_then(|s| s.players.iter().find(|p| p.id == owner_id))
                                                .map(|p| (p.gold, p.troops))
                                                .unwrap_or((0.0, 0.0));
                                            self.ui.app.hud_state.show_ask_panel = Some(owner_id);
                                            self.ui.app.hud_state.ask_gold = (ally_gold * 0.10).floor();
                                            self.ui.app.hud_state.ask_troops = (ally_troops * 0.10).floor();
                                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("transfer_active_tab"), 1_usize));
                                        } else {
                                            let lang = self.ui.app.settings_state.language;
                                            self.ui.app.hud_state.show_error = Some(
                                                sow_i18n::get(lang).hud.err_resources_allies_only.clone(),
                                            );
                                        }
                                    }
                                    ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                    ctx.data_mut(|d| d.insert_temp(missile_active_id, false));
                                    self.input.map_context_menu = None;
                                } else if sector == 1 {
                                    // Right Wedge (Boat) - Launch fleet
                                    if is_teammate {
                                        // Disabled for teammates
                                    } else if is_allied {
                                        let lang = self.ui.app.settings_state.language;
                                        self.ui.app.hud_state.show_error = Some(
                                            sow_i18n::get(lang).hud.err_break_alliance_boat.clone(),
                                        );
                                    } else {
                                        let troops = Some(self.ui.app.hud_state.troops * (self.ui.app.hud_state.attack_ratio as f64));
                                        self.send_intent(sow_core::protocol::GameplayIntent::LaunchFleet { target_tile: tile_idx, troops });
                                    }
                                    ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                    ctx.data_mut(|d| d.insert_temp(missile_active_id, false));
                                    self.input.map_context_menu = None;
                                } else if sector == 2 {
                                    // Bottom Wedge (Alliances)
                                    if is_teammate {
                                        // Alliance is permanent, cannot be broken or changed
                                    } else if is_friendly {
                                        if is_in_renewal_window {
                                            if has_alliance_request {
                                                self.send_intent(sow_core::protocol::GameplayIntent::AcceptAlliance { target_player: owner_id });
                                            } else if has_proposed_alliance {
                                                let lang = self.ui.app.settings_state.language;
                                                self.ui.app.hud_state.show_error = Some(
                                                    sow_i18n::get(lang)
                                                        .hud
                                                        .err_alliance_renewal_pending
                                                        .clone(),
                                                );
                                            } else {
                                                self.send_intent(sow_core::protocol::GameplayIntent::ProposeAlliance { target_player: owner_id });
                                                let lang = self.ui.app.settings_state.language;
                                                let msg = sow_i18n::get(lang).hud.alliance_requested.replace("{}", &target_name);
                                                self.ui.app.hud_state.show_info = Some(msg);
                                            }
                                        } else if is_allied {
                                            self.send_intent(sow_core::protocol::GameplayIntent::BreakAlliance { target_player: owner_id });
                                        } else if has_alliance_request {
                                            self.send_intent(sow_core::protocol::GameplayIntent::AcceptAlliance { target_player: owner_id });
                                        } else if has_proposed_alliance {
                                            let lang = self.ui.app.settings_state.language;
                                            self.ui.app.hud_state.show_error = Some(
                                                sow_i18n::get(lang)
                                                    .hud
                                                    .err_alliance_request_pending
                                                    .clone(),
                                            );
                                        } else {
                                            self.send_intent(sow_core::protocol::GameplayIntent::ProposeAlliance { target_player: owner_id });
                                            let lang = self.ui.app.settings_state.language;
                                            let msg = sow_i18n::get(lang).hud.alliance_requested.replace("{}", &target_name);
                                            self.ui.app.hud_state.show_info = Some(msg);
                                        }
                                    }
                                    ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                    ctx.data_mut(|d| d.insert_temp(missile_active_id, false));
                                    self.input.map_context_menu = None;
                                } else if sector == 3 {
                                    // Left Wedge (Build / Missile)
                                    if is_own_territory {
                                        ctx.data_mut(|d| d.insert_temp(build_active_id, !radial_build_active));
                                        ctx.data_mut(|d| d.insert_temp(missile_active_id, false));
                                    } else if !is_teammate {
                                        ctx.data_mut(|d| d.insert_temp(missile_active_id, !radial_missile_active));
                                        ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                    }
                                }
                            } else {
                                // Clicked outside, close
                                ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                ctx.data_mut(|d| d.insert_temp(missile_active_id, false));
                                self.input.map_context_menu = None;
                            }
                        }
                    }

                    // Render Build sub-popover
                    if radial_build_active && is_own_territory {
                        let mut area = egui::Area::new(egui::Id::new("radial_build_popover"))
                            .order(egui::Order::Tooltip);

                        if compact {
                            area = area.fixed_pos(screen.center()).pivot(egui::Align2::CENTER_CENTER);
                        } else {
                            area = area.fixed_pos(center - egui::vec2(outer_r + 240.0, 150.0));
                        }

                        let theme_color = Color32::from_rgb(34, 211, 238); // Cyan

                        area.show(ctx, |ui| {
                            let response_rect = ui.min_rect();
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("build_popover_rect"), response_rect));

                            egui::Frame::window(&ctx.global_style())
                                .fill(sow_ui::ui::theme::panel_bg())
                                .stroke(egui::Stroke::new(1.8_f32, theme_color))
                                .corner_radius(16)
                                .inner_margin(if compact { 16 } else { 12 })
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        // Header
                                        ui.vertical_centered(|ui| {
                                            ui.label(
                                                egui::RichText::new("CONSTRUCT")
                                                    .strong()
                                                    .color(theme_color)
                                                    .size(13.0)
                                            );
                                        });
                                        ui.add_space(8.0);

                                        let card_w = if compact { 280.0 } else { 220.0 };
                                        let card_h = 50.0;
                                        let city_opt = self.sim.current_snapshot.as_ref().and_then(|s| {
                                            s.buildings.iter().find(|b| b.tile_idx == tile_idx && b.kind == sow_core::game::BuildingKind::City)
                                                .map(|b| (b.id, b.modules, b.under_construction))
                                        });

                                        if let Some((city_id, city_modules, city_under_construction)) = city_opt {
                                            ui.vertical_centered(|ui| {
                                                ui.label(
                                                    egui::RichText::new("CITY DISTRICTS")
                                                        .strong()
                                                        .color(theme_color)
                                                        .size(13.0)
                                                );
                                            });
                                            ui.add_space(8.0);

                                            let card_w = if compact { 280.0 } else { 220.0 };
                                            let card_h = 50.0;

                                            // Draw City Module upgrades (Silo/Arsenal, Port, Foundry)
                                            let modules_list = [
                                                (sow_core::building::ModuleKind::Arsenal, "Silo District", "🚀"),
                                                (sow_core::building::ModuleKind::Port, "Port District", "⚓"),
                                                (sow_core::building::ModuleKind::Foundry, "Foundry District", "🏭"),
                                            ];

                                            for &(mod_kind, mod_name, icon) in &modules_list {
                                                let current_lvl = city_modules.get_level(mod_kind);
                                                let cost = sow_core::building::cost::module_upgrade_cost_gold(mod_kind, current_lvl + 1);
                                                let is_disabled = self.ui.app.hud_state.gold < cost;

                                                let label = if current_lvl == 0 {
                                                    format!("Build {}", mod_name)
                                                } else {
                                                    format!("Upgrade {}", mod_name)
                                                };

                                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                                let is_hovered = resp.hovered() && !is_disabled;
                                                let hover_id = ui.make_persistent_id(("popover_hover_mod", mod_name));
                                                let hover_t = ui.ctx().animate_bool_with_time(hover_id, is_hovered, 0.15);

                                                let border_glow = theme_color.linear_multiply(0.3 + 0.7 * hover_t);
                                                let bg_fill = if is_disabled {
                                                    Color32::from_rgba_unmultiplied(20, 20, 20, 180)
                                                } else if is_hovered {
                                                    theme_color.linear_multiply(0.12)
                                                } else {
                                                    Color32::from_rgba_unmultiplied(10, 15, 30, 220)
                                                };

                                                ui.painter().rect(
                                                    rect,
                                                    8.0,
                                                    bg_fill,
                                                    egui::Stroke::new(1.0_f32 + hover_t * 1.0_f32, border_glow),
                                                    egui::StrokeKind::Inside,
                                                );

                                                // Icon
                                                ui.painter().text(
                                                    rect.min + egui::vec2(20.0, card_h / 2.0),
                                                    egui::Align2::CENTER_CENTER,
                                                    icon,
                                                    egui::FontId::proportional((22.0 + 4.0 * hover_t) * scale),
                                                    if is_disabled { Color32::GRAY } else { Color32::WHITE }
                                                );

                                                // Label
                                                ui.painter().text(
                                                    rect.min + egui::vec2(44.0, card_h / 2.0 - 8.0),
                                                    egui::Align2::LEFT_CENTER,
                                                    label,
                                                    egui::FontId::proportional(13.0),
                                                    if is_disabled { Color32::GRAY } else { Color32::WHITE }
                                                );

                                                // Cost & Level info
                                                ui.painter().text(
                                                    rect.min + egui::vec2(44.0, card_h / 2.0 + 8.0),
                                                    egui::Align2::LEFT_CENTER,
                                                    format!("Lvl {} -> {} | {}g", current_lvl, current_lvl + 1, cost as u32),
                                                    egui::FontId::proportional(10.5),
                                                    if is_disabled { Color32::from_rgb(180, 100, 100) } else { Color32::from_rgb(251, 191, 36) }
                                                );

                                                if !is_disabled && resp.clicked() {
                                                    self.send_intent(sow_core::protocol::GameplayIntent::UpgradeCityModule {
                                                        building_id: city_id,
                                                        module: mod_kind,
                                                    });
                                                    ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                                    self.input.map_context_menu = None;
                                                }
                                                ui.add_space(4.0);
                                            }

                                            // If Port module is completed, also show Shipyard options!
                                            if city_modules.port > 0 && !city_under_construction {
                                                ui.add_space(6.0);
                                                ui.separator();
                                                ui.add_space(6.0);

                                                ui.vertical_centered(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("SHIPYARD")
                                                            .strong()
                                                            .color(theme_color)
                                                            .size(13.0)
                                                    );
                                                });
                                                ui.add_space(8.0);

                                                let ships = [
                                                    (sow_core::game::UnitType::Warship, "Warship", 100_000.0, "🚢"),
                                                    (sow_core::game::UnitType::TradeShip, "Trade Ship", 10_000.0, "⛴️"),
                                                ];

                                                for &(kind, label, cost, icon) in &ships {
                                                    let is_disabled = self.ui.app.hud_state.gold < cost;

                                                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                                    let is_hovered = resp.hovered() && !is_disabled;
                                                    let hover_id = ui.make_persistent_id(("popover_hover", label));
                                                    let hover_t = ui.ctx().animate_bool_with_time(hover_id, is_hovered, 0.15);

                                                    let border_glow = theme_color.linear_multiply(0.3 + 0.7 * hover_t);
                                                    let bg_fill = if is_disabled {
                                                        Color32::from_rgba_unmultiplied(20, 20, 20, 180)
                                                    } else if is_hovered {
                                                        theme_color.linear_multiply(0.12)
                                                    } else {
                                                        Color32::from_rgba_unmultiplied(10, 15, 30, 220)
                                                    };

                                                    ui.painter().rect(
                                                        rect,
                                                        8.0,
                                                        bg_fill,
                                                        egui::Stroke::new(1.0_f32 + hover_t * 1.0_f32, border_glow),
                                                        egui::StrokeKind::Inside,
                                                    );

                                                    // Icon
                                                    ui.painter().text(
                                                        rect.min + egui::vec2(20.0, card_h / 2.0),
                                                        egui::Align2::CENTER_CENTER,
                                                        icon,
                                                        egui::FontId::proportional((22.0 + 4.0 * hover_t) * scale),
                                                        if is_disabled { Color32::GRAY } else { Color32::WHITE }
                                                    );

                                                    // Label
                                                    ui.painter().text(
                                                        rect.min + egui::vec2(44.0, card_h / 2.0 - 8.0),
                                                        egui::Align2::LEFT_CENTER,
                                                        label,
                                                        egui::FontId::proportional(13.0),
                                                        if is_disabled { Color32::GRAY } else { Color32::WHITE }
                                                    );

                                                    // Cost
                                                    ui.painter().text(
                                                        rect.min + egui::vec2(44.0, card_h / 2.0 + 8.0),
                                                        egui::Align2::LEFT_CENTER,
                                                        format!("{}g", cost as u32),
                                                        egui::FontId::proportional(10.5),
                                                        if is_disabled { Color32::from_rgb(180, 100, 100) } else { Color32::from_rgb(251, 191, 36) }
                                                    );

                                                    if !is_disabled && resp.clicked() {
                                                        self.send_intent(sow_core::protocol::GameplayIntent::BuildShip {
                                                            port_tile: tile_idx,
                                                            kind,
                                                        });
                                                        ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                                        self.input.map_context_menu = None;
                                                    }
                                                    ui.add_space(4.0);
                                                }
                                            }
                                        } else {
                                            let current_level = if (tile_idx as usize) < self.sim.tile_upgrades.len() {
                                                self.sim.tile_upgrades[tile_idx as usize]
                                            } else {
                                                0
                                            };

                                            let tile_byte = self.gfx.map_renderer.as_ref()
                                                .and_then(|mr| mr.terrain.get(tile_idx as usize).copied())
                                                .unwrap_or(0b10000000);
                                            let map_tile = sow_core::map::MapTile::from_byte(tile_byte);

                                            // Procedural resource extraction identical to map.rs
                                            let magnitude = map_tile.magnitude();
                                            let seed = (col as u64).wrapping_mul(374761393)
                                                .wrapping_add((row as u64).wrapping_mul(668265263))
                                                .wrapping_add(magnitude as u64);
                                            let hash = (seed ^ (seed >> 13)).wrapping_mul(1274126177) % 100;

                                            let resource = if !map_tile.is_land() {
                                                sow_core::map::TileResource::None
                                            } else if magnitude >= 20 {
                                                match hash % 5 {
                                                    0 => sow_core::map::TileResource::Copper,
                                                    1 => sow_core::map::TileResource::Stone,
                                                    2 => sow_core::map::TileResource::Iron,
                                                    3 => sow_core::map::TileResource::Diamonds,
                                                    _ => sow_core::map::TileResource::None,
                                                }
                                            } else if magnitude >= 10 {
                                                match hash % 8 {
                                                    0 => sow_core::map::TileResource::Wheat,
                                                    1 => sow_core::map::TileResource::Stone,
                                                    2 => sow_core::map::TileResource::Copper,
                                                    3 => sow_core::map::TileResource::Iron,
                                                    4 => sow_core::map::TileResource::Jade,
                                                    _ => sow_core::map::TileResource::None,
                                                }
                                            } else {
                                                match hash % 10 {
                                                    0 => sow_core::map::TileResource::Corn,
                                                    1 => sow_core::map::TileResource::Rice,
                                                    2 => sow_core::map::TileResource::Wheat,
                                                    3 => sow_core::map::TileResource::Jade,
                                                    4 => sow_core::map::TileResource::Salt,
                                                    _ => sow_core::map::TileResource::None,
                                                }
                                            };

                                            let (upgrade_label, upgrade_icon) = match resource {
                                                sow_core::map::TileResource::Corn => ("Upgrade Farm (Corn)", "🌽"),
                                                sow_core::map::TileResource::Rice => ("Upgrade Farm (Rice)", "🌾"),
                                                sow_core::map::TileResource::Wheat => ("Upgrade Farm (Wheat)", "🍞"),
                                                sow_core::map::TileResource::Copper => ("Upgrade Mine (Copper)", "🪙"),
                                                sow_core::map::TileResource::Stone => ("Upgrade Quarry (Stone)", "🪨"),
                                                sow_core::map::TileResource::Iron => ("Upgrade Mine (Iron)", "⛓️"),
                                                sow_core::map::TileResource::Jade => ("Upgrade Jade Opp.", "🟢"),
                                                sow_core::map::TileResource::Diamonds => ("Upgrade Diamond Opp.", "💎"),
                                                sow_core::map::TileResource::Salt => ("Upgrade Salt Opp.", "🧂"),
                                                sow_core::map::TileResource::None => {
                                                    match map_tile.terrain_type() {
                                                        sow_core::map::TerrainType::Land => ("Upgrade Flatland Farm", "🌾"),
                                                        sow_core::map::TerrainType::Highland | sow_core::map::TerrainType::Mountain => ("Upgrade Highland Mine", "🪨"),
                                                        sow_core::map::TerrainType::Water | sow_core::map::TerrainType::Lake => ("Upgrade Water Opp.", "🐠"),
                                                    }
                                                }
                                            };

                                            let s = sow_core::config::GOLD_SCALE.max(1.0);
                                            let upgrade_cost = (1000.0 * 1.5f64.powi(current_level as i32)) / s;

                                            // Render Upgrade Card
                                            let is_upgrade_disabled = self.ui.app.hud_state.gold < upgrade_cost;
                                            let (upgrade_rect, upgrade_resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                            let is_upgrade_hovered = upgrade_resp.hovered() && !is_upgrade_disabled;
                                            let upgrade_hover_id = ui.make_persistent_id(("popover_hover", "upgrade_tile"));
                                            let upgrade_hover_t = ui.ctx().animate_bool_with_time(upgrade_hover_id, is_upgrade_hovered, 0.15);

                                            let upgrade_border_glow = theme_color.linear_multiply(0.3 + 0.7 * upgrade_hover_t);
                                            let upgrade_bg_fill = if is_upgrade_disabled {
                                                Color32::from_rgba_unmultiplied(20, 20, 20, 180)
                                            } else if is_upgrade_hovered {
                                                theme_color.linear_multiply(0.12)
                                            } else {
                                                Color32::from_rgba_unmultiplied(10, 15, 30, 220)
                                            };

                                            ui.painter().rect(
                                                upgrade_rect,
                                                8.0,
                                                upgrade_bg_fill,
                                                egui::Stroke::new(1.0_f32 + upgrade_hover_t * 1.0_f32, upgrade_border_glow),
                                                egui::StrokeKind::Inside,
                                            );

                                            ui.painter().text(
                                                upgrade_rect.min + egui::vec2(20.0, card_h / 2.0),
                                                egui::Align2::CENTER_CENTER,
                                                upgrade_icon,
                                                egui::FontId::proportional((22.0 + 4.0 * upgrade_hover_t) * scale),
                                                if is_upgrade_disabled { Color32::GRAY } else { Color32::WHITE }
                                            );

                                            ui.painter().text(
                                                upgrade_rect.min + egui::vec2(44.0, card_h / 2.0 - 8.0),
                                                egui::Align2::LEFT_CENTER,
                                                upgrade_label,
                                                egui::FontId::proportional(13.0),
                                                if is_upgrade_disabled { Color32::GRAY } else { Color32::WHITE }
                                            );

                                            ui.painter().text(
                                                upgrade_rect.min + egui::vec2(44.0, card_h / 2.0 + 8.0),
                                                egui::Align2::LEFT_CENTER,
                                                format!("Lvl {} -> {} | {}g", current_level, current_level + 1, upgrade_cost as u32),
                                                egui::FontId::proportional(10.5),
                                                if is_upgrade_disabled { Color32::from_rgb(180, 100, 100) } else { Color32::from_rgb(251, 191, 36) }
                                            );

                                            if !is_upgrade_disabled && upgrade_resp.clicked() {
                                                self.send_intent(sow_core::protocol::GameplayIntent::UpgradeTile { tile_idx });
                                                ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                                self.input.map_context_menu = None;
                                            }

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.add_space(4.0);

                                            let buildings_list = [
                                                (
                                                    sow_core::game::BuildingKind::City,
                                                    "City Center",
                                                    "Core of your empire. Increases troop generation, gold generation, and max troops. Can be upgraded with 6 powerful modules (Port, Foundry, Armory, Intel, Arsenal, Shield)!",
                                                ),
                                                (
                                                    sow_core::game::BuildingKind::Factory,
                                                    "Industrial Factory",
                                                    "Economic Engine: A specialized pure gold generator. Upgradable up to Level 5 to progressively multiply gold income. Must be spaced from other structures.",
                                                ),
                                                (
                                                    sow_core::game::BuildingKind::Port,
                                                    "Maritime Port",
                                                    "Maritime Port: Specialized coastal harbor. Generates gold and troop income and enables launching naval fleets. Must be built near the shore.",
                                                ),
                                                (
                                                    sow_core::game::BuildingKind::Bunker,
                                                    "Defense Tower",
                                                    "Frontline Anchor: Fortifies borders, slowing enemy land grabs. Naturally strong on mountains (3x) and highlands (2x), upgradable with gold!",
                                                ),
                                            ];

                                            for &(kind, label, desc) in &buildings_list {
                                                let my_player_id = self.sim.my_player_id.unwrap_or(1);
                                                let count = self.sim.current_snapshot.as_ref()
                                                    .map(|s| s.buildings.iter().filter(|b| b.owner_id == my_player_id && b.kind == kind).count() as u32)
                                                    .unwrap_or(0);
                                                let cost = sow_core::building::structure_build_cost_gold(kind, count, &self.sim.config);
                                                let is_disabled = self.ui.app.hud_state.gold < cost;

                                                let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                                let is_hovered = resp.hovered() && !is_disabled;
                                                let hover_id = ui.make_persistent_id(("popover_hover", label));
                                                let hover_t = ui.ctx().animate_bool_with_time(hover_id, is_hovered, 0.15);

                                                let border_glow = theme_color.linear_multiply(0.3 + 0.7 * hover_t);
                                                let bg_fill = if is_disabled {
                                                    Color32::from_rgba_unmultiplied(20, 20, 20, 180)
                                                } else if is_hovered {
                                                    theme_color.linear_multiply(0.12)
                                                } else {
                                                    Color32::from_rgba_unmultiplied(10, 15, 30, 220)
                                                };

                                                ui.painter().rect(
                                                    rect,
                                                    8.0,
                                                    bg_fill,
                                                    egui::Stroke::new(1.0_f32 + hover_t * 1.0_f32, border_glow),
                                                    egui::StrokeKind::Inside,
                                                );

                                                // Icon (Premium building image asset)
                                                let icon_size = 24.0 * scale;
                                                let icon_rect = egui::Rect::from_center_size(
                                                    rect.min + egui::vec2(20.0, card_h / 2.0),
                                                    egui::vec2(icon_size, icon_size),
                                                );
                                                let tint = if is_disabled { Color32::GRAY } else { Color32::WHITE };
                                                let emoji = match kind {
                                                    sow_core::game::BuildingKind::City => "🏛️",
                                                    sow_core::game::BuildingKind::Factory => "🏭",
                                                    sow_core::game::BuildingKind::Port => "⚓",
                                                    sow_core::game::BuildingKind::Bunker => "🛡️",
                                                };
                                                if !sow_ui::widgets::try_paint_emoji(ui.painter(), emoji, icon_rect, tint) {
                                                    ui.painter().text(
                                                        icon_rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        emoji,
                                                        egui::FontId::proportional(icon_size * 0.7),
                                                        tint,
                                                    );
                                                }

                                                // Label
                                                ui.painter().text(
                                                    rect.min + egui::vec2(44.0, card_h / 2.0 - 8.0),
                                                    egui::Align2::LEFT_CENTER,
                                                    label,
                                                    egui::FontId::proportional(13.0),
                                                    if is_disabled { Color32::GRAY } else { Color32::WHITE }
                                                );

                                                // Cost
                                                let cost_text = if cost.is_infinite() { "N/A".to_string() } else { format!("{}", cost as u32) };
                                                let cost_color = if is_disabled {
                                                    Color32::from_rgb(180, 100, 100)
                                                } else {
                                                    Color32::from_rgb(251, 191, 36)
                                                };
                                                let cost_label = if cost_text == "N/A" {
                                                    cost_text.clone()
                                                } else {
                                                    format!("🪙 {cost_text}")
                                                };
                                                sow_ui::widgets::paint_emoji_text_at(
                                                    ui.painter(),
                                                    rect.min + egui::vec2(44.0, card_h / 2.0 + 8.0),
                                                    egui::Align2::LEFT_CENTER,
                                                    &cost_label,
                                                    egui::FontId::proportional(10.5),
                                                    cost_color,
                                                    false,
                                                );

                                                resp = resp.on_hover_ui(|ui| {
                                                    ui.label(egui::RichText::new(label).strong().size(14.0).color(theme_color));
                                                    ui.add_space(4.0);
                                                    ui.label(egui::RichText::new(desc).size(12.0).color(egui::Color32::LIGHT_GRAY));
                                                    ui.add_space(6.0);
                                                    let cost_color = if !is_disabled { egui::Color32::from_rgb(74, 222, 128) } else { egui::Color32::from_rgb(239, 68, 68) };
                                                    sow_ui::widgets::emoji_label(
                                                        ui,
                                                        &format!("Cost: 🪙 {cost_text} Gold"),
                                                        egui::FontId::proportional(13.0),
                                                        cost_color,
                                                    );
                                                });

                                                if !is_disabled && resp.clicked() {
                                                    self.send_intent(sow_core::protocol::GameplayIntent::BuildStructure {
                                                        kind,
                                                        target_tile: tile_idx,
                                                    });
                                                    ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                                    self.input.map_context_menu = None;
                                                }
                                                ui.add_space(4.0);
                                            }
                                        }
                                    });
                                });
                        });
                    }

                    // Render Missile sub-popover
                    if radial_missile_active && !is_own_territory {
                        let mut area = egui::Area::new(egui::Id::new("radial_missile_popover"))
                            .order(egui::Order::Tooltip);

                        if compact {
                            area = area.fixed_pos(screen.center()).pivot(egui::Align2::CENTER_CENTER);
                        } else {
                            area = area.fixed_pos(center - egui::vec2(outer_r + 240.0, 100.0));
                        }

                        let theme_color = Color32::from_rgb(239, 68, 68); // Red

                        area.show(ctx, |ui| {
                            let response_rect = ui.min_rect();
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("missile_popover_rect"), response_rect));

                            egui::Frame::window(&ctx.global_style())
                                .fill(sow_ui::ui::theme::panel_bg())
                                .stroke(egui::Stroke::new(1.8_f32, theme_color))
                                .corner_radius(16)
                                .inner_margin(if compact { 16 } else { 12 })
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        ui.vertical_centered(|ui| {
                                            ui.label(
                                                egui::RichText::new("NUCLEAR STRIKE")
                                                    .strong()
                                                    .color(theme_color)
                                                    .size(13.0)
                                            );
                                        });
                                        ui.add_space(8.0);

                                        let card_w = if compact { 280.0 } else { 220.0 };
                                        let card_h = 50.0;

                                        let nukes = [
                                            (sow_core::game::NukeKind::AtomBomb, "Missile Strike", "☢️"),
                                        ];

                                        for &(kind, label, icon) in &nukes {
                                            let cost = kind.gold_cost(0);
                                            let is_disabled = self.ui.app.hud_state.gold < cost;

                                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                            let is_hovered = resp.hovered() && !is_disabled;
                                            let hover_id = ui.make_persistent_id(("popover_hover", label));
                                            let hover_t = ui.ctx().animate_bool_with_time(hover_id, is_hovered, 0.15);

                                            let border_glow = theme_color.linear_multiply(0.3 + 0.7 * hover_t);
                                            let bg_fill = if is_disabled {
                                                Color32::from_rgba_unmultiplied(20, 20, 20, 180)
                                            } else if is_hovered {
                                                theme_color.linear_multiply(0.12)
                                            } else {
                                                Color32::from_rgba_unmultiplied(10, 15, 30, 220)
                                            };

                                            ui.painter().rect(
                                                rect,
                                                8.0,
                                                bg_fill,
                                                egui::Stroke::new(1.0_f32 + hover_t * 1.0_f32, border_glow),
                                                egui::StrokeKind::Inside,
                                            );

                                            // Icon
                                            ui.painter().text(
                                                rect.min + egui::vec2(20.0, card_h / 2.0),
                                                egui::Align2::CENTER_CENTER,
                                                icon,
                                                egui::FontId::proportional((22.0 + 4.0 * hover_t) * scale),
                                                if is_disabled { Color32::GRAY } else { Color32::WHITE }
                                            );

                                            // Label
                                            ui.painter().text(
                                                rect.min + egui::vec2(44.0, card_h / 2.0 - 8.0),
                                                egui::Align2::LEFT_CENTER,
                                                label,
                                                egui::FontId::proportional(13.0),
                                                if is_disabled { Color32::GRAY } else { Color32::WHITE }
                                            );

                                            // Cost
                                            ui.painter().text(
                                                rect.min + egui::vec2(44.0, card_h / 2.0 + 8.0),
                                                egui::Align2::LEFT_CENTER,
                                                format!("{}g", cost as u32),
                                                egui::FontId::proportional(10.5),
                                                if is_disabled { Color32::from_rgb(180, 100, 100) } else { Color32::from_rgb(251, 191, 36) }
                                            );

                                            if !is_disabled && resp.clicked() {
                                                self.send_intent(sow_core::protocol::GameplayIntent::LaunchNuke {
                                                    kind,
                                                    target_tile: tile_idx,
                                                });
                                                ctx.data_mut(|d| d.insert_temp(missile_active_id, false));
                                                self.input.map_context_menu = None;
                                            }
                                            ui.add_space(4.0);
                                        }
                                    });
                                });
                        });
                    }
                });

            // Responsive modal backdrop dimmer
            if compact
                && ((radial_build_active && is_own_territory)
                    || (radial_missile_active && !is_own_territory))
            {
                ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Background,
                    egui::Id::new("radial_sub_dim_bg"),
                ))
                .rect_filled(screen, 0.0, Color32::from_black_alpha(150));
            }

            // Auto-close on Primary left-click outside only — never on Secondary/right-click
            // Also require a grace period so the opening right-click doesn't immediately dismiss
            let opened_duration = self
                .input
                .context_menu_open_time
                .map(|t| t.elapsed().as_secs_f32())
                .unwrap_or(0.0);

            if is_open_target
                && progress > 0.15
                && opened_duration > 0.1
                && ctx.input(|i| i.pointer.primary_clicked())
            {
                if let Some(pos) =
                    ctx.input(|i| i.pointer.press_origin().or(i.pointer.interact_pos()))
                {
                    let mut click_absorbed = false;
                    if radial_build_active && is_own_territory {
                        if let Some(r) = ctx
                            .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("build_popover_rect")))
                        {
                            if r.contains(pos) {
                                click_absorbed = true;
                            }
                        }
                    }
                    if radial_missile_active && !is_own_territory {
                        if let Some(r) = ctx.data(|d| {
                            d.get_temp::<egui::Rect>(egui::Id::new("missile_popover_rect"))
                        }) {
                            if r.contains(pos) {
                                click_absorbed = true;
                            }
                        }
                    }

                    if !click_absorbed && pos.distance(center) > 115.0 * scale.max(0.3) {
                        ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                        ctx.data_mut(|d| d.insert_temp(missile_active_id, false));
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
                    ctx.data_mut(|d| d.insert_temp(missile_active_id, false));
                    self.input.map_context_menu = None;
                }
            }
        }
    }
}
