use crate::app::SowApp;
use crate::hud::avatar::paint_circular_avatar;
use crate::hud::nameplate::nameplate_matte_player_rgb;
use egui::{Align2, Color32, FontId, Pos2, RichText, Stroke, Vec2};
use sow_core::player::{Leader, PlayerType};
use sow_core::protocol::{PlayerSnapshot, Team};
use std::collections::HashSet;

pub const INITIAL_VISIBLE_LIMIT: usize = 10;
const REFRESH_INTERVAL_SECS: f32 = 2.0;
const SCROLL_LOAD_STEP: usize = 10;
const SCROLL_NEAR_BOTTOM: f32 = 24.0;
const MAX_SCROLL_H: f32 = 320.0;

const ROW_HEIGHT: f32 = 44.0;
const AVATAR_RADIUS: f32 = 12.0;
const RANK_BADGE: f32 = 30.0;
const CONTROL_BAR_W: f32 = 48.0;
const TROOPS_COL_W: f32 = 72.0;
const STAT_FONT: f32 = 15.0;
const NAME_FONT: f32 = 14.0;

#[derive(Clone, Debug)]
pub struct LeaderboardRanking {
    pub id: u16,
    pub tiles: u32,
    pub troops: f64,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct LeaderboardRowDisplay {
    pub name: String,
    pub player_type: PlayerType,
    pub leader: Leader,
    pub color: [f32; 3],
    pub active_emoji: Option<String>,
    pub team: Option<Team>,
}

#[derive(Clone, Debug)]
pub struct TeamRanking {
    pub team: Team,
    pub tiles: u32,
    pub member_count: u32,
    pub color: [f32; 3],
}

fn snapshot_display_name(p: &PlayerSnapshot) -> String {
    if !p.name.is_empty() {
        return p.name.clone();
    }
    if p.id >= 200 {
        format!("Tribe {}", p.id - 199)
    } else {
        format!("Nation {}", p.id - 103)
    }
}

fn is_team_mode(app: &SowApp) -> bool {
    app.sim
        .engine
        .as_ref()
        .is_some_and(|e| e.state.config.game_mode == "Teams")
}

fn team_label(team: Team) -> &'static str {
    match team {
        Team::Red => "Red",
        Team::Blue => "Blue",
    }
}

fn team_color(team: Team) -> [f32; 3] {
    match team {
        Team::Red => [1.0, 0.2, 0.2],
        Team::Blue => [0.2, 0.5, 1.0],
    }
}

fn name_matches_query(name: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    name.to_lowercase().contains(&query.to_lowercase())
}

fn filtered_rankings<'a>(
    rankings: &'a [LeaderboardRanking],
    query: &str,
) -> Vec<(usize, &'a LeaderboardRanking)> {
    rankings
        .iter()
        .enumerate()
        .filter(|(_, r)| name_matches_query(&r.name, query))
        .collect()
}

fn rank_badge_style(rank_1based: usize) -> (&'static str, Color32, Color32) {
    match rank_1based {
        1 => (
            "👑",
            Color32::from_rgb(250, 204, 21),
            Color32::from_rgba_unmultiplied(250, 204, 21, 25),
        ),
        2 => (
            "🥈",
            Color32::from_rgb(203, 213, 225),
            Color32::from_rgba_unmultiplied(148, 163, 184, 25),
        ),
        3 => (
            "🥉",
            Color32::from_rgb(217, 119, 6),
            Color32::from_rgba_unmultiplied(217, 119, 6, 25),
        ),
        _ => (
            "",
            Color32::from_rgba_unmultiplied(255, 255, 255, 100),
            Color32::from_rgba_unmultiplied(255, 255, 255, 12),
        ),
    }
}

fn paint_rank_badge(ui: &mut egui::Ui, rank_1based: usize) {
    let (icon, fg, bg) = rank_badge_style(rank_1based);
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(RANK_BADGE), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 6.0, bg);
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0_f32, fg.linear_multiply(0.35)),
        egui::StrokeKind::Inside,
    );
    if rank_1based <= 3 {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            icon,
            FontId::proportional(16.0),
            fg,
        );
    } else {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            format!("{rank_1based}"),
            FontId::monospace(12.0),
            fg,
        );
    }
}

