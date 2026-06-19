use super::MainMenuState;
use crate::widgets::LobbyCard;
use crate::UiAction;
use egui::Ui;

#[allow(clippy::too_many_arguments)]
fn draw_lobby_list(
    ui: &mut Ui,
    state: &mut MainMenuState,
    side: f32,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
) {
    let strings = &sow_i18n::get(lang).main_menu;

    if state.lobbies.is_empty() {
        let label = if state.is_connected {
            &strings.no_lobbies_yet
        } else {
            &strings.waiting_for_lobby
        };
        crate::ui::theme::outlined_label(
            ui,
            label,
            egui::FontId::proportional(16.0),
            crate::ui::theme::palette::text_muted(),
        );
        return;
    }

    let ffa_lobbies: Vec<_> = state
        .lobbies
        .iter()
        .filter(|l| l.game_mode == "FFA")
        .collect();
    let team_lobbies: Vec<_> = state
        .lobbies
        .iter()
        .filter(|l| l.game_mode == "Teams")
        .collect();

    let mut draw_lobby = |ui: &mut Ui, lobby: &sow_core::protocol::LobbyInfo| {
        let thumbnail = asset_loader.thumbnail(&lobby.map_name);
        let response = ui.add(LobbyCard::new(lobby, thumbnail).side(side));
        if response.clicked() {
            *action = Some(UiAction::JoinLobby(lobby.id));
        }
        if thumbnail.is_none() {
            if let Some(err) = asset_loader.thumbnail_error(&lobby.map_name) {
                let _ = err;
                ui.label(
                    egui::RichText::new(&strings.thumbnail_load_failed)
                        .size(11.0)
                        .color(crate::ui::theme::palette::text_muted()),
                );
            } else if asset_loader.thumbnail_in_flight(&lobby.map_name) {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        egui::RichText::new(&strings.loading_thumbnail)
                            .size(11.0)
                            .color(crate::ui::theme::palette::text_muted()),
                    );
                });
            }
        }
        ui.add_space(8.0);
    };

    if !ffa_lobbies.is_empty() {
        for lobby in ffa_lobbies {
            draw_lobby(ui, lobby);
        }
    }

    if !team_lobbies.is_empty() {
        ui.add_space(8.0);
        for lobby in team_lobbies {
            draw_lobby(ui, lobby);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_left_column(
    ui: &mut Ui,
    state: &mut MainMenuState,
    _section_gap: f32,
    _action_min_h: f32,
    compact: bool,
    max_height: f32,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
) {
    let side = crate::ui::map_texture::thumbnail_square_side_bounded(
        ui.available_width(),
        max_height,
        compact,
    );

    if max_height > 0.0 {
        egui::ScrollArea::vertical()
            .id_salt("main_menu_lobby_scroll")
            .max_height(max_height)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                draw_lobby_list(ui, state, side, action, asset_loader, lang);
            });
    } else {
        draw_lobby_list(ui, state, side, action, asset_loader, lang);
    }
}
