use super::{LeaderboardRanking, LeaderboardRowDisplay, TeamRanking};
use crate::app::SowApp;
use crate::hud::avatar::paint_circular_avatar;
use crate::hud::nameplate::nameplate_matte_player_rgb;
use egui::{Align, Align2, Color32, FontId, Layout, RichText, Sense, Stroke, Vec2};
use sow_core::player::PlayerType;
use sow_core::protocol::{PlayerSnapshot, Team};

pub(super) const REFRESH_INTERVAL_SECS: f32 = 2.0;
pub(super) const SCROLL_LOAD_STEP: usize = 10;
pub(super) const SCROLL_NEAR_BOTTOM: f32 = 24.0;
pub(super) const TABLE_HEADER_H: f32 = 18.0;

pub(super) struct LeaderboardMetrics {
    pub(super) is_mobile: bool,
    pub(super) row_height: f32,
    pub(super) rank_badge: f32,
    pub(super) avatar_radius: f32,
    pub(super) stat_font: f32,
    pub(super) name_font: f32,
    pub(super) control_col_w: f32,
    pub(super) troops_col_w: f32,
}

impl LeaderboardMetrics {
    pub(super) fn from_ctx(ctx: &egui::Context) -> Self {
        let is_mobile = sow_ui_kit::theme::compact_viewport(ctx);
        if is_mobile {
            Self {
                is_mobile: true,
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
                row_height: 48.0,
                rank_badge: 32.0,
                avatar_radius: 13.0,
                stat_font: 16.0,
                name_font: 14.0,
                control_col_w: 64.0,
                troops_col_w: 80.0,
            }
        }
    }

    fn avatar_col_w(&self) -> f32 {
        self.avatar_radius * 2.0 + 8.0
    }
}

pub(super) fn snapshot_display_name(p: &PlayerSnapshot) -> String {
    sow_core::player::display_name(p.id, &p.name, p.player_type)
}

pub(super) fn is_team_mode(app: &SowApp) -> bool {
    app.sim.engine.as_ref().is_some_and(|e| {
        e.state.config.game_mode == "Teams" || e.state.config.game_mode == "HumansVsNations"
    })
}

pub(super) fn team_label(team: Team) -> &'static str {
    match team {
        Team::Red => "Red",
        Team::Blue => "Blue",
    }
}

pub(super) fn team_color(team: Team) -> [f32; 3] {
    sow_core::player::team_territory_rgb(team)
}

pub(super) fn name_matches_query(name: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    name.to_lowercase().contains(&query.to_lowercase())
}

pub(super) fn filtered_rankings<'a>(
    rankings: &'a [LeaderboardRanking],
    query: &str,
) -> Vec<(usize, &'a LeaderboardRanking)> {
    rankings
        .iter()
        .enumerate()
        .filter(|(_, r)| name_matches_query(&r.name, query))
        .collect()
}

pub(super) fn rank_badge_style(rank_1based: usize) -> (&'static str, Color32, Color32) {
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

pub(super) fn paint_rank_badge(
    ui: &mut egui::Ui,
    rank_1based: usize,
    metrics: &LeaderboardMetrics,
) {
    let (icon, fg, bg) = rank_badge_style(rank_1based);
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(metrics.rank_badge), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 6.0, bg);
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0_f32, fg.linear_multiply(0.35)),
        egui::StrokeKind::Inside,
    );
    if rank_1based <= 3 {
        let badge_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(24.0, 24.0));
        if !sow_ui_kit::widgets::try_paint_emoji(painter, icon, badge_rect, fg) {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                icon,
                FontId::proportional(16.0),
                fg,
            );
        }
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

