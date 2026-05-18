use egui::Context;
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
                is_mobile: false,
                spawn_timer_secs: None,
                sync_state: None,
                last_troops_ui_refresh: None,
                my_player_id: 0,
                attacks: Vec::new(),
                fleets: Vec::new(),
                players: Vec::new(),
            },
            splash_state: loading_screen::SplashState::default(),
            asset_loader: asset_loader::AssetLoader::new(),
            is_settings_open: false,
            settings_state: crate::ui::settings::SettingsState::default(),
        }
    }

    pub fn draw(&mut self, ctx: &Context, cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>) -> Option<UiAction> {
        let mut action = match self.phase {
            ClientPhase::MainMenu => {
                main_menu::draw(ctx, &mut self.main_menu_state, &self.asset_loader)
            }
            ClientPhase::Splash => {
                loading_screen::draw(ctx, &mut self.splash_state);
                None
            }
            ClientPhase::Playing => {
                hud::draw(ctx, &mut self.hud_state, cancel_intents)
            }
        };

        if self.is_settings_open {
            let settings_action = crate::ui::settings::draw(ctx, &mut self.settings_state);
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

