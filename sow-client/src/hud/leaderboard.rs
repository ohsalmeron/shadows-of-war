use crate::app::SowApp;
use crate::hud::avatar::paint_circular_avatar;
use crate::hud::nameplate::nameplate_matte_player_rgb;
use egui::{Align2, Color32, FontId, Pos2, RichText, Stroke, Vec2};
use sow_core::player::{Leader, PlayerType};
use sow_core::protocol::PlayerSnapshot;

const REFRESH_INTERVAL_SECS: f32 = 2.0;
const TOP_N: usize = 5;
const ROW_HEIGHT: f32 = 36.0;
const AVATAR_RADIUS: f32 = 11.0;
const RANK_BADGE: f32 = 28.0;

#[derive(Clone, Debug)]
pub struct LeaderboardRanking {
    pub id: u16,
    pub tiles: u32,
    pub troops: f64,
}

#[derive(Clone, Debug)]
pub struct LeaderboardRowDisplay {
    pub name: String,
    pub player_type: PlayerType,
    pub leader: Leader,
    pub color: [f32; 3],
    pub active_emoji: Option<String>,
}

/// Sorted rank index (0-based) and player id for rows shown this frame.
type VisibleRow = (usize, u16);

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

fn compute_visible_rows(
    rankings: &[LeaderboardRanking],
    show_all: bool,
    my_id: Option<u16>,
) -> Vec<VisibleRow> {
    let alive_count = rankings.len();
    if alive_count == 0 {
        return Vec::new();
    }

    let limit = if show_all {
        alive_count
    } else {
        TOP_N.min(alive_count)
    };

    let mut visible: Vec<VisibleRow> = rankings
        .iter()
        .take(limit)
        .enumerate()
        .map(|(rank_idx, r)| (rank_idx, r.id))
        .collect();

    if let Some(my_id) = my_id {
        let already_visible = visible.iter().any(|(_, id)| *id == my_id);
        if !already_visible {
            if let Some(my_rank_idx) = rankings.iter().position(|r| r.id == my_id) {
                if visible.len() >= limit && limit > 0 {
                    visible.pop();
                }
                visible.push((my_rank_idx, my_id));
            }
        }
    }

    visible
}

fn rank_badge(rank_1based: usize) -> (&'static str, Color32, Color32) {
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
    let (icon, fg, bg) = rank_badge(rank_1based);
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

    let (rect, _) = ui.allocate_exact_size(Vec2::splat(AVATAR_RADIUS * 2.0 + 4.0), egui::Sense::hover());
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

    // Tribe / bot emoji disc (nations and portrait rows handled above)
    let label = if display.player_type == PlayerType::Bot {
        "🏕"
    } else {
        display.leader.menu_emoji()
    };

    painter.circle_filled(center, AVATAR_RADIUS, Color32::from_rgba_unmultiplied(20, 24, 36, 220));
    painter.circle_stroke(center, AVATAR_RADIUS, Stroke::new(1.0_f32, matte));
    painter.text(
        center,
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(14.0),
        Color32::WHITE,
    );
}

fn paint_control_column(ui: &mut egui::Ui, control_pct: f32, matte: Color32) {
    let w = ui.available_width().max(72.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, ROW_HEIGHT - 8.0), egui::Sense::hover());
    let painter = ui.painter();

    let pct_text = format!("{control_pct:.1}%");
    painter.text(
        Pos2::new(rect.left(), rect.top()),
        Align2::LEFT_TOP,
        pct_text,
        FontId::proportional(12.0),
        Color32::from_gray(220),
    );

    let bar_top = rect.top() + 16.0;
    let bar_h = 4.0;
    let bar_rect = egui::Rect::from_min_max(
        Pos2::new(rect.left(), bar_top),
        Pos2::new(rect.right(), bar_top + bar_h),
    );
    painter.rect_filled(bar_rect, 2.0, Color32::from_rgba_unmultiplied(255, 255, 255, 20));
    let fill_w = bar_rect.width() * (control_pct / 100.0).clamp(0.0, 1.0);
    if fill_w > 0.5 {
        let fill_rect = egui::Rect::from_min_max(
            bar_rect.min,
            Pos2::new(bar_rect.left() + fill_w, bar_rect.bottom()),
        );
        painter.rect_filled(fill_rect, 2.0, matte);
    }
}

