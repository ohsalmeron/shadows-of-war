pub mod app;
pub mod hud;
pub mod lobby;

pub use app::ClientApp;

#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    ConnectToServer(String),
    JoinLobby(u64),
    CreateLobby,
    LeaveLobby,
    StartSinglePlayer,
    SetAttackRatio(f32),
    LaunchAttack {
        target_owner: u16,
        troops: Option<f64>,
    },
}