fn paint_player_icon(
    ui: &mut egui::Ui,
    app: &SowApp,
    display: &LeaderboardRowDisplay,
    use_portrait: bool,
) {
    let icon_rgb = if display.player_type == PlayerType::Nation {
        display.color
    } else {
        display.leader.filler_rgb()
    };
    let matte = nameplate_matte_player_rgb(icon_rgb);
    let vibrant = Color32::from_rgb(
        (icon_rgb[0] * 255.0) as u8,
        (icon_rgb[1] * 255.0) as u8,
        (icon_rgb[2] * 255.0) as u8,
    );

    let (rect, _) =
        ui.allocate_exact_size(Vec2::splat(AVATAR_RADIUS * 2.0 + 4.0), egui::Sense::hover());
    let center = rect.center();
    let painter = ui.painter();

    if display.player_type == PlayerType::Nation {
        paint_circular_avatar(painter, center, AVATAR_RADIUS, None, vibrant, vibrant);
        return;
    }

    if use_portrait {
        let avatar_tex = app
            .ui
            .app
            .asset_loader
            .avatars
            .get(&display.leader)
            .or(app.ui.app.asset_loader.avatar_fallback.as_ref());
        let tex_id = avatar_tex.map(|t| t.id());
        paint_circular_avatar(painter, center, AVATAR_RADIUS, tex_id, matte, vibrant);
        return;
    }

    let label = if display.player_type == PlayerType::Bot {
        "🏕"
    } else {
        display.leader.menu_emoji()
    };

    painter.circle_filled(
        center,
        AVATAR_RADIUS,
        Color32::from_rgba_unmultiplied(20, 24, 36, 220),
    );
    painter.circle_stroke(center, AVATAR_RADIUS, Stroke::new(1.0_f32, matte));
    painter.text(
        center,
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(14.0),
        Color32::WHITE,
    );
}

fn paint_control_stat(ui: &mut egui::Ui, control_pct: f32, matte: Color32) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(CONTROL_BAR_W + 52.0, ROW_HEIGHT - 8.0),
        egui::Sense::hover(),
    );
    let painter = ui.painter();

    let pct_text = format!("{control_pct:.1}%");
    painter.text(
        Pos2::new(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        pct_text,
        FontId::monospace(STAT_FONT),
        Color32::WHITE,
    );

    let bar_left = rect.left() + 44.0;
    let bar_rect = egui::Rect::from_center_size(
        Pos2::new(bar_left + CONTROL_BAR_W * 0.5, rect.center().y),
        Vec2::new(CONTROL_BAR_W, 6.0),
    );
    painter.rect_filled(bar_rect, 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 25));
    let fill_w = CONTROL_BAR_W * (control_pct / 100.0).clamp(0.0, 1.0);
    if fill_w > 0.5 {
        let fill_rect = egui::Rect::from_min_max(
            bar_rect.min,
            Pos2::new(bar_rect.left() + fill_w, bar_rect.bottom()),
        );
        painter.rect_filled(fill_rect, 3.0, matte);
    }
}

fn paint_troops_stat(ui: &mut egui::Ui, troops: f64) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(TROOPS_COL_W, ROW_HEIGHT - 8.0),
        egui::Sense::hover(),
    );
    ui.painter().text(
        rect.right_center(),
        Align2::RIGHT_CENTER,
        sow_ui::utils::format_number(troops),
        FontId::monospace(STAT_FONT),
        Color32::from_gray(220),
    );
}

struct RowPaintCtx<'a> {
    app: &'a SowApp,
    total_land_tiles: u32,
    my_id: Option<u16>,
}

