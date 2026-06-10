use crate::app::SowApp;
use egui::{Align2, Color32, FontId, RichText};

const DEFEAT_PANEL_DELAY: web_time::Duration = web_time::Duration::from_millis(2500);

impl SowApp {
    #[allow(deprecated)]
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
                if winner == my_id {
                    is_victory = true;
                    text_title = strings.victory_title.clone();
                    text_subtitle = strings.victory_subtitle.clone();
                } else {
                    is_victory = false;
                    text_title = strings.defeat_title.clone();
                    let winner_name = snap
                        .players
                        .iter()
                        .find(|p| p.id == winner)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| sow_i18n::get(lang).hud.default_player_name.clone());
                    text_subtitle = strings.winner_emerged.replace("{}", &winner_name);
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

        if is_player_defeat {
            if self.ui.defeat_time.is_none() {
                self.ui.defeat_time = Some(web_time::Instant::now());
            }
        } else {
            self.ui.defeat_time = None;
        }

        if endgame_active {
            self.ui.endgame_cache = Some((is_victory, text_title, text_subtitle));
        }

        let now = web_time::Instant::now();
        let defeat_delay_elapsed = self
            .ui
            .defeat_time
            .is_some_and(|t| now.duration_since(t) >= DEFEAT_PANEL_DELAY);

        let show_panel = if is_player_defeat {
            defeat_delay_elapsed
        } else {
            endgame_active
        };

        if is_player_defeat && !defeat_delay_elapsed {
            ctx.request_repaint();
        }

        let anim_dur = sow_ui::ui::theme::anim_duration_from_ctx(ctx);
        let anim = sow_ui::ui::animation::panel_in_out_anim(
            ctx,
            egui::Id::new("endgame_panel_animation"),
            show_panel,
            anim_dur,
        );

        if anim.progress <= 0.01 {
            return;
        }

        let Some((is_victory, text_title, text_subtitle)) = self.ui.endgame_cache.clone() else {
            return;
        };

        let alpha = anim.progress;
        let y_offset = anim.y_offset;

        egui::Area::new(egui::Id::new("endgame_dimmer"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let rect = ctx.content_rect();
                ui.painter().rect_filled(
                    rect,
                    0.0,
                    Color32::from_black_alpha((180.0 * alpha) as u8),
                );
            });

        let screen_width = ctx.content_rect().width();
        let is_mobile = sow_ui::ui::theme::compact_viewport(ctx);

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

        let border_color = if is_victory {
            sow_ui::ui::theme::accent_ranked_gold().linear_multiply(alpha)
        } else {
            sow_ui::ui::theme::palette::danger().linear_multiply(alpha)
        };

        egui::Window::new("Endgame")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_CENTER, [0.0, y_offset])
            .frame(
                egui::Frame::window(&ctx.global_style())
                    .fill(sow_ui::ui::theme::panel_bg().linear_multiply(alpha))
                    .stroke(egui::Stroke::new(2.0_f32 * anim.scale, border_color))
                    .corner_radius(12.0)
                    .inner_margin(win_margin),
            )
            .show(ctx, |ui| {
                let actual_width = win_width.min(screen_width - 32.0);
                ui.set_min_width(actual_width);
                ui.set_max_width(actual_width);

                ui.vertical_centered(|ui| {
                    let title_color = if is_victory {
                        sow_ui::ui::theme::accent_ranked_gold().linear_multiply(alpha)
                    } else {
                        sow_ui::ui::theme::palette::danger().linear_multiply(alpha)
                    };

                    let title_font = FontId::proportional(title_size);
                    let galley = ui.painter().layout_no_wrap(
                        text_title.clone(),
                        title_font.clone(),
                        title_color,
                    );
                    let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
                    if ui.is_rect_visible(rect) {
                        sow_ui::ui::theme::outlined_text(
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
                    let return_btn = sow_ui::widgets::ThemeButton::new(btn_label)
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

                        let spectate_btn = sow_ui::widgets::ThemeButton::new(&strings.spectate)
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
    }
}
