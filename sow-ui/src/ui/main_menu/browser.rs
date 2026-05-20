use crate::UiAction;
use egui::Ui;
use super::MainMenuState;
use crate::widgets::LobbyCard;

pub fn draw_left_column(
    ui: &mut Ui,
    state: &mut MainMenuState,
    _section_gap: f32,
    _action_min_h: f32,
    _compact: bool,
    action: &mut Option<UiAction>,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_lang::Language,
) {
    let strings = &sow_lang::get(lang).main_menu;
    let total_lobbies = state.lobbies.len();
    let max_h = if total_lobbies > 0 {
        ((ui.available_height() - 40.0) / total_lobbies as f32).max(100.0)
    } else {
        160.0
    };

    if state.lobbies.is_empty() {
        crate::ui::theme::outlined_label(
            ui,
            &strings.waiting_for_lobby,
            egui::FontId::proportional(16.0),
            crate::ui::theme::text_secondary()
        );

    } else {
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

        if !ffa_lobbies.is_empty() {
            for lobby in ffa_lobbies {
                let thumbnail = asset_loader.thumbnail(&lobby.map_name);
                let response = ui.add(LobbyCard::new(lobby, thumbnail).max_h(max_h));
                if response.clicked() {
                    *action = Some(UiAction::JoinLobby(lobby.id));
                }
                ui.add_space(8.0);
            }
        }

        if !team_lobbies.is_empty() {
            ui.add_space(8.0);
            for lobby in team_lobbies {
                let thumbnail = asset_loader.thumbnail(&lobby.map_name);
                let response = ui.add(LobbyCard::new(lobby, thumbnail).max_h(max_h));
                if response.clicked() {
                    *action = Some(UiAction::JoinLobby(lobby.id));
                }
                ui.add_space(8.0);
            }
        }
    }
}
