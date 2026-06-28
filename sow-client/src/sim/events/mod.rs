mod combat;
mod elimination;
mod structures;

use crate::app::SowApp;
use sow_core::protocol::SimSnapshot;

impl SowApp {
    pub(crate) fn process_tick_events(
        &mut self,
        events: Vec<sow_core::game::GameEvent>,
        snap: &SimSnapshot,
        my_id: u16,
        now_instant: web_time::Instant,
    ) -> crate::player_progress::SessionDefeats {
        let mut turn_defeats = crate::player_progress::SessionDefeats::default();
        let mut played_combat_this_tick = false;
        for event in events {
            match event {
                sow_core::game::GameEvent::PlayerEliminated {
                    player_id,
                    conqueror_id,
                    gold_bounty,
                    elimination_x,
                    elimination_y,
                    assists,
                    by_nuke,
                } => {
                    self.handle_player_eliminated(
                        snap,
                        my_id,
                        now_instant,
                        &mut turn_defeats,
                        player_id,
                        conqueror_id,
                        gold_bounty,
                        elimination_x,
                        elimination_y,
                        &assists,
                        by_nuke,
                    );
                    if conqueror_id == my_id && my_id != 0 {
                        self.ui.trigger_viewport_alert(crate::app::ViewportAlertKind::ConquerPlayer);
                    }
                }
                sow_core::game::GameEvent::TileCaptured {
                    x,
                    y,
                    new_owner,
                    previous_owner,
                    troops,
                } => {
                    self.handle_tile_captured(
                        snap,
                        my_id,
                        &mut played_combat_this_tick,
                        x,
                        y,
                        new_owner,
                        previous_owner,
                        troops,
                    );
                }
                sow_core::game::GameEvent::StructureSpawned {
                    tile_idx,
                    kind,
                    owner_id,
                    ..
                } => {
                    self.handle_structure_spawned(my_id, tile_idx, kind, owner_id);
                }
                _ => {}
            }
        }
        turn_defeats
    }
}
