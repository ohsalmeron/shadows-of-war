use super::{GameModeFilter, MainMenuState};
use crate::widgets::LobbyCard;
use crate::UiAction;
use egui::{Color32, CornerRadius, Frame, Margin, RichText, ScrollArea, Stroke};

fn filter_pill(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
    let (bg, text_col) = if active {
        (crate::ui::theme::palette::neon_cyan(), Color32::BLACK)
    } else {
        (
            crate::ui::theme::palette::button_inactive(),
            crate::ui::theme::palette::text_muted(),
        )
    };
    let stroke_col = if active {
        crate::ui::theme::palette::neon_cyan()
    } else {
        crate::ui::theme::palette::field_border()
    };
    let btn = egui::Button::new(RichText::new(label).size(13.0).color(text_col).strong())
        .fill(bg)
        .stroke(Stroke::new(1.0_f32, stroke_col))
        .corner_radius(CornerRadius::same(6));
    ui.add(btn).clicked()
}

fn section_header(ui: &mut egui::Ui, label: &str) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .size(12.0)
                .strong()
                .color(crate::ui::theme::palette::text_muted()),
        );
        let available = ui.available_width();
        if available > 10.0 {
            ui.add_space(8.0);
            ui.scope(|ui| {
                ui.visuals_mut().widgets.noninteractive.bg_stroke =
                    Stroke::new(1.0_f32, crate::ui::theme::palette::field_border());
                ui.separator();
            });
        }
    });
    ui.add_space(4.0);
}

pub fn draw(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    action: &mut Option<UiAction>,
    lang: sow_i18n::Language,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let compact = crate::ui::theme::compact_viewport(root_ui.ctx());

    // ── Top panel: title + mode filter pills ──────────────────────────────
    egui::Panel::top("join_browser_header")
        .frame(crate::ui::theme::screen_panel_frame())
        .show_inside(root_ui, |ui| {
            if compact {
                ui.vertical(|ui| {
                    ui.heading(&strings.game_browser_title);
                    ui.add_space(6.0);
                    draw_filter_pills(ui, state, strings);
                });
            } else {
                ui.horizontal(|ui| {
                    ui.heading(&strings.game_browser_title);
                    ui.add_space(16.0);
                    draw_filter_pills(ui, state, strings);
                });
            }
        });

    // ── Bottom panel: private code entry + back ───────────────────────────
    egui::Panel::bottom("join_browser_footer")
        .frame(crate::ui::theme::screen_panel_frame())
        .show_inside(root_ui, |ui| {
            if compact {
                ui.vertical(|ui| {
                    draw_private_join_row(ui, state, strings, action);
                    ui.add_space(6.0);
                    let back_btn = crate::widgets::ThemeButton::new(&strings.back)
                        .style(crate::widgets::ThemeButtonStyle::Tertiary)
                        .min_size(egui::vec2(ui.available_width(), 36.0));
                    if ui.add(back_btn).clicked() {
                        *action = Some(UiAction::CloseOverlay);
                    }
                });
            } else {
                ui.horizontal(|ui| {
                    let back_btn = crate::widgets::ThemeButton::new(&strings.back)
                        .style(crate::widgets::ThemeButtonStyle::Tertiary)
                        .min_size(egui::vec2(120.0, 36.0));
                    if ui.add(back_btn).clicked() {
                        *action = Some(UiAction::CloseOverlay);
                    }
                    ui.add_space(16.0);
                    draw_private_join_row(ui, state, strings, action);
                });
            }
        });

    // ── Central: scrollable lobby cards ──────────────────────────────────
    egui::CentralPanel::default()
        .frame(
            Frame::new()
                .fill(Color32::from_rgb(8, 10, 14))
                .inner_margin(egui::Margin::symmetric(16, 12)),
        )
        .show_inside(root_ui, |ui| {
            let side = crate::ui::map_texture::thumbnail_square_side_bounded(
                ui.available_width(),
                ui.available_height(),
                compact,
            );

            let filter = state.join_mode_filter;
            // Clone lobbies first to avoid borrowing state immutably and mutably at the same time inside the closure.
            // Game Browser shows ONLY Custom lobbies — Matchmaking queues live in the main menu.
            let lobbies = state.lobbies.clone();
            let ffa_lobbies: Vec<sow_core::protocol::LobbyInfo> = lobbies
                .iter()
                .filter(|l| {
                    l.kind == sow_core::protocol::LobbyKind::Custom
                        && matches!(filter, GameModeFilter::All | GameModeFilter::Ffa)
                        && l.game_mode == "FFA"
                })
                .cloned()
                .collect();
            let team_lobbies: Vec<sow_core::protocol::LobbyInfo> = lobbies
                .iter()
                .filter(|l| {
                    l.kind == sow_core::protocol::LobbyKind::Custom
                        && matches!(filter, GameModeFilter::All | GameModeFilter::Teams)
                        && l.game_mode == "Teams"
                })
                .cloned()
                .collect();

            let any = !ffa_lobbies.is_empty() || !team_lobbies.is_empty();

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if !any {
                        let center_y = ui.available_height() * 0.35;
                        ui.add_space(center_y);
                        ui.vertical_centered(|ui| {
                            crate::ui::theme::outlined_label(
                                ui,
                                &strings.no_public_games,
                                egui::FontId::proportional(16.0),
                                crate::ui::theme::palette::text_muted(),
                            );
                        });
                        return;
                    }

                    if !ffa_lobbies.is_empty() {
                        section_header(ui, "FFA");
                        ui.spacing_mut().item_spacing.y = 8.0;
                        for lobby in &ffa_lobbies {
                            draw_lobby_row(ui, lobby, side, asset_loader, state, action, strings);
                        }
                    }

                    if !team_lobbies.is_empty() {
                        ui.add_space(8.0);
                        section_header(ui, "TEAMS");
                        ui.spacing_mut().item_spacing.y = 8.0;
                        for lobby in &team_lobbies {
                            draw_lobby_row(ui, lobby, side, asset_loader, state, action, strings);
                        }
                    }
                });
        });

    // ── Password prompt modal ─────────────────────────────────────────────
    if let Some(target_id) = state.join_password_for_lobby {
        draw_password_modal(root_ui, state, target_id, action, strings, compact);
    }
}

