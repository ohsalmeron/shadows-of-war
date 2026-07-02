use super::{LobbyNotice, MainMenuState, UiLinkConflictInfo};
use crate::UiAction;
use egui::{Color32, Frame, RichText, Stroke};

fn draw_indicator_toast(
    ctx: &egui::Context,
    id: &str,
    pad_y: f32,
    label: &str,
    color: Color32,
    compact: bool,
) {
    let pad_x = if compact { 24.0 } else { 20.0 };
    egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-pad_x, pad_y))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(Color32::from_black_alpha(140))
                .corner_radius(8)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(egui::RichText::new(label).color(color).size(if compact {
                            13.0
                        } else {
                            14.0
                        }));
                    });
                });
        });
}

pub(crate) fn draw_map_download_indicator(
    ctx: &egui::Context,
    state: &MainMenuState,
    lang: sow_i18n::Language,
    compact: bool,
) {
    if !state.is_downloading_map {
        return;
    }
    let map_name = state.downloading_map_name.as_deref().unwrap_or("map");
    let strings = &sow_i18n::get(lang).main_menu;
    let label = strings
        .downloading_map
        .replacen("{}", map_name, 1)
        .replacen("{}", &state.map_download_progress.to_string(), 1);
    let pad_y = if compact { 96.0 } else { 56.0 };
    draw_indicator_toast(
        ctx,
        "main_menu_map_download",
        pad_y,
        &label,
        sow_ui_kit::theme::palette::neon_cyan(),
        compact,
    );
}

pub(crate) fn draw_connecting_indicator(
    ctx: &egui::Context,
    state: &MainMenuState,
    lang: sow_i18n::Language,
    compact: bool,
) {
    if state.is_connected {
        return;
    }
    let strings = &sow_i18n::get(lang).main_menu;
    let pad_y = if compact { 56.0 } else { 20.0 };
    draw_indicator_toast(
        ctx,
        "main_menu_connecting",
        pad_y,
        &strings.connecting,
        sow_ui_kit::theme::palette::text_muted(),
        compact,
    );
}

pub(crate) fn draw_link_conflict_modal(
    root_ui: &mut egui::Ui,
    action: &mut Option<UiAction>,
    conflict: &UiLinkConflictInfo,
    lang: sow_i18n::Language,
    compact: bool,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let screen_rect = root_ui.ctx().content_rect();
    let modal_w = if compact {
        screen_rect.width() - 32.0
    } else {
        480.0
    };

    egui::Area::new(egui::Id::new("link_conflict_backdrop"))
        .order(egui::Order::Background)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(root_ui.ctx(), |ui| {
            ui.painter()
                .rect_filled(screen_rect, 0.0, sow_ui_kit::theme::palette::backdrop());
        });

    egui::Window::new(&strings.link_conflict_title)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .fixed_size(egui::vec2(
            modal_w,
            (if compact { 360.0_f32 } else { 320.0_f32 })
                .min(screen_rect.height() - 32.0)
                .max(200.0),
        ))
        .frame(
            Frame::new()
                .fill(sow_ui_kit::theme::palette::surface())
                .stroke(Stroke::new(
                    1.5_f32,
                    sow_ui_kit::theme::palette::neon_cyan_hover(),
                ))
                .corner_radius(egui::CornerRadius::same(16))
                .inner_margin(24.0)
                .shadow(egui::Shadow {
                    blur: 32,
                    spread: 0,
                    color: sow_ui_kit::theme::palette::neon_cyan().linear_multiply(0.2),
                    offset: [0, 8],
                }),
        )
        .show(root_ui.ctx(), |ui| {
            ui.set_width(modal_w - 48.0);
            ui.vertical_centered(|ui| {
                sow_ui_kit::theme::outlined_label(
                    ui,
                    &strings.link_conflict_title,
                    egui::FontId::proportional(20.0),
                    sow_ui_kit::theme::palette::neon_cyan(),
                );
                ui.add_space(12.0);
                ui.label(
                    RichText::new(&strings.link_conflict_body)
                        .size(13.0)
                        .color(sow_ui_kit::theme::palette::text_muted()),
                );
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    let col_w = (ui.available_width() - 12.0) / 2.0;
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        ui.label(
                            RichText::new(&strings.link_conflict_guest_label)
                                .strong()
                                .color(sow_ui_kit::theme::palette::neon_cyan()),
                        );
                        ui.label(
                            RichText::new(
                                strings
                                    .link_conflict_level
                                    .replace("{}", &conflict.current_level.to_string()),
                            )
                            .size(16.0),
                        );
                    });
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        ui.label(
                            RichText::new(&strings.link_conflict_platform_label)
                                .strong()
                                .color(sow_ui_kit::theme::palette::neon_cyan()),
                        );
                        ui.label(
                            RichText::new(
                                strings
                                    .link_conflict_level
                                    .replace("{}", &conflict.existing_level.to_string()),
                            )
                            .size(16.0),
                        );
                    });
                });
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    let btn_w = if compact {
                        ui.available_width()
                    } else {
                        (ui.available_width() - 12.0) / 2.0
                    };
                    let guest_btn =
                        crate::widgets::ThemeButton::new(&strings.link_conflict_keep_guest)
                            .style(crate::widgets::ThemeButtonStyle::Secondary)
                            .min_size(egui::vec2(btn_w, 40.0));
                    if ui.add(guest_btn).clicked() {
                        *action = Some(UiAction::ResolveLinkConflict {
                            keep_account_id: conflict.current_account_id.clone(),
                        });
                    }
                    if !compact {
                        ui.add_space(12.0);
                    } else {
                        ui.add_space(8.0);
                    }
                    let platform_btn =
                        crate::widgets::ThemeButton::new(&strings.link_conflict_keep_platform)
                            .style(crate::widgets::ThemeButtonStyle::Primary)
                            .min_size(egui::vec2(btn_w, 40.0));
                    if ui.add(platform_btn).clicked() {
                        *action = Some(UiAction::ResolveLinkConflict {
                            keep_account_id: conflict.existing_account_id.clone(),
                        });
                    }
                });
            });
        });
}

