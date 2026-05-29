//! Gameplay HUD toolbar icons (`assets/ui/hud/hud_<name>.webp`).
//!
//! Placeholders ship in-tree; replace each webp with final art (keep filenames).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HudIcon {
    Inbox,
    Settings,
    Exit,
    ZoomIn,
    ZoomOut,
    CenterCamera,
    Emoji,
    BattleLog,
    Leaderboard,
    DevTools,
    Controls,
    Logs,
}

impl HudIcon {
    pub const ALL: [Self; 12] = [
        Self::Inbox,
        Self::Settings,
        Self::Exit,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::CenterCamera,
        Self::Emoji,
        Self::BattleLog,
        Self::Leaderboard,
        Self::DevTools,
        Self::Controls,
        Self::Logs,
    ];

    pub fn file_name(self) -> &'static str {
        match self {
            Self::Inbox => "hud_inbox.webp",
            Self::Settings => "hud_settings.webp",
            Self::Exit => "hud_exit.webp",
            Self::ZoomIn => "hud_zoom_in.webp",
            Self::ZoomOut => "hud_zoom_out.webp",
            Self::CenterCamera => "hud_center_camera.webp",
            Self::Emoji => "hud_emoji.webp",
            Self::BattleLog => "hud_battle_log.webp",
            Self::Leaderboard => "hud_leaderboard.webp",
            Self::DevTools => "hud_dev_tools.webp",
            Self::Controls => "hud_controls.webp",
            Self::Logs => "hud_logs.webp",
        }
    }

    pub fn texture_name(self) -> &'static str {
        match self {
            Self::Inbox => "hud_icon_inbox",
            Self::Settings => "hud_icon_settings",
            Self::Exit => "hud_icon_exit",
            Self::ZoomIn => "hud_icon_zoom_in",
            Self::ZoomOut => "hud_icon_zoom_out",
            Self::CenterCamera => "hud_icon_center_camera",
            Self::Emoji => "hud_icon_emoji",
            Self::BattleLog => "hud_icon_battle_log",
            Self::Leaderboard => "hud_icon_leaderboard",
            Self::DevTools => "hud_icon_dev_tools",
            Self::Controls => "hud_icon_controls",
            Self::Logs => "hud_icon_logs",
        }
    }

    pub fn bytes(self) -> &'static [u8] {
        match self {
            Self::Inbox => include_bytes!("../../../assets/ui/hud/hud_inbox.webp"),
            Self::Settings => include_bytes!("../../../assets/ui/hud/hud_settings.webp"),
            Self::Exit => include_bytes!("../../../assets/ui/hud/hud_exit.webp"),
            Self::ZoomIn => include_bytes!("../../../assets/ui/hud/hud_zoom_in.webp"),
            Self::ZoomOut => include_bytes!("../../../assets/ui/hud/hud_zoom_out.webp"),
            Self::CenterCamera => include_bytes!("../../../assets/ui/hud/hud_center_camera.webp"),
            Self::Emoji => include_bytes!("../../../assets/ui/hud/hud_emoji.webp"),
            Self::BattleLog => include_bytes!("../../../assets/ui/hud/hud_battle_log.webp"),
            Self::Leaderboard => include_bytes!("../../../assets/ui/hud/hud_leaderboard.webp"),
            Self::DevTools => include_bytes!("../../../assets/ui/hud/hud_dev_tools.webp"),
            Self::Controls => include_bytes!("../../../assets/ui/hud/hud_controls.webp"),
            Self::Logs => include_bytes!("../../../assets/ui/hud/hud_logs.webp"),
        }
    }
}

impl super::BottomHudTab {
    pub fn hud_icon(self) -> HudIcon {
        match self {
            Self::Controls => HudIcon::Controls,
            Self::BattleLog => HudIcon::BattleLog,
            Self::EventLog => HudIcon::Logs,
        }
    }
}