fn draw_filter_pills(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    strings: &sow_i18n::MainMenuStrings,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if filter_pill(
            ui,
            &strings.filter_all,
            state.join_mode_filter == GameModeFilter::All,
        ) {
            state.join_mode_filter = GameModeFilter::All;
        }
        if filter_pill(ui, "FFA", state.join_mode_filter == GameModeFilter::Ffa) {
            state.join_mode_filter = GameModeFilter::Ffa;
        }
        if filter_pill(ui, "TEAMS", state.join_mode_filter == GameModeFilter::Teams) {
            state.join_mode_filter = GameModeFilter::Teams;
        }
    });
}

fn draw_private_join_row(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    strings: &sow_i18n::MainMenuStrings,
    action: &mut Option<UiAction>,
) {
    ui.horizontal(|ui| {
        // Code input
        let field_frame = Frame::NONE
            .fill(crate::ui::theme::palette::field_bg())
            .stroke(Stroke::new(
                1.0_f32,
                crate::ui::theme::palette::field_border(),
            ))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 4));
        field_frame.show(ui, |ui| {
            ui.set_width(160.0);
            ui.add(
                egui::TextEdit::singleline(&mut state.join_lobby_code)
                    .hint_text(&strings.lobby_code_hint)
                    .desired_width(150.0)
                    .frame(Frame::NONE)
                    .font(egui::FontId::proportional(14.0))
                    .text_color(Color32::WHITE),
            );
        });
        ui.add_space(6.0);
        let join_btn = crate::widgets::ThemeButton::new(&strings.join_private_btn)
            .style(crate::widgets::ThemeButtonStyle::Secondary)
            .min_size(egui::vec2(140.0, 36.0));
        if ui.add(join_btn).clicked() {
            *action = Some(UiAction::JoinWithCode);
        }
    });
}