pub(super) fn paint_player_icon(
    ui: &mut egui::Ui,
    app: &SowApp,
    display: &LeaderboardRowDisplay,
    use_portrait: bool,
    metrics: &LeaderboardMetrics,
) {
    let icon_rgb = if let Some(team) = display.team {
        sow_core::player::team_territory_rgb(team)
    } else if display.player_type == PlayerType::Nation {
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
        let avatar_tex = app.ui.app.asset_loader.avatars.get(&display.leader).or(app
            .ui
            .app
            .asset_loader
            .avatar_fallback
            .as_ref());
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
    if !sow_ui_kit::widgets::paint_emoji_centered(painter, label, center, 20.0, Color32::WHITE) {
        painter.text(
            center,
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(14.0),
            Color32::WHITE,
        );
    }
}

pub(super) fn paint_leaderboard_header(ui: &mut egui::Ui, metrics: &LeaderboardMetrics) {
    let scroll_bar_style = ui.spacing().scroll;
    let scrollbar_w = scroll_bar_style.bar_width + scroll_bar_style.bar_outer_margin * 2.0;
    let spacing = ui.spacing().item_spacing.x;

    ui.horizontal(|ui| {
        let name_w = (ui.available_width()
            - scrollbar_w
            - metrics.rank_badge
            - metrics.avatar_col_w()
            - metrics.control_col_w
            - metrics.troops_col_w
            - 4.0 * spacing)
            .max(80.0);

        ui.allocate_exact_size(
            Vec2::new(metrics.rank_badge, TABLE_HEADER_H),
            Sense::hover(),
        );
        ui.allocate_exact_size(
            Vec2::new(metrics.avatar_col_w(), TABLE_HEADER_H),
            Sense::hover(),
        );
        ui.allocate_ui_with_layout(
            Vec2::new(name_w, TABLE_HEADER_H),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_width(name_w);
                ui.label(header_label("Player"));
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(metrics.control_col_w, TABLE_HEADER_H),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.set_width(metrics.control_col_w);
                ui.label(header_label("Control"));
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(metrics.troops_col_w, TABLE_HEADER_H),
            Layout::right_to_left(Align::Center),
            |ui| {
                ui.set_width(metrics.troops_col_w);
                ui.label(header_label("Troops"));
            },
        );
    });
}

pub(super) struct RowPaintCtx<'a> {
    pub(super) app: &'a SowApp,
    pub(super) total_land_tiles: u32,
    pub(super) my_id: Option<u16>,
}

pub(super) fn paint_leaderboard_player_row(
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
            let row_response = ui.horizontal(|ui| {
                let spacing = ui.spacing().item_spacing.x;
                let scroll_bar_style = ui.spacing().scroll;
                let scrollbar_w =
                    scroll_bar_style.bar_width + scroll_bar_style.bar_outer_margin * 2.0;
                let extra_sub = if is_sticky_self { scrollbar_w } else { 0.0 };
                let name_w = (ui.available_width()
                    - extra_sub
                    - metrics.rank_badge
                    - metrics.avatar_col_w()
                    - metrics.control_col_w
                    - metrics.troops_col_w
                    - 4.0 * spacing)
                    .max(80.0);

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
                                sow_ui_kit::theme::accent_ranked_gold(),
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
                    Vec2::new(name_w, metrics.row_height),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.set_width(name_w);
                        if let Some(ref row_display) = display {
                            let mut name_text = row_display.name.clone();
                            if highlight {
                                name_text = format!("YOU — {name_text}");
                            }
                            ui.horizontal(|ui| {
                                let name_color = if highlight {
                                    sow_ui_kit::theme::accent_ranked_gold()
                                } else {
                                    Color32::from_gray(235)
                                };
                                sow_ui_kit::widgets::emoji_label(
                                    ui,
                                    &name_text,
                                    FontId::proportional(metrics.name_font),
                                    name_color,
                                );
                                if let Some(emoji) = &row_display.active_emoji {
                                    let (icon_rect, _) = ui.allocate_exact_size(
                                        egui::vec2(18.0, 18.0),
                                        egui::Sense::hover(),
                                    );
                                    if !sow_ui_kit::widgets::try_paint_emoji(
                                        ui.painter(),
                                        emoji,
                                        icon_rect,
                                        Color32::WHITE,
                                    ) {
                                        ui.label(RichText::new(emoji.as_str()).size(12.0));
                                    }
                                }
                            });
                        }
                    },
                );

                ui.allocate_ui_with_layout(
                    Vec2::new(metrics.control_col_w, metrics.row_height),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        ui.set_width(metrics.control_col_w);
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
                        ui.set_width(metrics.troops_col_w);
                        ui.label(
                            RichText::new(sow_ui_kit::utils::format_number(ranking.troops))
                                .font(FontId::monospace(metrics.stat_font))
                                .color(Color32::from_gray(220)),
                        );
                    },
                );
            });
            let kda_ratio = if ranking.deaths == 0 {
                ranking.kills as f32 + ranking.assists as f32 * 0.5
            } else {
                (ranking.kills as f32 + ranking.assists as f32 * 0.5) / ranking.deaths as f32
            };
            row_response.response.on_hover_ui(|ui| {
                ui.label(format!(
                    "K/D/A: {}/{}/{}",
                    ranking.kills, ranking.deaths, ranking.assists
                ));
                ui.label(format!("Ratio: {kda_ratio:.2}"));
            });
        });
}

fn header_label(text: &str) -> RichText {
    RichText::new(text)
        .strong()
        .size(10.0)
        .color(Color32::from_gray(120))
}

pub(super) fn paint_team_row(
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
                            sow_ui_kit::theme::accent_ranked_gold()
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
