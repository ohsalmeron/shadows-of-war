
use crate::{UiAction, ui::{main_menu, hud, loading_screen, asset_loader}};

#[derive(Debug, Clone, PartialEq)]
pub enum ClientPhase {
    Splash,
    MainMenu,
    Playing,
}

pub struct ClientApp {
    pub phase: ClientPhase,
    pub main_menu_state: main_menu::MainMenuState,
    pub hud_state: hud::HudState,
    pub splash_state: loading_screen::SplashState,
    pub asset_loader: asset_loader::AssetLoader,
    pub is_settings_open: bool,
    pub settings_state: crate::ui::settings::SettingsState,
}

impl Default for ClientApp {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientApp {
    pub fn new() -> Self {
        Self {
            phase: ClientPhase::Splash,
            main_menu_state: main_menu::MainMenuState::default(),
            hud_state: hud::HudState {
                gold: 0.0,
                troops: 0.0,
                troops_display: 0.0,
                max_troops: 0.0,
                max_troops_display: 0.0,
                attack_ratio: 0.25,
                spawn_timer_secs: None,
                sync_state: None,
                last_troops_ui_refresh: None,
                my_player_id: 0,
                attacks: Vec::new(),
                fleets: Vec::new(),
                players: Vec::new(),
                safe_area_top: 0.0,
                safe_area_bottom: 0.0,
                selected_tile: None,
                show_emoji_panel: false,
                emoji_panel_pos: None,
                emoji_panel_just_opened: false,
                pin_emoji: false,
                show_alliance_inbox: false,
                prev_requests: Vec::new(),
                show_betrayal_warning: None,
                show_error: None,
                last_error_message: None,
                error_display_timer: None,
                selected_building_kind: None,
                building_costs: [0.0; 6],
                selected_nuke_kind: None,
            },
            splash_state: loading_screen::SplashState::default(),
            asset_loader: asset_loader::AssetLoader::new(),
            is_settings_open: false,
            settings_state: crate::ui::settings::SettingsState::default(),
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>) -> Option<UiAction> {
        let mut action = match self.phase {
            ClientPhase::MainMenu => {
                self.asset_loader.ensure_avatars_loaded(ui.ctx());
                self.asset_loader.ensure_ui_assets_loaded(ui.ctx());
                main_menu::draw(ui, &mut self.main_menu_state, &self.asset_loader, self.settings_state.language)
            }
            ClientPhase::Splash => {
                self.asset_loader.ensure_ui_assets_loaded(ui.ctx());
                if let Some(new_phase) = loading_screen::draw(ui, &mut self.splash_state, &self.asset_loader, self.settings_state.language) {
                    self.phase = new_phase;
                }
                None
            }
            ClientPhase::Playing => {
                hud::draw(ui, &mut self.hud_state, cancel_intents, self.settings_state.language)
            }
        };

        if self.is_settings_open {
            let settings_action = crate::ui::settings::draw(ui, &mut self.settings_state);
            if let Some(UiAction::ToggleSettings) = settings_action {
                self.is_settings_open = false;
            }
        } else if let Some(UiAction::ToggleSettings) = action {
            self.is_settings_open = true;
            action = None;
        }

        action
    }
}

