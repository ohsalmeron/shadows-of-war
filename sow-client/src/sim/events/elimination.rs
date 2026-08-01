use crate::app::SowApp;
use sow_core::protocol::SimSnapshot;

pub(crate) struct EliminationEventInfo<'a> {
    pub player_id: u16,
    pub conqueror_id: u16,
    pub gold_bounty: u32,
    pub pos: (u32, u32),
    pub assists: &'a [(u16, u32)],
}

impl SowApp {
    pub(crate) fn handle_player_eliminated(
        &mut self,
        snap: &SimSnapshot,
        my_id: u16,
        now_instant: web_time::Instant,
        turn_defeats: &mut crate::player_progress::SessionDefeats,
        info: &EliminationEventInfo,
    ) {
        let player_id = info.player_id;
        let conqueror_id = info.conqueror_id;
        let gold_bounty = info.gold_bounty;
        let (elimination_x, elimination_y) = info.pos;
        let assists = info.assists;
        let mut wx = 0.5;
        let mut wy = 0.5;

        let mut tile_found = false;
        if elimination_x > 0 || elimination_y > 0 {
            wx = elimination_x as f32 + 0.5;
            wy = elimination_y as f32 + 0.5;
            tile_found = true;
        }

        if let Some(target) = snap.players.iter().find(|p| p.id == player_id) {
            if !tile_found && (target.centroid_x > 0.001 || target.centroid_y > 0.001) {
                wx = target.centroid_x + 0.5;
                wy = target.centroid_y + 0.5;
                tile_found = true;
            }
        }

        if !tile_found {
            // Fallback: Use conqueror's position as the visual reward point,
            // since the conqueror just claimed the target's last tile.
            if let Some(conqueror) = snap.players.iter().find(|p| p.id == conqueror_id) {
                wx = conqueror.centroid_x + 0.5;
                wy = conqueror.centroid_y + 0.5;
            }
        }

        let victim_type = snap
            .players
            .iter()
            .find(|p| p.id == player_id)
            .map(|p| p.player_type)
            .unwrap_or(sow_core::player::PlayerType::Bot);

        let seed = (player_id as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(elimination_x.wrapping_mul(1597334977))
            .wrapping_add(elimination_y.wrapping_mul(3512401961));

        // Play retro synthesized death sound spatially
        sow_audio::play_death_sound(
            crate::player_sound_type(victim_type),
            seed,
            self.spatial_sound_params(wx, wy),
        );

        if conqueror_id == my_id && my_id != 0 {
            if let Some(victim) = snap.players.iter().find(|p| p.id == player_id) {
                use sow_core::player::PlayerType;
                match victim.player_type {
                    PlayerType::Human => turn_defeats.players += 1,
                    PlayerType::Nation => turn_defeats.empires += 1,
                    PlayerType::Bot => turn_defeats.tribes += 1,
                }
            }
        }

        // Spawn floating notice for killer and assist contributors
        if conqueror_id == my_id && my_id != 0 {
            let bounty_text = format!(
                "🪙 +{}",
                sow_ui_kit::utils::format_number(gold_bounty as f64)
            );
            self.ui.floating_notices.push(crate::app::FloatingNotice {
                text: bounty_text,
                world_x: wx,
                world_y: wy,
                start_time: now_instant,
                duration: web_time::Duration::from_millis(3000),
                color: egui::Color32::from_rgb(250, 204, 21),
            });
        }
        for (assist_id, assist_gold) in assists {
            if *assist_id == my_id && my_id != 0 {
                let bounty_text = format!(
                    "🪙 +{} assist",
                    sow_ui_kit::utils::format_number(*assist_gold as f64)
                );
                self.ui.floating_notices.push(crate::app::FloatingNotice {
                    text: bounty_text,
                    world_x: wx,
                    world_y: wy + 0.5,
                    start_time: now_instant,
                    duration: web_time::Duration::from_millis(3000),
                    color: egui::Color32::from_rgb(180, 220, 100),
                });
            }
        }
    }
}