/// Brief modal shown when the server removed this player from a lobby (host left,
/// kicked, or banned). Auto-dismisses on a timer in `draw`; the bottom close button
/// (same as our standard modals) lets the player dismiss it early. Returns `true`
/// when the close button was clicked.
pub(crate) fn draw_lobby_notice(
    root_ui: &mut egui::Ui,
    notice: LobbyNotice,
    strings: &sow_i18n::MainMenuStrings,
    compact: bool,
) -> bool {
    let (title, body, accent) = match notice {
        LobbyNotice::HostLeft => (
            &strings.notice_host_left_title,
            &strings.notice_host_left_body,
            sow_ui_kit::theme::palette::neon_cyan(),
        ),
        LobbyNotice::Kicked => (
            &strings.notice_kicked_title,
            &strings.notice_kicked_body,
            sow_ui_kit::theme::palette::neon_gold(),
        ),
        LobbyNotice::Banned => (
            &strings.notice_banned_title,
            &strings.notice_banned_body,
            sow_ui_kit::theme::palette::danger(),
        ),
    };
    let mut dismissed = false;

    egui::Area::new(egui::Id::new("lobby_notice_backdrop"))
        .order(egui::Order::Background)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(root_ui.ctx(), |ui| {
            let screen_rect = ui.ctx().content_rect();
            ui.painter()
                .rect_filled(screen_rect, 0.0, sow_ui_kit::theme::palette::backdrop());
        });

    let screen_rect = root_ui.ctx().content_rect();
    let modal_w = if compact {
        screen_rect.width() - 32.0
    } else {
        380.0
    };

    egui::Window::new("lobby_notice")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .fixed_size(egui::vec2(
            modal_w,
            220.0_f32.min(screen_rect.height() - 32.0).max(150.0),
        ))
        .frame(
            egui::Frame::new()
                .fill(sow_ui_kit::theme::palette::surface())
                .stroke(egui::Stroke::new(1.5_f32, accent))
                .corner_radius(egui::CornerRadius::same(16))
                .inner_margin(24.0)
                .shadow(egui::Shadow {
                    blur: 32,
                    spread: 0,
                    color: accent.linear_multiply(0.2),
                    offset: [0, 8],
                }),
        )
        .show(root_ui.ctx(), |ui| {
            ui.set_width(modal_w - 48.0);
            ui.vertical_centered(|ui| {
                ui.add_space(4.0);
                sow_ui_kit::theme::outlined_label(
                    ui,
                    title,
                    egui::FontId::proportional(22.0),
                    accent,
                );
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new(body)
                        .size(14.0)
                        .color(sow_ui_kit::theme::palette::text_muted()),
                );
                ui.add_space(24.0);
                let btn_w = if compact { ui.available_width() } else { 160.0 };
                let close_btn = crate::widgets::ThemeButton::new(&strings.dismiss)
                    .style(crate::widgets::ThemeButtonStyle::Primary)
                    .min_size(egui::vec2(btn_w, 40.0));
                if ui.add(close_btn).clicked() {
                    dismissed = true;
                }
                ui.add_space(4.0);
            });
        });

    dismissed
}

