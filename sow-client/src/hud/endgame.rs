use crate::app::SowApp;
use egui::{Align2, Color32, FontId, RichText};
use sow_core::protocol::Team;

impl SowApp {
    pub(crate) fn render_endgame_ui(&mut self, ctx: &egui::Context) {
        let lang = self.ui.app.settings_state.language;
        let strings = &sow_i18n::get(lang).endgame;

        let mut endgame_active = false;
        let mut is_victory = false;
        let mut text_title = String::new();
        let mut text_subtitle = String::new();
        let mut is_player_defeat = false;
        let my_id = self.sim.my_player_id.unwrap_or(0);

        if let Some(snap) = &self.sim.current_snapshot {
            if let Some(winner) = snap.winner {
                endgame_active = true;
                let my_team = snap
                    .players
                    .iter()
                    .find(|p| p.id == my_id)
                    .and_then(|p| p.team);
                let team_won = snap.winning_team.is_some_and(|team| my_team == Some(team));
                if team_won || (snap.winning_team.is_none() && winner == my_id) {
                    is_victory = true;
                    text_title = strings.victory_title.clone();
                    text_subtitle = if snap.winning_team.is_some() {
                        strings.team_victory_subtitle.clone()
                    } else {
                        strings.victory_subtitle.clone()
                    };
                } else {
                    is_victory = false;
                    text_title = strings.defeat_title.clone();
                    if let Some(winning_team) = snap.winning_team {
                        text_subtitle = strings
                            .team_winner_emerged
                            .replace("{}", team_label(winning_team));
                    } else {
                        let winner_name = snap
                            .players
                            .iter()
                            .find(|p| p.id == winner)
                            .map(|p| p.name.clone())
                            .unwrap_or_else(|| sow_i18n::get(lang).hud.default_player_name.clone());
                        text_subtitle = strings.winner_emerged.replace("{}", &winner_name);
                    }
                }
            } else if let Some(me) = snap.players.iter().find(|p| p.id == my_id) {
                if !me.alive && me.has_spawned && !self.ui.is_spectating {
                    endgame_active = true;
                    is_player_defeat = true;
                    is_victory = false;
                    text_title = strings.defeat_title.clone();
                    text_subtitle = strings.defeat_subtitle.clone();
                }
            }
        }

        if endgame_active {
            if self.ui.reward_cache.is_none() {
                if let Some(snap) = &self.sim.current_snapshot
                    && let Some(me) = snap.players.iter().find(|p| p.id == my_id)
                {
                    self.ui.reward_cache = Some(sow_data::rewards::calculate(
                        sow_data::rewards::RewardInput {
                            won: is_victory,
                            players_defeated: self.progress_session_defeats.players,
                            empires_defeated: self.progress_session_defeats.empires,
                            tribes_defeated: self.progress_session_defeats.tribes,
                            kills: me.kills,
                            assists: me.assists,
                            ..Default::default()
                        },
                    ));
                }
            }
            if self.ui.endgame_cache.is_none() {
                if is_victory {
                    crate::store_portals::happytime();
                    sow_audio::play_victory_sound();
                } else if is_player_defeat {
                    sow_audio::play_defeat_sound();
                }
            }
            self.ui.endgame_cache = Some((is_victory, text_title, text_subtitle));
        }

        let show_panel = endgame_active;

        let anim_dur = sow_ui_kit::theme::anim_duration_from_ctx(ctx);
        let anim = sow_ui::ui::animation::panel_in_out_anim(
            ctx,
            egui::Id::new("endgame_panel_animation"),
            show_panel,
            anim_dur,
            sow_ui::ui::animation::PANEL_Y_SLIDE,
            sow_ui::ui::animation::SlideDir::Down,
        );

        if anim.progress <= 0.01 {
            return;
        }

        let Some((is_victory, text_title, text_subtitle)) = self.ui.endgame_cache.clone() else {
            return;
        };

        let alpha = anim.progress;
        let y_offset = anim.offset;

        sow_ui_kit::theme::paint_scrim(ctx, "endgame_dimmer", alpha);

        let screen_width = ctx.content_rect().width();
        let is_mobile = sow_ui_kit::theme::compact_viewport(ctx);

        let (
            title_size,
            subtitle_size,
            space_top,
            space_mid,
            space_bot,
            btn_size,
            win_width,
            win_margin,
        ): (f32, f32, f32, f32, f32, egui::Vec2, f32, f32) = if is_mobile {
            (
                44.0,
                16.0,
                8.0,
                12.0,
                20.0,
                egui::vec2(180.0, 44.0),
                300.0,
                20.0,
            )
        } else {
            (
                64.0,
                22.0,
                10.0,
                15.0,
                30.0,
                egui::vec2(220.0, 50.0),
                400.0,
                30.0,
            )
        };

        // Team victories frame the panel in the team's color (when the player's
        // team won). Defeats keep the danger red.
        let victory_team_color: Option<[f32; 3]> =
            self.sim.current_snapshot.as_ref().and_then(|snap| {
                let my_team = snap
                    .players
                    .iter()
                    .find(|p| p.id == my_id)
                    .and_then(|p| p.team);
                if snap.winning_team.is_some_and(|wt| my_team == Some(wt)) {
                    my_team.map(sow_core::player::team_territory_rgb)
                } else {
                    None
                }
            });

        let border_color = if let Some(rgb) = victory_team_color {
            Color32::from_rgb(
                (rgb[0] * 255.0).round() as u8,
                (rgb[1] * 255.0).round() as u8,
                (rgb[2] * 255.0).round() as u8,
            )
            .linear_multiply(alpha)
        } else if is_victory {
            sow_ui_kit::theme::accent_ranked_gold().linear_multiply(alpha)
        } else {
            sow_ui_kit::theme::palette::danger().linear_multiply(alpha)
        };

        let mut win = egui::Window::new("Endgame")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_CENTER, [0.0, y_offset])
            .frame(egui::Frame::NONE);