impl SowApp {
    fn refresh_leaderboard_cache(&mut self) {
        let Some(snap) = &self.sim.current_snapshot else {
            return;
        };

        let mut rankings = Vec::new();
        for p in &snap.players {
            if p.alive {
                rankings.push(LeaderboardRanking {
                    id: p.id,
                    tiles: p.tile_count,
                    troops: p.troops,
                });
            }
        }
        rankings.sort_unstable_by_key(|r| std::cmp::Reverse(r.tiles));
        self.ui.leaderboard_rankings = rankings;

        let my_id = self.sim.my_player_id;
        let visible = compute_visible_rows(
            &self.ui.leaderboard_rankings,
            self.ui.leaderboard_show_all,
            my_id,
        );

        for (_, id) in visible {
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
                },
            );
        }
    }

    pub fn render_leaderboard(&mut self, ctx: &egui::Context) {
        self.ui.app.asset_loader.ensure_hud_icons_loaded(ctx);
        let icon_size = sow_ui::ui::theme::hud_icon_size();

        self.ui.leaderboard_timer -= self.ui.raw_input.predicted_dt;
        if self.ui.leaderboard_timer <= 0.0 {
            self.ui.leaderboard_timer = REFRESH_INTERVAL_SECS;
            self.refresh_leaderboard_cache();
        }

        let leaderboard_icon = self
            .ui
            .app
            .asset_loader
            .hud_icon(sow_ui::ui::HudIcon::Leaderboard)
            .cloned();
        let dev_tools_icon = self
            .ui
            .app
            .asset_loader
            .hud_icon(sow_ui::ui::HudIcon::DevTools)
            .cloned();

        let total_land_tiles = self
            .sim
            .current_snapshot
            .as_ref()
            .map(|s| s.total_land_tiles)
            .unwrap_or(1)
            .max(1);

        let my_id = self.sim.my_player_id;
        let alive_count = self.ui.leaderboard_rankings.len();
        let visible_rows = compute_visible_rows(
            &self.ui.leaderboard_rankings,
            self.ui.leaderboard_show_all,
            my_id,
        );

        egui::Area::new(egui::Id::new("leaderboard_area"))
            .anchor(Align2::LEFT_TOP, Vec2::new(12.0, 12.0))
            .show(ctx, |ui| {
                sow_ui::ui::theme::hud_icon_rail_spacing(ui);
                ui.horizontal(|ui| {
                    if ui
                        .add(sow_ui::widgets::HudIconButton::new(
                            leaderboard_icon.as_ref(),
                            icon_size,
                        ))
                        .clicked()
                    {
                        self.ui.show_leaderboard = !self.ui.show_leaderboard;
                    }

                    if ui
                        .add(sow_ui::widgets::HudIconButton::new(
                            dev_tools_icon.as_ref(),
                            icon_size,
                        ))
                        .clicked()
                    {
                        self.ui.show_dev_sidebar = !self.ui.show_dev_sidebar;
                    }
                });

                if !self.ui.show_leaderboard && !self.ui.show_dev_sidebar {
                    return;
                }

                ui.add_space(8.0);
                ui.set_min_width(320.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;

                    if self.ui.show_leaderboard {
                            let win_pct = self
                                .sim
                                .engine
                                .as_ref()
                                .map(|e| e.state.config.map_control_win_percentage)
                                .unwrap_or(0.60);

                            ui.vertical_centered(|ui| {
                                egui::Frame::new()
                                    .fill(sow_ui::ui::theme::nickname_field_bg())
                                    .stroke(egui::Stroke::new(
                                        1.0_f32,
                                        sow_ui::ui::theme::accent_ranked_gold(),
                                    ))
                                    .corner_radius(8.0)
                                    .inner_margin(egui::Margin::symmetric(16, 8))
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

                            ui.add_space(6.0);

                            // Column headers
                            ui.horizontal(|ui| {
                                ui.allocate_exact_size(Vec2::new(RANK_BADGE, 14.0), egui::Sense::hover());
                                ui.add_space(4.0);
                                ui.label(RichText::new("Player").strong().size(11.0).color(Color32::from_gray(160)));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(
                                        RichText::new("Troops")
                                            .strong()
                                            .size(11.0)
                                            .color(Color32::from_gray(160)),
                                    );
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new("Control")
                                            .strong()
                                            .size(11.0)
                                            .color(Color32::from_gray(160)),
                                    );
                                });
                            });

                            ui.add_space(2.0);

                            let max_scroll_h = if self.ui.leaderboard_show_all {
                                360.0
                            } else {
                                (ROW_HEIGHT + 4.0) * TOP_N as f32 + 8.0
                            };

                            egui::ScrollArea::vertical()
                                .max_height(max_scroll_h)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    for (rank_idx, player_id) in &visible_rows {
                                        let rank_1based = rank_idx + 1;
                                        let is_me = Some(*player_id) == my_id;

                                        let tiles = self
                                            .ui
                                            .leaderboard_rankings
                                            .iter()
                                            .find(|r| r.id == *player_id)
                                            .map(|r| r.tiles)
                                            .unwrap_or(0);
                                        let troops = self
                                            .ui
                                            .leaderboard_rankings
                                            .iter()
                                            .find(|r| r.id == *player_id)
                                            .map(|r| r.troops)
                                            .unwrap_or(0.0);

                                        let control_pct =
                                            (tiles as f32 / total_land_tiles as f32) * 100.0;

                                        let display =
                                            self.ui.leaderboard_display.get(player_id).cloned();
                                        let matte = display
                                            .as_ref()
                                            .map(|d| nameplate_matte_player_rgb(d.color))
                                            .unwrap_or(Color32::from_gray(140));

                                        let use_portrait = rank_1based <= 3 || is_me;

                                        let row_bg = if is_me {
                                            Color32::from_rgba_unmultiplied(250, 204, 21, 28)
                                        } else if rank_1based % 2 == 0 {
                                            Color32::from_rgba_unmultiplied(255, 255, 255, 6)
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
                                                    // Left accent for self
                                                    if is_me {
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
                                                        paint_player_icon(
                                                            ui,
                                                            self,
                                                            row_display,
                                                            use_portrait,
                                                        );

                                                        ui.vertical(|ui| {
                                                            ui.spacing_mut().item_spacing.y = 0.0;
                                                            let mut name_text =
                                                                row_display.name.clone();
                                                            if is_me {
                                                                name_text =
                                                                    format!("YOU — {name_text}");
                                                            }
                                                            ui.horizontal(|ui| {
                                                                ui.label(
                                                                    RichText::new(&name_text)
                                                                        .size(13.0)
                                                                        .color(if is_me {
                                                                            sow_ui::ui::theme::accent_ranked_gold()
                                                                        } else {
                                                                            Color32::from_gray(235)
                                                                        })
                                                                        .strong(),
                                                                );
                                                                if let Some(emoji) =
                                                                    &row_display.active_emoji
                                                                {
                                                                    ui.label(
                                                                        RichText::new(emoji)
                                                                            .size(12.0),
                                                                    );
                                                                }
                                                            });
                                                        });
                                                    } else {
                                                        ui.label(
                                                            RichText::new("…")
                                                                .color(Color32::GRAY),
                                                        );
                                                    }

                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            ui.label(
                                                                RichText::new(
                                                                    sow_ui::utils::format_number(
                                                                        troops,
                                                                    ),
                                                                )
                                                                .size(12.0)
                                                                .color(Color32::from_gray(200))
                                                                .family(egui::FontFamily::Monospace),
                                                            );
                                                            ui.add_space(8.0);
                                                            ui.scope(|ui| {
                                                                ui.set_min_width(80.0);
                                                                paint_control_column(
                                                                    ui,
                                                                    control_pct,
                                                                    matte,
                                                                );
                                                            });
                                                        },
                                                    );
                                                });
                                            });

                                        ui.add_space(2.0);
                                    }
                                });

                            if alive_count > TOP_N {
                                ui.add_space(4.0);
                                let label = if self.ui.leaderboard_show_all {
                                    "- Show top 5".to_owned()
                                } else {
                                    format!("+ Show all ({alive_count})")
                                };
                                if ui
                                    .button(RichText::new(label).size(12.0))
                                    .clicked()
                                {
                                    self.ui.leaderboard_show_all =
                                        !self.ui.leaderboard_show_all;
                                }
                            }
                        }

                    if self.ui.show_dev_sidebar {
                        if self.ui.show_leaderboard {
                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);
                        }
                            ui.style_mut().spacing.slider_width = 100.0;
                            ui.style_mut().spacing.item_spacing = Vec2::new(4.0, 4.0);

                            let mut thick = ctx.data_mut(|d| {
                                *d.get_temp_mut_or_insert_with(
                                    egui::Id::new("dev_thickness"),
                                    || 1.0f32,
                                )
                            });
                            let mut dark = ctx.data_mut(|d| {
                                *d.get_temp_mut_or_insert_with(
                                    egui::Id::new("dev_darkness"),
                                    || 0.35f32,
                                )
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
                            ui.add(
                                egui::Slider::new(&mut bscale, 0.3..=3.0).text("Building Scale"),
                            );
                            ctx.data_mut(|d| {
                                d.insert_temp(egui::Id::new("dev_building_scale"), bscale)
                            });

                            ui.add(egui::Slider::new(&mut thick, 0.0..=1.0).text("Border Thk"));
                            ui.add(egui::Slider::new(&mut dark, 0.0..=1.0).text("Border Drk"));
                            ui.add(egui::Slider::new(&mut s_thick, 0.0..=1.0).text("Shore Thk"));
                            ui.add(egui::Slider::new(&mut s_dark, 0.0..=1.0).text("Shore Drk"));

                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_thickness"), thick));
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_darkness"), dark));
                            ctx.data_mut(|d| {
                                d.insert_temp(egui::Id::new("dev_shore_thickness"), s_thick)
                            });
                            ctx.data_mut(|d| {
                                d.insert_temp(egui::Id::new("dev_shore_darkness"), s_dark)
                            });
                    }
                });
            });
    }
}
