use super::rows::*;
use crate::app::SowApp;
use egui::{Align2, Color32, RichText, Stroke, Vec2};
use sow_core::protocol::Team;
use sow_ui_game::{LeaderboardRanking, LeaderboardRowDisplay, TeamRanking, INITIAL_VISIBLE_LIMIT};
use std::collections::HashSet;

impl SowApp {
    fn refresh_leaderboard_cache(&mut self) {
        let Some(snap) = &self.sim.current_snapshot else {
            return;
        };

        let team_mode = is_team_mode(self);
        let mut rankings = Vec::new();
        let mut team_tiles: std::collections::HashMap<Team, (u32, u32)> =
            std::collections::HashMap::new();

        for p in &snap.players {
            if !p.alive {
                continue;
            }
            let name = snapshot_display_name(p);
            rankings.push(LeaderboardRanking {
                id: p.id,
                tiles: p.tile_count,
                troops: p.troops,
                name,
                kills: p.kills,
                deaths: p.deaths,
                assists: p.assists,
            });

            if team_mode {
                if let Some(team) = p.team {
                    let entry = team_tiles.entry(team).or_insert((0, 0));
                    entry.0 += p.tile_count;
                    entry.1 += 1;
                }
            }
        }
        rankings.sort_unstable_by_key(|r| std::cmp::Reverse(r.tiles));
        self.ui.leaderboard_rankings = rankings;

        if team_mode {
            let mut team_rankings: Vec<TeamRanking> = team_tiles
                .into_iter()
                .map(|(team, (tiles, member_count))| TeamRanking {
                    team,
                    tiles,
                    member_count,
                    color: team_color(team),
                })
                .collect();
            team_rankings.sort_unstable_by_key(|t| std::cmp::Reverse(t.tiles));
            self.ui.leaderboard_team_rankings = team_rankings;
        } else {
            self.ui.leaderboard_team_rankings.clear();
        }

        let my_id = self.sim.my_player_id;
        let search_active = !self.ui.leaderboard_search.is_empty();
        let filtered =
            filtered_rankings(&self.ui.leaderboard_rankings, &self.ui.leaderboard_search);

        let render_count = if search_active {
            filtered.len()
        } else {
            filtered.len().min(self.ui.leaderboard_visible_limit)
        };

        let mut hydrate_ids: HashSet<u16> = HashSet::new();
        for (_, r) in filtered.iter().take(render_count) {
            hydrate_ids.insert(r.id);
        }
        for (_, r) in filtered.iter().take(3) {
            hydrate_ids.insert(r.id);
        }
        if let Some(my_id) = my_id {
            hydrate_ids.insert(my_id);
        }

        for id in hydrate_ids {
            let Some(p) = snap.players.iter().find(|p| p.id == id) else {
                continue;
            };
            self.ui.leaderboard_display.insert(
                id,
                LeaderboardRowDisplay {
                    name: snapshot_display_name(p),
                    player_type: p.player_type,
                    leader: p.leader,
                    color: p.color,
                    active_emoji: p.active_emoji.clone(),
                    team: p.team,
                },
            );
        }
    }

    fn sync_leaderboard_ui_state(&mut self) {
        if self.ui.leaderboard_search != self.ui.leaderboard_prev_search {
            self.ui.leaderboard_visible_limit = INITIAL_VISIBLE_LIMIT;
            self.ui.leaderboard_paged_through_limit = 0;
            self.ui.leaderboard_prev_search = self.ui.leaderboard_search.clone();
        }

        let is_open = self.ui.show_leaderboard;
        if is_open && !self.ui.leaderboard_was_open {
            self.ui.leaderboard_visible_limit = INITIAL_VISIBLE_LIMIT;
            self.ui.leaderboard_paged_through_limit = 0;
        }
        self.ui.leaderboard_was_open = is_open;
    }

    #[allow(clippy::too_many_arguments)]
    fn render_leaderboard_body(
        &mut self,
        ui: &mut egui::Ui,
        metrics: &LeaderboardMetrics,
        total_land_tiles: u32,
        my_id: Option<u16>,
        my_team: Option<Team>,
        team_mode: bool,
        search_active: bool,
        filtered: &[(usize, LeaderboardRanking)],
        scroll_row_count: usize,
        show_sticky_self: bool,
        team_rankings: &[TeamRanking],
        win_pct: f32,
    ) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = if metrics.is_mobile { 10.0 } else { 8.0 };

