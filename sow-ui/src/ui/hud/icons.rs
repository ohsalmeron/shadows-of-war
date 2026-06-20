pub use sow_ui_kit::hud::HudIcon;

use super::state::BottomHudTab;

impl BottomHudTab {
    pub fn hud_icon(self) -> HudIcon {
        match self {
            Self::Controls => HudIcon::Controls,
            Self::BattleLog => HudIcon::BattleLog,
            Self::EventLog => HudIcon::Logs,
        }
    }
}
