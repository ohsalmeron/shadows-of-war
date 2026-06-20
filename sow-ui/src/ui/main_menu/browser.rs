use super::MainMenuState;
use crate::widgets::LobbyCard;
use crate::UiAction;
use egui::Ui;

fn find_primary_lobby<'a>(
    lobbies: &[&'a sow_core::protocol::LobbyInfo],
) -> Option<&'a sow_core::protocol::LobbyInfo> {
    let mut counting: Vec<_> = lobbies
        .iter()
        .filter(|l| l.is_counting_down)
        .copied()
        .collect();
    if !counting.is_empty() {
        counting.sort_by_key(|l| l.id);
        return Some(counting[0]);
    }
    let mut waiting: Vec<_> = lobbies
        .iter()
        .filter(|l| !l.is_counting_down)
        .copied()
        .collect();
    if !waiting.is_empty() {
        waiting.sort_by_key(|l| l.id);
        return Some(waiting[0]);
    }
    None
}

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

    // Main menu shows ONLY Matchmaking queues — Custom lobbies live in the Game Browser.
    let ffa_lobbies: Vec<_> = state
        .lobbies
        .iter()
        .filter(|l| l.game_mode == "FFA" && l.kind == sow_core::protocol::LobbyKind::Matchmaking)
        .collect();
    let team_lobbies: Vec<_> = state
        .lobbies
        .iter()
        .filter(|l| l.game_mode == "Teams" && l.kind == sow_core::protocol::LobbyKind::Matchmaking)
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

    let ffa_lobby = find_primary_lobby(&ffa_lobbies);
    let team_lobby = find_primary_lobby(&team_lobbies);

    if let Some(lobby) = ffa_lobby {
        draw_lobby(ui, lobby);
    }

    if let Some(lobby) = team_lobby {
        if ffa_lobby.is_some() {
            ui.add_space(8.0);
        }
        draw_lobby(ui, lobby);
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
    let side = if compact && !sow_ui_kit::theme::portrait_layout(ui.ctx()) {
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