fn paint_player_row(
    ui: &mut egui::Ui,
    ctx: &RowPaintCtx<'_>,
    rank_idx: usize,
    ranking: &LeaderboardRanking,
    is_sticky_self: bool,
) {
    let rank_1based = rank_idx + 1;
    let player_id = ranking.id;
    let is_me = Some(player_id) == ctx.my_id;
    let control_pct = (ranking.tiles as f32 / ctx.total_land_tiles as f32) * 100.0;

    let display = ctx.app.ui.leaderboard_display.get(&player_id).cloned();
    let matte = display
        .as_ref()
        .map(|d| nameplate_matte_player_rgb(d.color))
        .unwrap_or(Color32::from_gray(140));

    let use_portrait = rank_1based <= 3 || is_me || is_sticky_self;

    let row_bg = if is_me || is_sticky_self {
        Color32::from_rgba_unmultiplied(250, 204, 21, 28)
    } else if rank_1based % 2 == 0 {
        Color32::from_rgba_unmultiplied(255, 255, 255, 8)
    } else {
        Color32::TRANSPARENT
    };

    egui::Frame::new()
        .fill(row_bg)
        .inner_margin(egui::Margin::symmetric(4, 2))
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.set_min_height(ROW_HEIGHT);
            ui.horizontal(|ui| {
                if is_me || is_sticky_self {
                    let (accent, _) = ui.allocate_exact_size(
                        Vec2::new(3.0, ROW_HEIGHT - 4.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(
                        accent,
                        2.0,
                        sow_ui::ui::theme::accent_ranked_gold(),
                    );
                }

                paint_rank_badge(ui, rank_1based);

                if let Some(ref row_display) = display {
                    paint_player_icon(ui, ctx.app, row_display, use_portrait);

                    ui.scope(|ui| {
                        ui.set_min_width(80.0);
                        let mut name_text = row_display.name.clone();
                        if is_me || is_sticky_self {
                            name_text = format!("YOU — {name_text}");
                        }
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&name_text)
                                    .size(NAME_FONT)
                                    .color(if is_me || is_sticky_self {
                                        sow_ui::ui::theme::accent_ranked_gold()
                                    } else {
                                        Color32::from_gray(235)
                                    })
                                    .strong(),
                            );
                            if let Some(emoji) = &row_display.active_emoji {
                                ui.label(RichText::new(emoji).size(12.0));
                            }
                        });
                    });
                } else {
                    ui.label(RichText::new("…").color(Color32::GRAY));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    paint_troops_stat(ui, ranking.troops);
                    paint_control_stat(ui, control_pct, matte);
                });
            });
        });
}

