pub mod app;
pub mod ui;
pub mod utils;
pub mod widgets;

pub use app::ClientApp;
pub use sow_ui_kit::hud::HudIcon;
pub use sow_ui_kit::{self, ClientPhase};
pub use ui::main_menu::LobbyNotice;

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
    #[cfg(any(feature = "dev", debug_assertions))]
    ToggleDevSidebar,
    CopyInviteLink(u64),
    StartPrivateLobby(u64),
    ResolveLinkConflict {
        keep_account_id: String,
    },
    PortalShowAuthPrompt,
    OpenCreateGame,
    CreateGame {
        config: Box<sow_core::game_config::GameConfig>,
        is_private: bool,
        password: Option<String>,
    },
    OpenJoinBrowser,
    CloseOverlay,
    JoinWithCode,
    JoinWithPassword(u64),
    KickPlayer {
        lobby_id: u64,
        target_player_id: u16,
    },
    BanPlayer {
        lobby_id: u64,
        target_player_id: u16,
    },
    /// Host toggles a player's team (Red↔Blue) in a Teams lobby.
    MovePlayerTeam {
        lobby_id: u64,
        target_player_id: u16,
    },
}