        if is_mobile {
            let screen_height = ctx.content_rect().height();
            win = win.fixed_size(egui::vec2(screen_width, screen_height));
        }

        win.show(ctx, |ui| {
            let prepaint_idx = ui.painter().add(egui::Shape::Noop);
            let actual_width = if is_mobile {
                screen_width
            } else {
                win_width.min(screen_width - 32.0)
            };

            let frame_res = egui::Frame::NONE.inner_margin(win_margin).show(ui, |ui| {
                ui.set_min_width(actual_width - win_margin * 2.0);
                ui.set_max_width(actual_width - win_margin * 2.0);

                ui.vertical_centered(|ui| {
                    let title_color = if let Some(rgb) = victory_team_color {
                        Color32::from_rgb(
                            (rgb[0] * 255.0).round() as u8,
                            (rgb[1] * 255.0).round() as u8,
                            (rgb[2] * 255.0).round() as u8,
                        )
                        .linear_multiply(alpha)
                    } else if is_victory {
                        sow_ui_kit::theme::accent_ranked_gold().linear_multiply(alpha)
                    } else {
                        sow_ui_kit::theme::palette::danger().linear_multiply(alpha)
                    };

                    let title_font = FontId::proportional(title_size);
                    let galley = ui.painter().layout_no_wrap(
                        text_title.clone(),
                        title_font.clone(),
                        title_color,
                    );
                    let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
                    if ui.is_rect_visible(rect) {
                        sow_ui_kit::theme::outlined_text(
                            ui.painter(),
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &text_title,
                            title_font,
                            title_color,
                            Color32::BLACK.linear_multiply(alpha),
                        );
                    }

                    ui.add_space(space_top);
                    ui.label(
                        RichText::new(&text_subtitle)
                            .color(Color32::LIGHT_GRAY.linear_multiply(alpha))
                            .font(FontId::proportional(subtitle_size)),
                    );

                    if let Some(snap) = &self.sim.current_snapshot {
                        if let Some(me) = snap.players.iter().find(|p| p.id == my_id) {
                            if me.kills > 0 || me.deaths > 0 || me.assists > 0 {
                                ui.add_space(space_mid);
                                let kda_text = format!(
                                    "K / D / A:  {} / {} / {}",
                                    me.kills, me.deaths, me.assists
                                );
                                ui.label(
                                    RichText::new(kda_text)
                                        .color(Color32::WHITE.linear_multiply(alpha))
                                        .font(FontId::monospace(if is_mobile {
                                            14.0
                                        } else {
                                            18.0
                                        })),
                                );
                            }
                        }
                    }

                    if let Some(reward) = self.ui.reward_cache {
                        ui.add_space(space_mid);
                        ui.group(|ui| {
                            ui.set_min_width(actual_width - win_margin * 2.0 - 12.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new("MATCH REWARDS")
                                        .color(Color32::GRAY.linear_multiply(alpha))
                                        .font(FontId::monospace(if is_mobile { 12.0 } else { 14.0 })),
                                );
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        RichText::new(format!("+{} XP", reward.xp))
                                            .color(Color32::WHITE.linear_multiply(alpha))
                                            .font(FontId::monospace(if is_mobile { 14.0 } else { 17.0 })),
                                    );
                                    ui.separator();
                                    ui.label(
                                        RichText::new(format!(
                                            "+{} {} XP",
                                            reward.leader_xp,
                                            self.ui.app.main_menu_state.selected_leader.name()
                                        ))
                                        .color(Color32::from_rgb(255, 190, 80).linear_multiply(alpha))
                                        .font(FontId::monospace(if is_mobile { 14.0 } else { 17.0 })),
                                    );
                                    ui.separator();
                                    ui.label(
                                        RichText::new(format!("+{} Laurels", reward.laurels))
                                            .color(Color32::from_rgb(220, 180, 90).linear_multiply(alpha))
                                            .font(FontId::monospace(if is_mobile { 14.0 } else { 17.0 })),
                                    );
                                });
                                ui.label(
                                    RichText::new(format!("ACCOUNT LEVEL {}", self.progress.level))
                                        .color(Color32::LIGHT_GRAY.linear_multiply(alpha))
                                        .font(FontId::monospace(if is_mobile { 11.0 } else { 13.0 })),
                                );
                            });
                        });
                    }

                    ui.add_space(space_bot);

                    let btn_color = if is_victory {
                        Color32::from_rgb(40, 140, 40).linear_multiply(alpha)
                    } else {
                        Color32::from_rgb(140, 40, 40).linear_multiply(alpha)
                    };

                    let private_party = self.ui.app.main_menu_state.in_private_match;
                    let btn_label = if private_party {
                        &strings.play_again
                    } else {
                        &strings.return_to_lobby
                    };
                    let return_btn = sow_ui_kit::widgets::ThemeButton::new(btn_label)
                        .min_size(btn_size)
                        .text_size(if is_mobile { 16.0 } else { 20.0 })
                        .custom_fill(btn_color)
                        .custom_text_color(Color32::WHITE.linear_multiply(alpha));

                    if ui.add(return_btn).clicked() {
                        if private_party {
                            let req = sow_core::protocol::ClientMessage::RematchRequest {
                                lobby_id: self.sim.my_lobby_id.unwrap_or(0),
                            };
                            if let Ok(json) = bincode::serialize(&req) {
                                if let Some(c) = self.net.client.as_ref() {
                                    c.send(json);
                                }
                            }
                        } else {
                            self.net.client = None;
                            self.begin_exit_to_main_menu(true);
                        }
                    }

                    if !is_victory
                        && self
                            .sim
                            .current_snapshot
                            .as_ref()
                            .is_some_and(|s| s.winner.is_none())
                    {
                        ui.add_space(space_mid);

                        let spectate_btn = sow_ui_kit::widgets::ThemeButton::new(&strings.spectate)
                            .min_size(btn_size)
                            .text_size(if is_mobile { 16.0 } else { 20.0 })
                            .custom_fill(Color32::from_rgb(60, 60, 60).linear_multiply(alpha))
                            .custom_text_color(Color32::WHITE.linear_multiply(alpha));

                        if ui.add(spectate_btn).clicked() {
                            self.ui.is_spectating = true;
                        }
                    }
                });
            });

            sow_ui_kit::theme::paint_hud_panel_gradient(
                ui,
                prepaint_idx,
                frame_res.response.rect,
                border_color,
                if is_mobile {
                    egui::CornerRadius::ZERO
                } else {
                    sow_ui_kit::theme::radius::lg()
                },
            );
        });
    }
}

fn team_label(team: Team) -> &'static str {
    match team {
        Team::Red => "Red",
        Team::Blue => "Blue",
    }
}
