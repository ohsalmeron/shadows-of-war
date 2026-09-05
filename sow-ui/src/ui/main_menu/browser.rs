use super::MainMenuState;
use crate::UiAction;
use crate::widgets::LobbyCard;
use egui::Ui;

fn draw_lobby_list(
    ui: &mut Ui,
    state: &mut MainMenuState,
    _side: f32,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
) {
    let strings = &sow_i18n::get(lang).main_menu;

    if state.is_waiting || state.joined_lobby_id.is_some() || state.pending_join_lobby_id.is_some()
    {
        return;
    }

    let has_matchmaking = state
        .lobbies
        .iter()
        .any(|l| l.kind == sow_core::protocol::LobbyKind::Matchmaking);
    if !has_matchmaking {
        let label = if state.is_connected {
            &strings.no_lobbies_yet
        } else {
            &strings.waiting_for_lobby
        };
        sow_ui_kit::theme::outlined_label(
            ui,
            label,
            egui::FontId::proportional(16.0),
            sow_ui_kit::theme::palette::text_muted(),
        );
        return;
    }

    // Main menu shows the Matchmaking queue — every joinable lobby, one card per
    // game mode (OpenFront-style: the FFA and Teams rooms side by side).
    let matchmaking_lobbies: Vec<_> = state
        .lobbies
        .iter()
        .filter(|l| l.kind == sow_core::protocol::LobbyKind::Matchmaking)
        .collect();

    let mut draw_lobby = |ui: &mut Ui, lobby: &sow_core::protocol::LobbyInfo| {
        let thumbnail = asset_loader.thumbnail(&lobby.map_name);
        let parent_w = ui.available_width();
        let card_w = if parent_w > 640.0 {
            (parent_w * 0.65).min(560.0)
        } else {
            parent_w
        };
        let display_name = asset_loader
            .map_catalog
            .as_ref()
            .and_then(|catalog| sow_core::maps::catalog_lookup(catalog, &lobby.map_name))
            .map(|entry| entry.display_name.clone())
            .unwrap_or_else(|| lobby.map_name.clone());
        let response = ui.add(
            LobbyCard::new(lobby, thumbnail)
                .width(card_w)
                .display_name(display_name),
        );
        if response.clicked() {
            *action = Some(UiAction::JoinLobby(lobby.id));
        }
        if thumbnail.is_none() {
            if let Some(err) = asset_loader.thumbnail_error(&lobby.map_name) {
                let _ = err;
                ui.label(
                    egui::RichText::new(&strings.thumbnail_load_failed)
                        .size(11.0)
                        .color(sow_ui_kit::theme::palette::text_muted()),
                );
            } else if asset_loader.thumbnail_in_flight(&lobby.map_name) {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        egui::RichText::new(&strings.loading_thumbnail)
                            .size(11.0)
                            .color(sow_ui_kit::theme::palette::text_muted()),
                    );
                });
            }
        }
        ui.add_space(8.0);
    };

    if matchmaking_lobbies.is_empty() {
        return;
    }
    for lobby in matchmaking_lobbies {
        draw_lobby(ui, lobby);
    }
}

pub fn draw_left_column(
    ui: &mut Ui,
    state: &mut MainMenuState,
    compact: bool,
    max_height: f32,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
) {
    let side = if compact && !super::layout::main_menu_metrics(ui.ctx()).is_phone() {
        crate::ui::map_texture::thumbnail_square_side_bounded(
            ui.available_width(),
            max_height,
            compact,
        )
    } else {
        max_height
    };

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
