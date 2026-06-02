use crate::app::SowApp;
use crate::hud::avatar::paint_circular_avatar;
use crate::hud::nameplate::nameplate_matte_player_rgb;
use egui::{Align, Align2, Color32, FontId, Layout, Pos2, RichText, Sense, Stroke, Vec2};
use sow_core::player::{Leader, PlayerType};
use sow_core::protocol::{PlayerSnapshot, Team};
use std::collections::HashSet;

pub const INITIAL_VISIBLE_LIMIT: usize = 10;
const REFRESH_INTERVAL_SECS: f32 = 2.0;
const SCROLL_LOAD_STEP: usize = 10;
const SCROLL_NEAR_BOTTOM: f32 = 24.0;
const DESKTOP_PANEL_W: f32 = 520.0;
const TABLE_HEADER_H: f32 = 18.0;

struct LeaderboardMetrics {
    is_mobile: bool,
    panel_width: f32,
    row_height: f32,
    rank_badge: f32,
    avatar_radius: f32,
    stat_font: f32,
    name_font: f32,
    control_col_w: f32,
    troops_col_w: f32,
}

impl LeaderboardMetrics {
    fn from_ctx(ctx: &egui::Context) -> Self {
        let screen = ctx.content_rect();
        let is_mobile = sow_ui::ui::theme::compact_viewport(ctx);
        if is_mobile {
            Self {
                is_mobile: true,
                panel_width: screen.width(),
                row_height: 52.0,
                rank_badge: 34.0,
                avatar_radius: 14.0,
                stat_font: 16.0,
                name_font: 15.0,
                control_col_w: 56.0,
                troops_col_w: 76.0,
            }
        } else {
            Self {
                is_mobile: false,
                panel_width: DESKTOP_PANEL_W,
                row_height: 48.0,
                rank_badge: 32.0,
                avatar_radius: 13.0,
                stat_font: 16.0,
                name_font: 14.0,
                control_col_w: 56.0,
                troops_col_w: 80.0,
            }
        }
    }

    fn avatar_col_w(&self) -> f32 {
        self.avatar_radius * 2.0 + 8.0
    }

    fn name_col_w(&self) -> f32 {
        (self.panel_width
            - self.rank_badge
            - self.avatar_col_w()
            - self.control_col_w
            - self.troops_col_w
            - 8.0)
            .max(80.0)
    }
}

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

fn paint_rank_badge(ui: &mut egui::Ui, rank_1based: usize, metrics: &LeaderboardMetrics) {
    let (icon, fg, bg) = rank_badge_style(rank_1based);
    let (rect, _) = ui.allocate_exact_size(
        Vec2::splat(metrics.rank_badge),
        egui::Sense::hover(),
    );
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
    metrics: &LeaderboardMetrics,
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

    let (rect, _) = ui.allocate_exact_size(
        Vec2::splat(metrics.avatar_radius * 2.0 + 4.0),
        egui::Sense::hover(),
    );
    let center = rect.center();
    let painter = ui.painter();
    let avatar_r = metrics.avatar_radius;

    if display.player_type == PlayerType::Nation {
        paint_circular_avatar(painter, center, avatar_r, None, vibrant, vibrant);
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
        paint_circular_avatar(painter, center, avatar_r, tex_id, matte, vibrant);
        return;
    }

    let label = if display.player_type == PlayerType::Bot {
        "🏕"
    } else {
        display.leader.menu_emoji()
    };

    painter.circle_filled(
        center,
        avatar_r,
        Color32::from_rgba_unmultiplied(20, 24, 36, 220),
    );
    painter.circle_stroke(center, avatar_r, Stroke::new(1.0_f32, matte));
    painter.text(
        center,
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(14.0),
        Color32::WHITE,
    );
}

fn paint_leaderboard_header(ui: &mut egui::Ui, metrics: &LeaderboardMetrics) {
    ui.horizontal(|ui| {
        ui.allocate_exact_size(Vec2::new(metrics.rank_badge, TABLE_HEADER_H), Sense::hover());
        ui.allocate_exact_size(
            Vec2::new(metrics.avatar_col_w(), TABLE_HEADER_H),
            Sense::hover(),
        );
        ui.allocate_ui_with_layout(
            Vec2::new(metrics.name_col_w(), TABLE_HEADER_H),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.label(header_label("Player"));
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(metrics.control_col_w, TABLE_HEADER_H),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.label(header_label("Control"));
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(metrics.troops_col_w, TABLE_HEADER_H),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.label(header_label("Troops"));
            },
        );
    });
}

struct RowPaintCtx<'a> {
    app: &'a SowApp,
    total_land_tiles: u32,
    my_id: Option<u16>,
}

