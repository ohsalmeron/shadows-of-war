use egui::Context;
use crate::{UiAction, ui::{main_menu, hud}};

#[derive(Debug, Clone, PartialEq)]
pub enum ClientPhase {
    MainMenu,
    Playing,
    GameOver,
}

pub struct ClientApp {
    pub phase: ClientPhase,
    pub main_menu_state: main_menu::MainMenuState,
    pub hud_state: hud::HudState,
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
                attack_ratio: 1.0,
                is_mobile: false,
            },
        }
    }

    pub fn draw(&mut self, ctx: &Context) -> Option<UiAction> {
        match self.phase {
            ClientPhase::MainMenu | ClientPhase::GameOver => {
                main_menu::draw(ctx, &mut self.main_menu_state)
            }
            ClientPhase::Playing => {
                hud::draw(ctx, &mut self.hud_state)
            }
        }
    }
}
