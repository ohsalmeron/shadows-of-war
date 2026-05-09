use egui::Context;
use crate::{UiAction, hud, lobby};

#[derive(Debug, Clone, PartialEq)]
pub enum ClientPhase {
    MainMenu,
    Playing,
    GameOver,
}

pub struct ClientApp {
    pub phase: ClientPhase,
    pub lobby_state: lobby::LobbyState,
    pub hud_state: hud::HudState,
}

impl ClientApp {
    pub fn new() -> Self {
        Self {
            phase: ClientPhase::MainMenu,
            lobby_state: lobby::LobbyState::default(),
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
                lobby::draw(ctx, &mut self.lobby_state)
            }
            ClientPhase::Playing => {
                hud::draw(ctx, &mut self.hud_state)
            }
        }
    }
}