fn paint_leaderboard_player_row(
    ui: &mut egui::Ui,
    ctx: &RowPaintCtx<'_>,
    metrics: &LeaderboardMetrics,
    rank_idx: usize,
    ranking: &LeaderboardRanking,
    is_sticky_self: bool,
    striped: bool,
) {
    let rank_1based = rank_idx + 1;
    let player_id = ranking.id;
    let is_me = Some(player_id) == ctx.my_id;
    let highlight = is_me || is_sticky_self;
    let control_pct = (ranking.tiles as f32 / ctx.total_land_tiles as f32) * 100.0;
    let display = ctx.app.ui.leaderboard_display.get(&player_id).cloned();
    let use_portrait = rank_1based <= 3 || highlight;

    let row_fill = if highlight {
        Color32::from_rgba_unmultiplied(250, 204, 21, 28)
    } else if striped {
        Color32::from_rgba_unmultiplied(255, 255, 255, 8)
    } else {
        Color32::TRANSPARENT
    };

    egui::Frame::new()
        .fill(row_fill)
        .inner_margin(egui::Margin::symmetric(0, 0))
        .show(ui, |ui| {
            ui.set_min_height(metrics.row_height);
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(metrics.rank_badge, metrics.row_height),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        if highlight {
                            let (accent, _) = ui.allocate_exact_size(
                                Vec2::new(3.0, metrics.row_height - 4.0),
                                Sense::hover(),
                            );
                            ui.painter().rect_filled(
                                accent,
                                2.0,
                                sow_ui::ui::theme::accent_ranked_gold(),
                            );
                        }
                        paint_rank_badge(ui, rank_1based, metrics);
                    },
                );

                ui.allocate_ui_with_layout(
                    Vec2::new(metrics.avatar_col_w(), metrics.row_height),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        if let Some(ref row_display) = display {
                            paint_player_icon(ui, ctx.app, row_display, use_portrait, metrics);
                        } else {
                            ui.label(RichText::new("…").color(Color32::GRAY));
                        }
                    },
                );

                ui.allocate_ui_with_layout(
                    Vec2::new(metrics.name_col_w(), metrics.row_height),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        if let Some(ref row_display) = display {
                            let mut name_text = row_display.name.clone();
                            if highlight {
                                name_text = format!("YOU — {name_text}");
                            }
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&name_text)
                                            .size(metrics.name_font)
                                            .color(if highlight {
                                                sow_ui::ui::theme::accent_ranked_gold()
                                            } else {
                                                Color32::from_gray(235)
                                            })
                                            .strong(),
                                    )
                                    .truncate(),
                                );
                                if let Some(emoji) = &row_display.active_emoji {
                                    ui.label(RichText::new(emoji).size(12.0));
                                }
                            });
                        }
                    },
                );

                ui.allocate_ui_with_layout(
                    Vec2::new(metrics.control_col_w, metrics.row_height),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        ui.label(
                            RichText::new(format!("{control_pct:.1}%"))
                                .font(FontId::monospace(metrics.stat_font))
                                .color(Color32::WHITE),
                        );
                    },
                );

                ui.allocate_ui_with_layout(
                    Vec2::new(metrics.troops_col_w, metrics.row_height),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        ui.label(
                            RichText::new(sow_ui::utils::format_number(ranking.troops))
                                .font(FontId::monospace(metrics.stat_font))
                                .color(Color32::from_gray(220)),
                        );
                    },
                );
            });
        });
}

fn header_label(text: &str) -> RichText {
    RichText::new(text)
        .strong()
        .size(10.0)
        .color(Color32::from_gray(120))
}

