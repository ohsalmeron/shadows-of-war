//! Gameplay HUD bottom-tab icons (`assets/static/ui/hud/hud_<name>.webp`).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HudIcon {
    BattleLog,
    Controls,
    Logs,
}

impl HudIcon {
    pub const ALL: [Self; 3] = [Self::Controls, Self::BattleLog, Self::Logs];

    pub fn file_name(self) -> &'static str {
        match self {
            Self::Controls => "hud_controls.webp",
            Self::BattleLog => "hud_battle_log.webp",
            Self::Logs => "hud_logs.webp",
        }
    }

    pub fn texture_name(self) -> &'static str {
        match self {
            Self::Controls => "hud_icon_controls",
            Self::BattleLog => "hud_icon_battle_log",
            Self::Logs => "hud_icon_logs",
        }
    }

    pub fn bytes(self) -> &'static [u8] {
        match self {
            Self::Controls => sow_core::repo_asset_bytes!("ui/hud/hud_controls.webp"),
            Self::BattleLog => sow_core::repo_asset_bytes!("ui/hud/hud_battle_log.webp"),
            Self::Logs => sow_core::repo_asset_bytes!("ui/hud/hud_logs.webp"),
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