fn draw_lobby_row(
    ui: &mut egui::Ui,
    lobby: &sow_core::protocol::LobbyInfo,
    side: f32,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    state: &mut MainMenuState,
    action: &mut Option<UiAction>,
    strings: &sow_i18n::MainMenuStrings,
) {
    let thumbnail = asset_loader.thumbnail(&lobby.map_name);
    let response = ui.add(LobbyCard::new(lobby, thumbnail).side(side));

    // Draw 🔒 badge overlay if password-protected
    if lobby.has_password {
        let painter = ui.painter();
        let card_rect = response.rect;
        let lock_pos = egui::pos2(card_rect.min.x + 8.0, card_rect.max.y - 38.0);
        let lock_galley = painter.layout_no_wrap(
            "🔒".to_string(),
            egui::FontId::proportional(16.0),
            Color32::WHITE,
        );
        let lock_size = lock_galley.size() + egui::vec2(8.0, 4.0);
        let lock_rect = egui::Rect::from_min_size(lock_pos, lock_size);
        painter.rect_filled(lock_rect, 4.0, Color32::from_black_alpha(180));
        painter.galley(
            egui::pos2(lock_pos.x + 4.0, lock_pos.y + 2.0),
            lock_galley,
            Color32::WHITE,
        );
    }

    // Host name badge (top center area, below mode badge)
    if !lobby.host_name.is_empty() {
        let painter = ui.painter();
        let card_rect = response.rect;
        let host_text = format!("HOST: {}", lobby.host_name.to_uppercase());
        let host_galley = painter.layout_no_wrap(
            host_text.clone(),
            crate::ui::theme::font_regular(11.0),
            crate::ui::theme::palette::text_muted(),
        );
        let host_w = host_galley.size().x + 10.0;
        let host_h = host_galley.size().y + 4.0;
        let host_rect = egui::Rect::from_min_size(
            egui::pos2(card_rect.min.x + 8.0, card_rect.min.y + 32.0),
            egui::vec2(host_w, host_h),
        );
        painter.rect_filled(host_rect, 3.0, Color32::from_black_alpha(160));
        painter.galley(
            egui::pos2(host_rect.min.x + 5.0, host_rect.min.y + 2.0),
            host_galley,
            crate::ui::theme::palette::text_muted(),
        );
    }

    if response.clicked() {
        if lobby.has_password {
            // Prompt for password before joining
            state.join_password_for_lobby = Some(lobby.id);
            state.join_password_input.clear();
        } else {
            *action = Some(UiAction::JoinLobby(lobby.id));
        }
    }

    // Missing thumbnail status
    if thumbnail.is_none() && asset_loader.thumbnail_in_flight(&lobby.map_name) {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(
                RichText::new(&strings.loading_thumbnail)
                    .size(11.0)
                    .color(crate::ui::theme::palette::text_muted()),
            );
        });
    }
}

fn draw_password_modal(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    target_id: u64,
    action: &mut Option<UiAction>,
    strings: &sow_i18n::MainMenuStrings,
    compact: bool,
) {
    let screen_rect = root_ui.ctx().content_rect();
    let modal_w = if compact {
        screen_rect.width() - 48.0
    } else {
        360.0
    };

    // Backdrop
    egui::Area::new(egui::Id::new("pw_modal_backdrop"))
        .order(egui::Order::Background)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(root_ui.ctx(), |ui| {
            ui.painter()
                .rect_filled(screen_rect, 0.0, crate::ui::theme::palette::backdrop());
        });

    let mut close = false;
    egui::Window::new("pw_modal")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .fixed_size(egui::vec2(modal_w, 180.0))
        .frame(
            Frame::new()
                .fill(crate::ui::theme::palette::surface())
                .stroke(Stroke::new(
                    1.5_f32,
                    crate::ui::theme::palette::neon_cyan_hover(),
                ))
                .corner_radius(CornerRadius::same(16))
                .inner_margin(24.0)
                .shadow(egui::Shadow {
                    blur: 32,
                    spread: 0,
                    color: crate::ui::theme::palette::neon_cyan().linear_multiply(0.2),
                    offset: [0, 8],
                }),
        )
        .show(root_ui.ctx(), |ui| {
            ui.set_width(modal_w - 48.0);
            ui.vertical_centered(|ui| {
                crate::ui::theme::outlined_label(
                    ui,
                    &strings.password_label,
                    egui::FontId::proportional(18.0),
                    crate::ui::theme::palette::neon_cyan(),
                );
                ui.add_space(12.0);

                // Password field
                let field_frame = Frame::NONE
                    .fill(crate::ui::theme::palette::field_bg())
                    .stroke(Stroke::new(
                        1.0_f32,
                        crate::ui::theme::palette::field_border(),
                    ))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(8, 6));
                field_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut state.join_password_input)
                            .hint_text(&strings.password_hint)
                            .password(true)
                            .desired_width(f32::INFINITY)
                            .frame(Frame::NONE)
                            .font(egui::FontId::proportional(15.0))
                            .text_color(Color32::WHITE),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        *action = Some(UiAction::JoinWithPassword(target_id));
                        close = true;
                    }
                });

                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    let half = (ui.available_width() - 8.0) / 2.0;
                    let cancel_btn = crate::widgets::ThemeButton::new(&strings.back)
                        .style(crate::widgets::ThemeButtonStyle::Tertiary)
                        .min_size(egui::vec2(half, 36.0));
                    if ui.add(cancel_btn).clicked() {
                        close = true;
                    }
                    ui.add_space(8.0);
                    let join_btn = crate::widgets::ThemeButton::new(&strings.join_private_btn)
                        .style(crate::widgets::ThemeButtonStyle::Primary)
                        .min_size(egui::vec2(half, 36.0));
                    if ui.add(join_btn).clicked() {
                        *action = Some(UiAction::JoinWithPassword(target_id));
                        close = true;
                    }
                });
            });
        });

    if close {
        state.join_password_for_lobby = None;
        state.join_password_input.clear();
    }
}