fn paint_team_row(
    ui: &mut egui::Ui,
    team: &TeamRanking,
    control_pct: f32,
    is_my_team: bool,
    metrics: &LeaderboardMetrics,
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
                        .size(metrics.name_font)
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

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{control_pct:.1}%"))
                            .font(FontId::monospace(metrics.stat_font))
                            .color(Color32::WHITE),
                    );
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
        ui.set_min_width(metrics.panel_width);
        ui.set_max_width(metrics.panel_width);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = if metrics.is_mobile { 10.0 } else { 8.0 };

            if metrics.is_mobile {
                ui.add_space(44.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Leaderboard")
                            .size(20.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new("✕").size(16.0))
                            .on_hover_text("Close")
                            .clicked()
                        {
                            self.ui.show_leaderboard = false;
                        }
                    });
                });
                ui.add_space(4.0);
            }

            ui.vertical_centered(|ui| {
                egui::Frame::new()
                    .fill(sow_ui::ui::theme::nickname_field_bg())
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        sow_ui::ui::theme::accent_ranked_gold(),
                    ))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(
                        if metrics.is_mobile { 14 } else { 12 },
                        if metrics.is_mobile { 8 } else { 6 },
                    ))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!(
                                "👑 Domination Victory: Control {:.0}% of Map",
                                win_pct * 100.0
                            ))
                            .color(sow_ui::ui::theme::accent_ranked_gold())
                            .size(if metrics.is_mobile { 14.0 } else { 13.0 })
                            .strong(),
                        );
                    });
            });

            ui.add_space(4.0);

            egui::Frame::new()
                .fill(sow_ui::ui::theme::leaderboard_search_field_bg())
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    sow_ui::ui::theme::leaderboard_search_field_border(),
                ))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(8, if metrics.is_mobile { 6 } else { 4 }))
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
            let table_scroll_h = (ui.available_height()
                - TABLE_HEADER_H
                - sticky_reserve
                - mobile_back_reserve)
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
                let near_bottom =
                    state.offset.y + viewport_h >= content_h - SCROLL_NEAR_BOTTOM;
                let visible_limit = self.ui.leaderboard_visible_limit;
                if near_bottom && visible_limit < filtered.len() {
                    if self.ui.leaderboard_paged_through_limit != visible_limit {
                        self.ui.leaderboard_visible_limit =
                            (visible_limit + SCROLL_LOAD_STEP).min(filtered.len());
                        self.ui.leaderboard_paged_through_limit =
                            self.ui.leaderboard_visible_limit;
                    }
                } else if !near_bottom {
                    self.ui.leaderboard_paged_through_limit = 0;
                }
            }

            if show_sticky_self {
                if let Some(my_id) = my_id {
                    ui.separator();
                    if let Some((rank_idx, ranking)) =
                        filtered.iter().find(|(_, r)| r.id == my_id)
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

        let metrics = LeaderboardMetrics::from_ctx(ctx);

        // Mobile fullscreen overlay (drawn first, under the toggle bar).
        if self.ui.show_leaderboard && metrics.is_mobile {
            let screen = ctx.content_rect();

            egui::Area::new(egui::Id::new("leaderboard_mobile_dimmer"))
                .order(egui::Order::Foreground)
                .fixed_pos(Pos2::ZERO)
                .show(ctx, |ui| {
                    ui.set_min_size(screen.size());
                    ui.painter()
                        .rect_filled(screen, 0.0, Color32::from_black_alpha(160));
                });

            egui::Area::new(egui::Id::new("leaderboard_mobile_fullscreen"))
                .order(egui::Order::Foreground)
                .fixed_pos(Pos2::ZERO)
                .show(ctx, |ui| {
                    ui.set_min_size(screen.size());
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(6, 8, 12, 250))
                        .inner_margin(egui::Margin::symmetric(16, 12))
                        .show(ui, |ui| {
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
                        });
                });
        }

        // Single toggle area — always on top, always clickable (trophy toggles open/closed).
        egui::Area::new(egui::Id::new("leaderboard_area"))
            .order(egui::Order::Foreground)
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

                    if self.ui.show_dev_sidebar {
                        ui.add_space(8.0);
                        self.render_dev_sidebar(ctx, ui);
                    }

                    if self.ui.show_leaderboard && !metrics.is_mobile {
                        ui.add_space(8.0);
                        sow_ui::ui::theme::leaderboard_panel_frame().show(ui, |ui| {
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
                        });
                    }
                });
    }

    fn render_dev_sidebar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.style_mut().spacing.slider_width = 100.0;
        ui.style_mut().spacing.item_spacing = Vec2::new(4.0, 4.0);

        let mut thick = ctx.data_mut(|d| {
            *d.get_temp_mut_or_insert_with(egui::Id::new("dev_thickness"), || 1.0f32)
        });
        let mut dark = ctx.data_mut(|d| {
            *d.get_temp_mut_or_insert_with(egui::Id::new("dev_darkness"), || 0.35f32)
        });
        let mut s_thick = ctx.data_mut(|d| {
            *d.get_temp_mut_or_insert_with(egui::Id::new("dev_shore_thickness"), || 1.0f32)
        });
        let mut s_dark = ctx.data_mut(|d| {
            *d.get_temp_mut_or_insert_with(egui::Id::new("dev_shore_darkness"), || 1.0f32)
        });
        let mut bscale = ctx.data_mut(|d| {
            *d.get_temp_mut_or_insert_with(egui::Id::new("dev_building_scale"), || 1.0f32)
        });
        ui.add(egui::Slider::new(&mut bscale, 0.3..=3.0).text("Building Scale"));
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_building_scale"), bscale));

        ui.add(egui::Slider::new(&mut thick, 0.0..=1.0).text("Border Thk"));
        ui.add(egui::Slider::new(&mut dark, 0.0..=1.0).text("Border Drk"));
        ui.add(egui::Slider::new(&mut s_thick, 0.0..=1.0).text("Shore Thk"));
        ui.add(egui::Slider::new(&mut s_dark, 0.0..=1.0).text("Shore Drk"));

        ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_thickness"), thick));
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_darkness"), dark));
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_shore_thickness"), s_thick));
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_shore_darkness"), s_dark));
    }
}