fn paint_team_row(
    ui: &mut egui::Ui,
    team: &TeamRanking,
    control_pct: f32,
    is_my_team: bool,
    total_land_tiles: u32,
) {
    let matte = nameplate_matte_player_rgb(team.color);
    let vibrant = Color32::from_rgb(
        (team.color[0] * 255.0) as u8,
        (team.color[1] * 255.0) as u8,
        (team.color[2] * 255.0) as u8,
    );

    let row_bg = if is_my_team {
        Color32::from_rgba_unmultiplied(250, 204, 21, 20)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 6)
    };

    egui::Frame::new()
        .fill(row_bg)
        .inner_margin(egui::Margin::symmetric(6, 4))
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.set_min_height(36.0);
            ui.horizontal(|ui| {
                let (swatch, _) =
                    ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::hover());
                ui.painter().rect_filled(swatch, 3.0, vibrant);
                ui.painter().rect_stroke(
                    swatch,
                    3.0,
                    Stroke::new(1.0_f32, matte),
                    egui::StrokeKind::Inside,
                );

                ui.label(
                    RichText::new(team_label(team.team))
                        .size(NAME_FONT)
                        .strong()
                        .color(if is_my_team {
                            sow_ui::ui::theme::accent_ranked_gold()
                        } else {
                            Color32::WHITE
                        }),
                );

                ui.label(
                    RichText::new(format!("{} players", team.member_count))
                        .size(11.0)
                        .color(Color32::from_gray(140)),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let _ = total_land_tiles;
                    paint_control_stat(ui, control_pct, matte);
                });
            });
        });
}

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
        let filtered = filtered_rankings(&self.ui.leaderboard_rankings, &self.ui.leaderboard_search);

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
            self.ui.leaderboard_prev_search = self.ui.leaderboard_search.clone();
        }

        let is_open = self.ui.show_leaderboard;
        if is_open && !self.ui.leaderboard_was_open {
            self.ui.leaderboard_visible_limit = INITIAL_VISIBLE_LIMIT;
        }
        self.ui.leaderboard_was_open = is_open;
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
        let my_team = my_id.and_then(|id| {
            self.ui
                .leaderboard_display
                .get(&id)
                .and_then(|d| d.team)
        });

        let team_mode = is_team_mode(self);
        let search_active = !self.ui.leaderboard_search.is_empty();
        let filtered: Vec<(usize, LeaderboardRanking)> = filtered_rankings(
            &self.ui.leaderboard_rankings,
            &self.ui.leaderboard_search,
        )
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
            self.ui
                .leaderboard_rankings
                .iter()
                .any(|r| r.id == id)
                && !scroll_ids.contains(&id)
        });

        let team_rankings = self.ui.leaderboard_team_rankings.clone();
        let win_pct = self
            .sim
            .engine
            .as_ref()
            .map(|e| e.state.config.map_control_win_percentage)
            .unwrap_or(0.60);

        egui::Area::new(egui::Id::new("leaderboard_area"))
            .anchor(Align2::LEFT_TOP, Vec2::new(12.0, 12.0))
            .show(ctx, |ui| {
                sow_ui::ui::theme::hud_panel_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .add(sow_ui::widgets::HudButton::new("🏆"))
                            .on_hover_text("Leaderboard")
                            .clicked()
                        {
                            self.ui.show_leaderboard = !self.ui.show_leaderboard;
                        }

                        if ui
                            .add(sow_ui::widgets::HudButton::new("🛠"))
                            .on_hover_text("Dev Utils")
                            .clicked()
                        {
                            self.ui.show_dev_sidebar = !self.ui.show_dev_sidebar;
                        }
                    });
                });

                if !self.ui.show_leaderboard && !self.ui.show_dev_sidebar {
                    return;
                }

                ui.add_space(8.0);
                ui.set_min_width(340.0);

                if self.ui.show_leaderboard {
                    sow_ui::ui::theme::leaderboard_panel_frame().show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 8.0;

                            ui.vertical_centered(|ui| {
                                egui::Frame::new()
                                    .fill(sow_ui::ui::theme::nickname_field_bg())
                                    .stroke(egui::Stroke::new(
                                        1.0_f32,
                                        sow_ui::ui::theme::accent_ranked_gold(),
                                    ))
                                    .corner_radius(8.0)
                                    .inner_margin(egui::Margin::symmetric(12, 6))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "👑 Domination Victory: Control {:.0}% of Map",
                                                win_pct * 100.0
                                            ))
                                            .color(sow_ui::ui::theme::accent_ranked_gold())
                                            .size(13.0)
                                            .strong(),
                                        );
                                    });
                            });

                            ui.add_space(4.0);

                            // Search bar
                            egui::Frame::new()
                                .fill(sow_ui::ui::theme::leaderboard_search_field_bg())
                                .stroke(egui::Stroke::new(
                                    1.0_f32,
                                    sow_ui::ui::theme::leaderboard_search_field_border(),
                                ))
                                .corner_radius(6.0)
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("🔍").size(14.0));
                                        let response = ui.add(
                                            egui::TextEdit::singleline(&mut self.ui.leaderboard_search)
                                                .hint_text("Search players…")
                                                .desired_width(ui.available_width() - 24.0),
                                        );
                                        if !self.ui.leaderboard_search.is_empty()
                                            && ui.small_button("✕").clicked()
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
                                for team in &team_rankings {
                                    let control_pct =
                                        (team.tiles as f32 / total_land_tiles as f32) * 100.0;
                                    let is_my_team = my_team == Some(team.team);
                                    paint_team_row(
                                        ui,
                                        team,
                                        control_pct,
                                        is_my_team,
                                        total_land_tiles,
                                    );
                                    ui.add_space(2.0);
                                }
                                ui.separator();
                            }

                            ui.horizontal(|ui| {
                                ui.allocate_exact_size(Vec2::new(RANK_BADGE, 14.0), egui::Sense::hover());
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("Player")
                                        .strong()
                                        .size(10.0)
                                        .color(Color32::from_gray(120)),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new("Troops")
                                                .strong()
                                                .size(10.0)
                                                .color(Color32::from_gray(120)),
                                        );
                                        ui.add_space(TROOPS_COL_W - 40.0);
                                        ui.label(
                                            RichText::new("Control")
                                                .strong()
                                                .size(10.0)
                                                .color(Color32::from_gray(120)),
                                        );
                                    },
                                );
                            });

                            let scroll_output = egui::ScrollArea::vertical()
                                .max_height(MAX_SCROLL_H)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let row_ctx = RowPaintCtx {
                                        app: self,
                                        total_land_tiles,
                                        my_id,
                                    };
                                    for (rank_idx, ranking) in filtered.iter().take(scroll_row_count) {
                                        paint_player_row(ui, &row_ctx, *rank_idx, ranking, false);
                                        ui.add_space(2.0);
                                    }
                                });

                            if !search_active {
                                let state = &scroll_output.state;
                                let viewport_h = scroll_output.inner_rect.height();
                                let content_h = scroll_output.content_size.y;
                                let near_bottom = state.offset.y + viewport_h
                                    >= content_h - SCROLL_NEAR_BOTTOM;
                                if near_bottom
                                    && self.ui.leaderboard_visible_limit < filtered.len()
                                {
                                    self.ui.leaderboard_visible_limit = (self
                                        .ui
                                        .leaderboard_visible_limit
                                        + SCROLL_LOAD_STEP)
                                        .min(filtered.len());
                                }
                            }

                            if show_sticky_self {
                                if let Some(my_id) = my_id {
                                    ui.separator();
                                    if let Some((rank_idx, ranking)) = filtered
                                        .iter()
                                        .find(|(_, r)| r.id == my_id)
                                    {
                                        let row_ctx = RowPaintCtx {
                                            app: self,
                                            total_land_tiles,
                                            my_id: Some(my_id),
                                        };
                                        paint_player_row(
                                            ui,
                                            &row_ctx,
                                            *rank_idx,
                                            ranking,
                                            true,
                                        );
                                    }
                                }
                            }
                        });
                    });
                }

                if self.ui.show_dev_sidebar {
                    if self.ui.show_leaderboard {
                        ui.add_space(4.0);
                    }
                    ui.style_mut().spacing.slider_width = 100.0;
                    ui.style_mut().spacing.item_spacing = Vec2::new(4.0, 4.0);

                    let mut thick = ctx.data_mut(|d| {
                        *d.get_temp_mut_or_insert_with(egui::Id::new("dev_thickness"), || 1.0f32)
                    });
                    let mut dark = ctx.data_mut(|d| {
                        *d.get_temp_mut_or_insert_with(egui::Id::new("dev_darkness"), || 0.35f32)
                    });
                    let mut s_thick = ctx.data_mut(|d| {
                        *d.get_temp_mut_or_insert_with(
                            egui::Id::new("dev_shore_thickness"),
                            || 1.0f32,
                        )
                    });
                    let mut s_dark = ctx.data_mut(|d| {
                        *d.get_temp_mut_or_insert_with(
                            egui::Id::new("dev_shore_darkness"),
                            || 1.0f32,
                        )
                    });
                    let mut bscale = ctx.data_mut(|d| {
                        *d.get_temp_mut_or_insert_with(
                            egui::Id::new("dev_building_scale"),
                            || 2.0f32,
                        )
                    });
                    ui.add(egui::Slider::new(&mut bscale, 0.3..=3.0).text("Building Scale"));
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_building_scale"), bscale));

                    ui.add(egui::Slider::new(&mut thick, 0.0..=1.0).text("Border Thk"));
                    ui.add(egui::Slider::new(&mut dark, 0.0..=1.0).text("Border Drk"));
                    ui.add(egui::Slider::new(&mut s_thick, 0.0..=1.0).text("Shore Thk"));
                    ui.add(egui::Slider::new(&mut s_dark, 0.0..=1.0).text("Shore Drk"));

                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_thickness"), thick));
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_darkness"), dark));
                    ctx.data_mut(|d| {
                        d.insert_temp(egui::Id::new("dev_shore_thickness"), s_thick)
                    });
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_shore_darkness"), s_dark));
                }
            });
    }
}
