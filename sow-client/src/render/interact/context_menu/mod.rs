mod build_popover;
mod missile_popover;
mod popovers;

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
                    sow_ui::ui::animation::spring_overshoot(t)
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
                            if !sow_ui_kit::widgets::try_paint_emoji(painter, icon, icon_rect, tint) {
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
                        if !sow_ui_kit::widgets::try_paint_emoji(painter, "⚔", icon_rect, tint) {
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
                                            let msg = sow_i18n::get(lang).hud.err_resources_allies_only.clone();
                                            let world_x = (tile_idx % self.sim.map_w) as f32 + 0.5;
                                            let world_y = (tile_idx / self.sim.map_w) as f32 + 0.5;
                                            self.ui.floating_notices.push(crate::app::FloatingNotice {
                                                text: msg,
                                                world_x,
                                                world_y,
                                                start_time: web_time::Instant::now(),
                                                duration: web_time::Duration::from_millis(2000),
                                                color: egui::Color32::from_rgb(248, 113, 113),
                                            });
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
                                        let msg = sow_i18n::get(lang).hud.err_break_alliance_boat.clone();
                                        let world_x = (tile_idx % self.sim.map_w) as f32 + 0.5;
                                        let world_y = (tile_idx / self.sim.map_w) as f32 + 0.5;
                                        self.ui.floating_notices.push(crate::app::FloatingNotice {
                                            text: msg,
                                            world_x,
                                            world_y,
                                            start_time: web_time::Instant::now(),
                                            duration: web_time::Duration::from_millis(2000),
                                            color: egui::Color32::from_rgb(248, 113, 113),
                                        });
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
                                                let msg = sow_i18n::get(lang).hud.err_alliance_renewal_pending.clone();
                                                let world_x = (tile_idx % self.sim.map_w) as f32 + 0.5;
                                                let world_y = (tile_idx / self.sim.map_w) as f32 + 0.5;
                                                self.ui.floating_notices.push(crate::app::FloatingNotice {
                                                    text: msg,
                                                    world_x,
                                                    world_y,
                                                    start_time: web_time::Instant::now(),
                                                    duration: web_time::Duration::from_millis(2000),
                                                    color: egui::Color32::from_rgb(248, 113, 113),
                                                });
                                            } else {
                                                self.send_intent(sow_core::protocol::GameplayIntent::ProposeAlliance { target_player: owner_id });
                                                let lang = self.ui.app.settings_state.language;
                                                let msg = sow_i18n::get(lang).hud.alliance_requested.replace("{}", &target_name);
                                                let world_x = (tile_idx % self.sim.map_w) as f32 + 0.5;
                                                let world_y = (tile_idx / self.sim.map_w) as f32 + 0.5;
                                                self.ui.floating_notices.push(crate::app::FloatingNotice {
                                                    text: msg,
                                                    world_x,
                                                    world_y,
                                                    start_time: web_time::Instant::now(),
                                                    duration: web_time::Duration::from_millis(2000),
                                                    color: egui::Color32::from_rgb(74, 222, 128),
                                                });
                                            }
                                        } else if is_allied {
                                            self.send_intent(sow_core::protocol::GameplayIntent::BreakAlliance { target_player: owner_id });
                                        } else if has_alliance_request {
                                            self.send_intent(sow_core::protocol::GameplayIntent::AcceptAlliance { target_player: owner_id });
                                        } else if has_proposed_alliance {
                                            let lang = self.ui.app.settings_state.language;
                                            let msg = sow_i18n::get(lang).hud.err_alliance_request_pending.clone();
                                            let world_x = (tile_idx % self.sim.map_w) as f32 + 0.5;
                                            let world_y = (tile_idx / self.sim.map_w) as f32 + 0.5;
                                            self.ui.floating_notices.push(crate::app::FloatingNotice {
                                                text: msg,
                                                world_x,
                                                world_y,
                                                start_time: web_time::Instant::now(),
                                                duration: web_time::Duration::from_millis(2000),
                                                color: egui::Color32::from_rgb(248, 113, 113),
                                            });
                                        } else {
                                            self.send_intent(sow_core::protocol::GameplayIntent::ProposeAlliance { target_player: owner_id });
                                            let lang = self.ui.app.settings_state.language;
                                            let msg = sow_i18n::get(lang).hud.alliance_requested.replace("{}", &target_name);
                                            let world_x = (tile_idx % self.sim.map_w) as f32 + 0.5;
                                            let world_y = (tile_idx / self.sim.map_w) as f32 + 0.5;
                                            self.ui.floating_notices.push(crate::app::FloatingNotice {
                                                text: msg,
                                                world_x,
                                                world_y,
                                                start_time: web_time::Instant::now(),
                                                duration: web_time::Duration::from_millis(2000),
                                                color: egui::Color32::from_rgb(74, 222, 128),
                                            });
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

                    self.draw_context_menu_popovers(
                        ui, ctx, tile_idx, col, row, center, scale, compact, screen,
                        outer_r, is_own_territory, radial_build_active,
                        radial_missile_active, build_active_id, missile_active_id,
                    );
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
