use crate::app::SowApp;
use sow_core::protocol::SimSnapshot;

pub(crate) struct CaptureEventInfo {
    pub pos: (u32, u32),
    pub new_owner: u16,
    pub previous_owner: u16,
    pub troops: f64,
}

impl SowApp {
    pub(crate) fn handle_tile_captured(
        &mut self,
        snap: &SimSnapshot,
        my_id: u16,
        played_combat_this_tick: &mut bool,
        info: &CaptureEventInfo,
    ) {
        let (x, y) = info.pos;
        let new_owner = info.new_owner;
        let previous_owner = info.previous_owner;
        let troops = info.troops;
        if *played_combat_this_tick || my_id == 0 {
            return;
        }
        if new_owner != my_id && previous_owner != my_id {
            return;
        }
        *played_combat_this_tick = true;

        use sow_audio::{CombatSoundKind, play_combat_sound};
        use sow_core::player::PlayerType;

        let kind = if previous_owner == my_id {
            CombatSoundKind::CounterAttack
        } else if previous_owner == 0 {
            CombatSoundKind::WildernessExpansion
        } else {
            snap.players
                .iter()
                .find(|p| p.id == previous_owner)
                .map(|p| match p.player_type {
                    PlayerType::Human => CombatSoundKind::AttackHuman,
                    PlayerType::Nation => CombatSoundKind::AttackEmpire,
                    PlayerType::Bot => CombatSoundKind::AttackTribe,
                })
                .unwrap_or(CombatSoundKind::AttackTribe)
        };

        let seed = (previous_owner as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(x.wrapping_mul(1597334977))
            .wrapping_add(y.wrapping_mul(3512401961))
            .wrapping_add((troops as u32).wrapping_mul(7243));

        play_combat_sound(
            kind,
            troops as f32,
            seed,
            self.spatial_sound_params(x as f32 + 0.5, y as f32 + 0.5),
        );
    }
}
