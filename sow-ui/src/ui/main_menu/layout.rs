use super::{actions, browser, profile, MainMenuState};
use crate::UiAction;
use egui::{Align, Layout};

pub(crate) fn menu_footer_height(section_gap: f32, action_min_h: f32, scale: f32) -> f32 {
    let settings_h = action_min_h * 0.75;
    action_min_h // Solo button
        + section_gap
        + action_min_h // Create button
        + section_gap
        + action_min_h // Join button
        + section_gap
        + settings_h // Settings button
        + 6.0 * scale
}

pub(crate) fn menu_layout_chrome(
    ctx: &egui::Context,
    panel_h: f32,
    available_w: f32,
    compact: bool,
) -> (f32, f32, f32, f32) {
    let scale = sow_ui_kit::theme::viewport_scale(ctx);
    let portrait = sow_ui_kit::theme::portrait_layout(ctx);
    let mut section_gap = (if portrait {
        8.0
    } else if compact {
        12.0
    } else {
        16.0
    }) * scale;
    let mut action_min_h = (if portrait {
        54.0
    } else if compact {
        64.0
    } else {
        72.0
    }) * scale;
    let mut profile_height = 56.0 * scale;
    if portrait {
        profile_height *= 0.85;
    }

    let mut lobby_h = crate::ui::map_texture::thumbnail_square_side(available_w, compact);
    if portrait {
        lobby_h = (lobby_h * 0.55).clamp(110.0, 160.0);
    }

    let footer_h = menu_footer_height(section_gap, action_min_h, scale);
    let header_h = profile_height + section_gap;
    let needed = header_h + section_gap + footer_h + section_gap + lobby_h;
    let shrink = sow_ui_kit::theme::fit_scale(needed, panel_h);
    if shrink < 1.0 {
        section_gap *= shrink;
        action_min_h *= shrink;
        profile_height *= shrink;
        lobby_h *= shrink;
    }
    (section_gap, action_min_h, profile_height, lobby_h)
}

#[allow(clippy::too_many_arguments)]
fn draw_menu_footer(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    compact: bool,
    action: &mut Option<UiAction>,
    lang: sow_i18n::Language,
) {
    actions::draw_right_column(ui, state, section_gap, action_min_h, compact, action, lang);
}

#[allow(clippy::too_many_arguments)]
fn draw_menu_right_panel_contents(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    compact: bool,
    profile_height: f32,
    lobby_height: f32,
    panel_inner_h: f32,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
) {
    let portrait = !compact || sow_ui_kit::theme::portrait_layout(ui.ctx());
    let landscape_compact = compact && !portrait;
    let panel_w = ui.available_width();
    let header_h = profile_height + section_gap;

    let panel_rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(panel_w, panel_inner_h));

    ui.scope_builder(egui::UiBuilder::new().max_rect(panel_rect), |ui| {
        profile::draw_user_profile_header(
            ui,
            state,
            compact,
            profile_height,
            asset_loader,
            lang,
            action,
        );
        ui.add_space(section_gap);

        if landscape_compact {
            let row_h = (panel_inner_h - header_h).max(0.0);
            let action_col_w = (panel_w * 0.38).clamp(120.0, 160.0);
            let lobby_w = (panel_w - action_col_w - section_gap).max(80.0);

            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(lobby_w, row_h),
                    Layout::top_down(Align::Min),
                    |ui| {
                        browser::draw_left_column(
                            ui,
                            state,
                            section_gap,
                            action_min_h,
                            compact,
                            row_h,
                            action,
                            asset_loader,
                            lang,
                        );
                    },
                );
                ui.add_space(section_gap);
                ui.allocate_ui_with_layout(
                    egui::vec2(action_col_w, row_h),
                    Layout::top_down(Align::Min),
                    |ui| {
                        draw_menu_footer(
                            ui,
                            state,
                            section_gap,
                            action_min_h,
                            compact,
                            action,
                            lang,
                        );
                    },
                );
            });
        } else {
            browser::draw_left_column(
                ui,
                state,
                section_gap,
                action_min_h,
                compact,
                lobby_height,
                action,
                asset_loader,
                lang,
            );

            ui.add_space(section_gap);

            draw_menu_footer(ui, state, section_gap, action_min_h, compact, action, lang);
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_menu_right_panel(
    ui: &mut egui::Ui,
    state: &mut MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    compact: bool,
    profile_height: f32,
    lobby_height: f32,
    panel_outer_h: f32,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
) {
    let panel_w = ui.available_width();

    ui.allocate_ui_with_layout(
        egui::vec2(panel_w, panel_outer_h),
        Layout::top_down(Align::Min),
        |ui| {
            if compact {
                draw_menu_right_panel_contents(
                    ui,
                    state,
                    section_gap,
                    action_min_h,
                    compact,
                    profile_height,
                    lobby_height,
                    panel_outer_h,
                    action,
                    asset_loader,
                    lang,
                );
            } else {
                sow_ui_kit::theme::menu_right_panel_frame(false).show(ui, |ui| {
                    let h = ui.available_height();
                    ui.set_min_height(h);
                    draw_menu_right_panel_contents(
                        ui,
                        state,
                        section_gap,
                        action_min_h,
                        compact,
                        profile_height,
                        lobby_height,
                        h,
                        action,
                        asset_loader,
                        lang,
                    );
                });
            }
        },
    );
}