pub(crate) fn draw_connection_error_modal(
    root_ui: &mut egui::Ui,
    state: &mut MainMenuState,
    action: &mut Option<UiAction>,
    err_msg: &str,
    strings: &sow_i18n::MainMenuStrings,
    compact: bool,
) {
    let mut clear_error = false;
    let mut retry = false;

    egui::Area::new(egui::Id::new("error_modal_backdrop"))
        .order(egui::Order::Background)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(root_ui.ctx(), |ui| {
            let screen_rect = ui.ctx().content_rect();
            ui.painter()
                .rect_filled(screen_rect, 0.0, sow_ui_kit::theme::palette::backdrop());
        });

    let screen_rect = root_ui.ctx().content_rect();
    let is_mobile = compact;
    let modal_w = if is_mobile {
        screen_rect.width() - 32.0
    } else {
        420.0
    };

    egui::Window::new(&strings.connection_error_title)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .fixed_size(egui::vec2(
            modal_w,
            280.0_f32.min(screen_rect.height() - 32.0).max(200.0),
        ))
        .frame(
            egui::Frame::new()
                .fill(sow_ui_kit::theme::palette::surface())
                .stroke(egui::Stroke::new(
                    1.5_f32,
                    sow_ui_kit::theme::palette::danger_border(),
                ))
                .corner_radius(egui::CornerRadius::same(16))
                .inner_margin(24.0)
                .shadow(egui::Shadow {
                    blur: 32,
                    spread: 0,
                    color: sow_ui_kit::theme::palette::danger().linear_multiply(0.2),
                    offset: [0, 8],
                }),
        )
        .show(root_ui.ctx(), |ui| {
            ui.set_width(modal_w - 48.0);
            ui.vertical_centered(|ui| {
                ui.add_space(4.0);

                sow_ui_kit::theme::outlined_label(
                    ui,
                    "⚠",
                    egui::FontId::proportional(36.0),
                    sow_ui_kit::theme::palette::danger(),
                );

                ui.add_space(12.0);

                sow_ui_kit::theme::outlined_label(
                    ui,
                    &strings.connection_error_header,
                    egui::FontId::proportional(22.0),
                    sow_ui_kit::theme::palette::danger(),
                );

                ui.add_space(16.0);

                ui.label(
                    egui::RichText::new(err_msg)
                        .size(14.0)
                        .color(sow_ui_kit::theme::palette::text_muted()),
                );

                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(&strings.connection_error_hint)
                        .size(12.0)
                        .color(sow_ui_kit::theme::palette::text_muted()),
                );

                ui.add_space(24.0);

                ui.horizontal(|ui| {
                    let btn_w = if is_mobile {
                        (ui.available_width() - 12.0) / 2.0
                    } else {
                        140.0
                    };
                    let retry_btn = crate::widgets::ThemeButton::new(&strings.connection_retry)
                        .style(crate::widgets::ThemeButtonStyle::Primary)
                        .min_size(egui::vec2(btn_w, 40.0));
                    if ui.add(retry_btn).clicked() {
                        retry = true;
                    }
                    ui.add_space(12.0);
                    let dismiss_btn = crate::widgets::ThemeButton::new(&strings.dismiss)
                        .style(crate::widgets::ThemeButtonStyle::Danger)
                        .min_size(egui::vec2(btn_w, 40.0));
                    if ui.add(dismiss_btn).clicked() {
                        clear_error = true;
                    }
                });

                ui.add_space(4.0);
            });
        });

    if retry {
        state.error_message = None;
        *action = Some(UiAction::RetryConnection);
    } else if clear_error {
        state.error_message = None;
    }
}
