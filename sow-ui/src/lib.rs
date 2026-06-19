pub mod app;
pub mod ui;
pub mod ui_font;
pub mod utils;
pub mod widgets;

pub use app::ClientApp;

#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    ConnectToServer(String),
    RetryConnection,
    JoinLobby(u64),
    LeaveLobby,
    HostPrivateLobby,
    StartSinglePlayer(Box<sow_core::game_config::GameConfig>),
    SetAttackRatio(f32),
    CenterCamera,
    /// Pan + zoom camera to world-space tile coords (col, row).
    FocusTile(f32, f32),
    ZoomIn,
    ZoomOut,
    ToggleSettings,
    ToggleCredits,
    TogglePrivacy,
    ToggleTerms,
    ToggleDevSidebar,
    CopyInviteLink(u64),
    StartPrivateLobby(u64),
    ResolveLinkConflict {
        keep_account_id: String,
    },
    PortalShowAuthPrompt,
}
