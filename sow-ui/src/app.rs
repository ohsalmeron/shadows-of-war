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
            },
            splash_state: loading_screen::SplashState::default(),
            asset_loader: asset_loader::AssetLoader::new(),
        }
    }

    pub fn draw(&mut self, ctx: &Context) -> Option<UiAction> {
        match self.phase {
            ClientPhase::MainMenu => {
                main_menu::draw(ctx, &mut self.main_menu_state, &self.asset_loader)
            }
            ClientPhase::Splash => {
                loading_screen::draw(ctx, &mut self.splash_state);
                None
            }
            ClientPhase::Playing => {
                hud::draw(ctx, &mut self.hud_state)
            }
        }
    }
}
