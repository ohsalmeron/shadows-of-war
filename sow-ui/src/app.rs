use egui::Context;
use crate::{UiAction, ui::{main_menu, hud, loading_screen}};

#[derive(Debug, Clone, PartialEq)]
pub enum ClientPhase {
    MainMenu,
    Loading,
    Playing,
    GameOver,
}

pub struct ClientApp {
    pub phase: ClientPhase,
    pub main_menu_state: main_menu::MainMenuState,
    pub hud_state: hud::HudState,
    pub loading_state: loading_screen::LoadingState,
}

impl ClientApp {
    pub fn new() -> Self {
        Self {
            phase: ClientPhase::MainMenu,
            main_menu_state: main_menu::MainMenuState::default(),
            hud_state: hud::HudState {
                gold: 0.0,
                troops: 0.0,
                max_troops: 0.0,
                attack_ratio: 0.25,
                is_mobile: false,
                sync_state: None,
            },
            loading_state: loading_screen::LoadingState::default(),
        }
    }

    pub fn draw(&mut self, ctx: &Context) -> Option<UiAction> {
        match self.phase {
            ClientPhase::MainMenu | ClientPhase::GameOver => {
                main_menu::draw(ctx, &mut self.main_menu_state)
            }
            ClientPhase::Loading => {
                loading_screen::draw(ctx, &mut self.loading_state);
                None
            }
            ClientPhase::Playing => {
                hud::draw(ctx, &mut self.hud_state)
            }
        }
    }
}
