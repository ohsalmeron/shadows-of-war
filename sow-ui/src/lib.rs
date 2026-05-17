pub mod app;
pub mod ui;
pub mod ui_font;

pub use app::ClientApp;

#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    ConnectToServer(String),
    JoinLobby(u64),
    LeaveLobby,
    StartSinglePlayer,
    SetAttackRatio(f32),
    CenterCamera,
    ToggleSettings,
}
