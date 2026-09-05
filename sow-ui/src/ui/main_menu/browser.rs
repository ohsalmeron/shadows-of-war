use super::MainMenuState;
use crate::UiAction;
use crate::widgets::LobbyCard;
use egui::Ui;

fn primary_matchmaking_lobby(
    lobbies: &[sow_core::protocol::LobbyInfo],
) -> Option<&sow_core::protocol::LobbyInfo> {
    lobbies
        .iter()
        .filter(|lobby| lobby.kind == sow_core::protocol::LobbyKind::Matchmaking)
        .min_by_key(|lobby| (!lobby.is_counting_down, lobby.id))
}

fn visible_matchmaking_lobby(state: &mut MainMenuState) -> Option<sow_core::protocol::LobbyInfo> {
    if !state.is_connected {
        return None;
    }

    if let Some(lobby) = primary_matchmaking_lobby(&state.lobbies).cloned() {
        state.last_matchmaking_lobby = Some(lobby.clone());
        Some(lobby)
    } else {
        state.last_matchmaking_lobby.clone()
    }
}

fn draw_empty_lobby_slot(ui: &mut Ui, card_width: f32, label: &str) {
    let card_height = card_width * 9.0 / 16.0;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(card_width, card_height), egui::Sense::hover());
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(12),
        sow_ui_kit::theme::palette::button_inactive(),
    );
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(12),
        egui::Stroke::new(1.0_f32, sow_ui_kit::theme::palette::field_border()),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        sow_ui_kit::theme::font_regular(15.0),
        sow_ui_kit::theme::palette::text_muted(),
    );
}

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

    // The server exposes one rotating matchmaking slot. Keep one fixed UI slot
    // as well; changing the lobby must replace card data, never the layout tree.
    let card_width = if ui.available_width() > 640.0 {
        (ui.available_width() * 0.65).min(560.0)
    } else {
        ui.available_width()
    };
    let primary = visible_matchmaking_lobby(state);

    ui.push_id("quick_match_slot", |ui| {
        if let Some(lobby) = primary.as_ref() {
            let thumbnail = asset_loader.thumbnail(&lobby.map_name);
            let display_name = asset_loader
                .map_catalog
                .as_ref()
                .and_then(|catalog| sow_core::maps::catalog_lookup(catalog, &lobby.map_name))
                .map(|entry| entry.display_name.clone())
                .unwrap_or_else(|| lobby.map_name.clone());
            let response = ui.add(
                LobbyCard::new(lobby, thumbnail)
                    .width(card_width)
                    .display_name(display_name),
            );
            if response.clicked() {
                *action = Some(UiAction::JoinLobby(lobby.id));
            }
        } else {
            let label = if state.is_connected {
                &strings.no_lobbies_yet
            } else {
                &strings.waiting_for_lobby
            };
            draw_empty_lobby_slot(ui, card_width, label);
        }
    });
    ui.add_space(8.0);
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

#[cfg(test)]
mod tests {
    use super::{MainMenuState, primary_matchmaking_lobby, visible_matchmaking_lobby};
    use sow_core::protocol::{LobbyInfo, LobbyKind};

    fn lobby(id: u64, kind: LobbyKind, is_counting_down: bool) -> LobbyInfo {
        LobbyInfo {
            id,
            num_players: 0,
            max_players: 10,
            is_counting_down,
            timer_secs: 0.0,
            map_name: "world".to_string(),
            game_mode: "FFA".to_string(),
            players: Vec::new(),
            has_password: false,
            host_name: String::new(),
            bot_count: 0,
            nation_count: 0,
            bot_difficulty: Default::default(),
            kind,
        }
    }

    #[test]
    fn quick_match_selects_one_matchmaking_lobby() {
        let lobbies = vec![
            lobby(1, LobbyKind::Custom, false),
            lobby(20, LobbyKind::Matchmaking, false),
            lobby(12, LobbyKind::Matchmaking, true),
            lobby(8, LobbyKind::Matchmaking, true),
        ];

        assert_eq!(
            primary_matchmaking_lobby(&lobbies).map(|lobby| lobby.id),
            Some(8)
        );
    }

    #[test]
    fn quick_match_keeps_last_lobby_during_connected_empty_snapshot() {
        let mut state = MainMenuState::default();
        state.is_connected = true;
        state.lobbies = vec![lobby(1, LobbyKind::Matchmaking, true)];

        assert_eq!(
            visible_matchmaking_lobby(&mut state).map(|lobby| lobby.id),
            Some(1)
        );

        state.lobbies.clear();
        assert_eq!(
            visible_matchmaking_lobby(&mut state).map(|lobby| lobby.id),
            Some(1)
        );

        state.lobbies = vec![lobby(2, LobbyKind::Matchmaking, true)];
        assert_eq!(
            visible_matchmaking_lobby(&mut state).map(|lobby| lobby.id),
            Some(2)
        );
    }
}