            egui::Frame::new()
                .fill(sow_ui_kit::theme::nickname_field_bg())
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    sow_ui_kit::theme::accent_ranked_gold(),
                ))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::symmetric(
                    if metrics.is_mobile { 14 } else { 12 },
                    if metrics.is_mobile { 8 } else { 6 },
                ))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "👑 Domination Victory: Control {:.0}% of Map",
                                win_pct * 100.0
                            ))
                            .color(sow_ui_kit::theme::accent_ranked_gold())
                            .size(if metrics.is_mobile { 14.0 } else { 13.0 })
                            .strong(),
                        );
                    });
                });

            ui.add_space(4.0);

            egui::Frame::new()
                .fill(sow_ui_kit::theme::leaderboard_search_field_bg())
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    sow_ui_kit::theme::leaderboard_search_field_border(),
                ))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(
                    8,
                    if metrics.is_mobile { 6 } else { 4 },
                ))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.set_width(ui.available_width());
                        let (search_icon, _) =
                            ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::hover());
                        sow_ui_kit::widgets::try_paint_emoji(
                            ui.painter(),
                            "🔍",
                            search_icon,
                            Color32::LIGHT_GRAY,
                        );
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.ui.leaderboard_search)
                                .hint_text("Search players…")
                                .desired_width(ui.available_width() - 32.0),
                        );
                        if !self.ui.leaderboard_search.is_empty() && ui.small_button("✕").clicked()
                        {
                            self.ui.leaderboard_search.clear();
                        }
                        if response.changed() {
                            self.sync_leaderboard_ui_state();
                        }
                    });
                });

            if team_mode && !team_rankings.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("TEAMS")
                        .size(10.0)
                        .color(Color32::from_gray(120))
                        .strong(),
                );
                for team in team_rankings {
                    let control_pct = (team.tiles as f32 / total_land_tiles as f32) * 100.0;
                    let is_my_team = my_team == Some(team.team);
                    paint_team_row(ui, team, control_pct, is_my_team, metrics);
                    ui.add_space(2.0);
                }
                ui.separator();
            }

            let safe_bottom = ui.ctx().input(|i| i.safe_area_insets().0.bottom);
            let mobile_back_reserve = if metrics.is_mobile {
                52.0 + 8.0 + safe_bottom
            } else {
                0.0
            };
            let sticky_reserve = if show_sticky_self {
                metrics.row_height + ui.spacing().item_spacing.y + 1.0
            } else {
                0.0
            };
            let table_scroll_h =
                (ui.available_height() - TABLE_HEADER_H - sticky_reserve - mobile_back_reserve)
                    .max(120.0);

            let visible_rows: Vec<(usize, LeaderboardRanking)> = filtered
                .iter()
                .take(scroll_row_count)
                .map(|(idx, r)| (*idx, r.clone()))
                .collect();
            let row_ctx = RowPaintCtx {
                app: self,
                total_land_tiles,
                my_id,
            };

            paint_leaderboard_header(ui, metrics);

            let scroll_output = egui::ScrollArea::vertical()
                .id_salt("leaderboard_players")
                .max_height(table_scroll_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (i, (rank_idx, ranking)) in visible_rows.iter().enumerate() {
                        paint_leaderboard_player_row(
                            ui,
                            &row_ctx,
                            metrics,
                            *rank_idx,
                            ranking,
                            false,
                            i % 2 == 1,
                        );
                    }
                });

            if !search_active {
                let state = &scroll_output.state;
                let viewport_h = scroll_output.inner_rect.height();
                let content_h = scroll_output.content_size.y;
                let near_bottom = state.offset.y + viewport_h >= content_h - SCROLL_NEAR_BOTTOM;
                let visible_limit = self.ui.leaderboard_visible_limit;
                if near_bottom && visible_limit < filtered.len() {
                    if self.ui.leaderboard_paged_through_limit != visible_limit {
                        self.ui.leaderboard_visible_limit =
                            (visible_limit + SCROLL_LOAD_STEP).min(filtered.len());
                        self.ui.leaderboard_paged_through_limit = self.ui.leaderboard_visible_limit;
                    }
                } else if !near_bottom {
                    self.ui.leaderboard_paged_through_limit = 0;
                }
            }

            if show_sticky_self {
                if let Some(my_id) = my_id {
                    ui.separator();
                    if let Some((rank_idx, ranking)) = filtered.iter().find(|(_, r)| r.id == my_id)
                    {
                        let sticky_ctx = RowPaintCtx {
                            app: self,
                            total_land_tiles,
                            my_id: Some(my_id),
                        };
                        paint_leaderboard_player_row(
                            ui,
                            &sticky_ctx,
                            metrics,
                            *rank_idx,
                            ranking,
                            true,
                            false,
                        );
                    }
                }
            }

            if metrics.is_mobile {
                ui.add_space(8.0);
                let safe_bottom = ui.ctx().input(|i| i.safe_area_insets().0.bottom);
                ui.allocate_space(Vec2::new(0.0, safe_bottom));
                let back_h = 52.0;
                let back = ui.add(
                    egui::Button::new(RichText::new("← Back").size(18.0).strong())
                        .min_size(Vec2::new(ui.available_width(), back_h))
                        .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 12))
                        .stroke(Stroke::new(
                            1.0_f32,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 30),
                        )),
                );
                if back.clicked() {
                    self.ui.show_leaderboard = false;
                }
            }
        });
    }

    pub fn render_leaderboard(&mut self, ctx: &egui::Context) {
        self.sync_leaderboard_ui_state();

        self.ui.leaderboard_timer -= self.ui.raw_input.predicted_dt;
        if self.ui.leaderboard_timer <= 0.0 {
            self.ui.leaderboard_timer = REFRESH_INTERVAL_SECS;
            self.refresh_leaderboard_cache();
        }

        let total_land_tiles = self
            .sim
            .current_snapshot
            .as_ref()
            .map(|s| s.total_land_tiles)
            .unwrap_or(1)
            .max(1);

        let my_id = self.sim.my_player_id;
        let my_team =
            my_id.and_then(|id| self.ui.leaderboard_display.get(&id).and_then(|d| d.team));

        let team_mode = is_team_mode(self);
        let search_active = !self.ui.leaderboard_search.is_empty();
        let filtered: Vec<(usize, LeaderboardRanking)> =
            filtered_rankings(&self.ui.leaderboard_rankings, &self.ui.leaderboard_search)
                .into_iter()
                .map(|(idx, r)| (idx, (*r).clone()))
                .collect();

        let scroll_row_count = if search_active {
            filtered.len()
        } else {
            filtered.len().min(self.ui.leaderboard_visible_limit)
        };

        let scroll_ids: HashSet<u16> = filtered
            .iter()
            .take(scroll_row_count)
            .map(|(_, r)| r.id)
            .collect();

        let show_sticky_self = my_id.is_some_and(|id| {
            self.ui.leaderboard_rankings.iter().any(|r| r.id == id) && !scroll_ids.contains(&id)
        });

        let team_rankings = self.ui.leaderboard_team_rankings.clone();
        let win_pct = self
            .sim
            .engine
            .as_ref()
            .map(|e| e.state.config.map_control_win_percentage)
            .unwrap_or(0.60);

        let metrics = LeaderboardMetrics::from_ctx(ctx);

        // Render leaderboard pop-up modal overlay
        let mut show_leaderboard = self.ui.show_leaderboard;
        sow_ui_kit::theme::draw_standard_modal(
            ctx,
            &mut show_leaderboard,
            "leaderboard",
            "Leaderboard",
            "CLOSE",
            self.ui.app.settings_state.reduced_motion,
            |ui| {
                self.render_leaderboard_body(
                    ui,
                    &metrics,
                    total_land_tiles,
                    my_id,
                    my_team,
                    team_mode,
                    search_active,
                    &filtered,
                    scroll_row_count,
                    show_sticky_self,
                    &team_rankings,
                    win_pct,
                );
            },
        );
        self.ui.show_leaderboard = show_leaderboard;

        // Single toggle area — always on top, always clickable (trophy toggles open/closed).
        egui::Area::new(egui::Id::new("leaderboard_area"))
            .order(egui::Order::Foreground)
            .anchor(Align2::LEFT_TOP, Vec2::new(12.0, 12.0))
            .show(ctx, |ui| {
                sow_ui_kit::theme::hud_panel_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .add(sow_ui_kit::widgets::HudEmojiButton::new("🏆"))
                            .on_hover_text("Leaderboard")
                            .clicked()
                        {
                            self.ui.show_leaderboard = !self.ui.show_leaderboard;
                            if self.ui.show_leaderboard {
                                self.ui.show_dev_sidebar = false;
                            }
                        }

                        #[cfg(any(feature = "dev", debug_assertions))]
                        if ui
                            .add(sow_ui_kit::widgets::HudEmojiButton::new("🛠"))
                            .on_hover_text("Dev Utils")
                            .clicked()
                        {
                            self.ui.show_dev_sidebar = !self.ui.show_dev_sidebar;
                            if self.ui.show_dev_sidebar {
                                self.ui.show_leaderboard = false;
                            }
                        }
                    });
                });

                #[cfg(any(feature = "dev", debug_assertions))]
                if self.ui.show_dev_sidebar {
                    ui.add_space(8.0);
                    sow_ui_kit::theme::hud_panel_frame().show(ui, |ui| {
                        self.render_dev_sidebar(ctx, ui);
                    });
                }
            });
    }

    #[cfg(any(feature = "dev", debug_assertions))]
    fn render_dev_sidebar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.style_mut().spacing.slider_width = 100.0;
            ui.style_mut().spacing.item_spacing = Vec2::new(4.0, 4.0);

            ui.horizontal(|ui| {
                ui.label(RichText::new("🛠 Dev Tools").strong().color(Color32::WHITE));
            });
            ui.add_space(2.0);
            ui.separator();
            ui.add_space(2.0);

            let mut thick = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(egui::Id::new("dev_thickness"), || 0.5f32)
            });
            let mut dark = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(egui::Id::new("dev_darkness"), || 0.35f32)
            });
            let mut s_thick = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(egui::Id::new("dev_shore_thickness"), || 1.0f32)
            });
            let s_dark = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(egui::Id::new("dev_shore_darkness"), || 1.0f32)
            });
            let mut opacity = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(egui::Id::new("dev_territory_opacity"), || 1.0f32)
            });
            let mut blend_mode = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(egui::Id::new("dev_blend_mode"), || 0.0f32)
            });
            let mut bscale = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(egui::Id::new("dev_building_scale"), || 0.75f32)
            });
            let mut conquest_duration = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(egui::Id::new("dev_conquest_duration"), || 2.5f32)
            });
            ui.add(egui::Slider::new(&mut bscale, 0.3..=3.0).text("Building Scale"));
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_building_scale"), bscale));

            ui.add(egui::Slider::new(&mut thick, 0.0..=1.0).text("Border Thk"));
            ui.add(egui::Slider::new(&mut dark, 0.0..=1.0).text("Border Drk"));
            ui.add(egui::Slider::new(&mut s_thick, 0.0..=1.0).text("Shore Thk"));
            ui.add(egui::Slider::new(&mut conquest_duration, 0.1..=10.0).text("Conquest Duration"));
            ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0).text("Territory Opacity"));

            egui::ComboBox::from_label("Map Blend Mode")
                .selected_text(match blend_mode as i32 {
                    0 => "Normal Mix",
                    1 => "Multiply",
                    2 => "Overlay",
                    3 => "All Albedo",
                    _ => "Overlay",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut blend_mode, 0.0f32, "Normal Mix");
                    ui.selectable_value(&mut blend_mode, 1.0f32, "Multiply");
                    ui.selectable_value(&mut blend_mode, 2.0f32, "Overlay");
                     ui.selectable_value(&mut blend_mode, 3.0f32, "All Albedo");
                });

            ui.separator();
            ui.label(
                RichText::new("Bunker Laser VFX")
                    .strong()
                    .color(Color32::WHITE),
            );
            let mut laser_target = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(egui::Id::new("dev_bunker_laser_target"), || true)
            });
            let mut laser_arc = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(egui::Id::new("dev_bunker_laser_arc"), || true)
            });
            let mut laser_scatter = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(egui::Id::new("dev_bunker_laser_scatter"), || false)
            });
            ui.checkbox(&mut laser_target, "Target seeking");
            ui.checkbox(&mut laser_arc, "Plasma arc");
            ui.checkbox(&mut laser_scatter, "Volley scatter");
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_bunker_laser_target"), laser_target));
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_bunker_laser_arc"), laser_arc));
            ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new("dev_bunker_laser_scatter"), laser_scatter)
            });

            let mut vfx_flags = ctx.data_mut(|d| {
                *d.get_temp_mut_or_insert_with(egui::Id::new("dev_vfx_flags"), crate::app::DevVfxFlags::default)
            });

            ui.separator();
            ui.collapsing(RichText::new("VFX Toggles (Benchmark)").strong().color(Color32::WHITE), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("All On").clicked() {
                        vfx_flags = crate::app::DevVfxFlags::default();
                    }
                    if ui.button("All Off").clicked() {
                        vfx_flags = crate::app::DevVfxFlags {
                            conquer: false,
                            border_breathe: false,
                            energy_flow: false,
                            heartbeat: false,
                            war_fog: false,
                            fallout: false,
                            ambient_grade: false,
                            holo_grid: false,
                            tower: false,
                            tower_range: false,
                            attack_lines: false,
                            attack_badges: false,
                            click_markers: false,
                            nuke_preview: false,
                            floating_notices: false,
                            death_nameplates: false,
                            status_emojis: false,
                            upgrade_plate: false,
                            placement_preview: false,
                            mover_trails: false,
                            railways: false,
                            fleet_blink: false,
                            bot_avatars: false,
                            nameplate_names: false,
                            nameplate_troops: false,
                        };
                    }
                });

                ui.small("GPU Effects");
                ui.checkbox(&mut vfx_flags.conquer, "Conquer shockwave");
                ui.checkbox(&mut vfx_flags.border_breathe, "Border breathe");
                ui.checkbox(&mut vfx_flags.energy_flow, "Contested shimmer");
                ui.checkbox(&mut vfx_flags.heartbeat, "Territory heartbeat");
                ui.checkbox(&mut vfx_flags.war_fog, "War fog / Frontier");
                ui.checkbox(&mut vfx_flags.fallout, "Nuclear fallout");
                ui.checkbox(&mut vfx_flags.ambient_grade, "Ambient grading");
                ui.checkbox(&mut vfx_flags.holo_grid, "Holographic grid");

                ui.separator();
                ui.small("Tower & Combat VFX");
                ui.checkbox(&mut vfx_flags.tower, "Bunker laser");
                ui.checkbox(&mut vfx_flags.tower_range, "Bunker range circle");
                ui.checkbox(&mut vfx_flags.attack_lines, "Attack threat lines");
                ui.checkbox(&mut vfx_flags.attack_badges, "Attack troop badges");

                ui.separator();
                ui.small("World & UI VFX");
                ui.checkbox(&mut vfx_flags.click_markers, "Click markers");
                ui.checkbox(&mut vfx_flags.nuke_preview, "Nuke preview");
                ui.checkbox(&mut vfx_flags.floating_notices, "Floating notices");
                ui.checkbox(&mut vfx_flags.death_nameplates, "Death nameplates");
                ui.checkbox(&mut vfx_flags.status_emojis, "Status emojis");
                ui.checkbox(&mut vfx_flags.upgrade_plate, "Upgrade plate");
                ui.checkbox(&mut vfx_flags.placement_preview, "Placement preview");
                ui.checkbox(&mut vfx_flags.mover_trails, "Mover trails");
                ui.checkbox(&mut vfx_flags.railways, "Railways");
                ui.checkbox(&mut vfx_flags.fleet_blink, "Fleet retreat cross");

                ui.separator();
                ui.small("Nameplate Benchmark");
                ui.checkbox(&mut vfx_flags.bot_avatars, "Bot avatars");
                ui.checkbox(&mut vfx_flags.nameplate_names, "Nameplate names");
                ui.checkbox(&mut vfx_flags.nameplate_troops, "Nameplate troops");
            });
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_vfx_flags"), vfx_flags));

            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_thickness"), thick));
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_darkness"), dark));
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_shore_thickness"), s_thick));
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_shore_darkness"), s_dark));
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_territory_opacity"), opacity));
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_blend_mode"), blend_mode));
            ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new("dev_conquest_duration"), conquest_duration)
            });
        });
    }
}
