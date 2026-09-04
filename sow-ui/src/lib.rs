pub mod app;
pub mod ui;
pub mod utils;
pub mod widgets;
extern crate self as sow_ui_kit;

pub mod kit;
pub use kit::{
    ClientPhase, EMOJI_ATLAS_BYTES, assets, atlas_texture, atlas_uv, register_emoji_atlas,
    register_game_assets, texture_options, theme, ui_font,
};

pub use app::ClientApp;
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
    StartPrivateLobby(u64),
    PortalShowAuthPrompt,
    /// Persist the anonymous player's edited display name.
    SaveDisplayName(String),
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
    /// Native desktop fullscreen toggle.
    SetFullscreen(bool),
    /// Toggle the UI Kit Showcase full-screen gallery.
    ToggleShowcase,
    /// Open the native store page. The platform-specific purchase action is separate.
    OpenStorePage,
    /// Open the native player profile page.
    OpenProfilePage,
    /// Load the currently authenticated player's public profile.
    LoadOwnProfile,
    /// Open another public player profile.
    OpenPublicProfilePage(String),
    /// Load more match history for the active profile.
    LoadProfileHistory,
    /// Load ranked records for the active profile.
    LoadProfileRatings,
    /// Search public player profiles.
    SearchProfiles(String),
    /// Load a public match detail modal.
    LoadMatchDetail(String),
    /// Close the public match detail modal.
    CloseMatchDetail,
    /// Open the native platform store (iOS RevenueCat; web/Android use their own shell).
    OpenStore,
    /// Open the RevenueCat web checkout for a gem bundle on desktop.
    BuyGems(String),
    /// Spend authoritative laurels or gems on a leader.
    UnlockLeader {
        leader_id: String,
        currency: String,
    },
    /// Spend authoritative gems on a skin.
    UnlockSkin(String),
    /// Equip an already-owned skin.
    EquipSkin(String),
}
