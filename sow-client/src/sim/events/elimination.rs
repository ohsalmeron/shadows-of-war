use crate::app::SowApp;
use sow_core::protocol::SimSnapshot;

impl SowApp {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_player_eliminated(
        &mut self,
        snap: &SimSnapshot,
        my_id: u16,
        now_instant: web_time::Instant,
        turn_defeats: &mut crate::player_progress::SessionDefeats,
        player_id: u16,
        conqueror_id: u16,
        gold_bounty: u32,
        elimination_x: u32,
        elimination_y: u32,
        assists: &[(u16, u32)],
        by_nuke: bool,
    ) {
        let mut wx = 0.5;
        let mut wy = 0.5;
        let mut target_name = format!("Player {}", player_id);

        let mut tile_found = false;
        if elimination_x > 0 || elimination_y > 0 {
            wx = elimination_x as f32 + 0.5;
            wy = elimination_y as f32 + 0.5;
            tile_found = true;
        }

        if let Some(target) = snap.players.iter().find(|p| p.id == player_id) {
            target_name =
                sow_core::player::display_name(target.id, &target.name, target.player_type);
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
            let bounty_text = format!("🪙 +{}", sow_ui::utils::format_number(gold_bounty as f64));
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
                    sow_ui::utils::format_number(*assist_gold as f64)
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

        // Spawn death nameplate animations on desktop only
        if self.input.screen_w >= 600.0 {
            // Spawn death nameplate animation
            let mut target_player_type = sow_core::player::PlayerType::Bot;
            let mut player_color = egui::Color32::WHITE;
            let mut target_nameplate_size = 0.0;

            if let Some(target) = snap.players.iter().find(|p| p.id == player_id) {
                target_player_type = target.player_type;
                player_color = crate::hud::nameplate::ensure_readable_nameplate_color(target.color);
                target_nameplate_size = target.nameplate_size;
            }

            // Prefer smoothed label positions and sizes if available
            let anim_wx = self
                .ui
                .label_positions
                .get(&player_id)
                .map(|p| p.0)
                .unwrap_or(wx);
            let anim_wy = self
                .ui
                .label_positions
                .get(&player_id)
                .map(|p| p.1)
                .unwrap_or(wy);
            let anim_size = self
                .ui
                .label_sizes
                .get(&player_id)
                .copied()
                .unwrap_or(target_nameplate_size)
                .max(0.2);

            let seed = (player_id as u32)
                .wrapping_mul(2654435761)
                .wrapping_add(now_instant.elapsed().as_millis() as u32);
            self.ui
                .death_nameplates
                .push(crate::app::DeathNameplateAnimation {
                    name: target_name.clone(),
                    color: player_color,
                    world_x: anim_wx,
                    world_y: anim_wy,
                    start_time: now_instant,
                    duration: web_time::Duration::from_millis(1200),
                    seed,
                    player_type: target_player_type,
                    player_id,
                    nameplate_size: anim_size,
                    by_nuke,
                });
        }

        // Push notification message (always, including mobile!)
        let msg = if conqueror_id == my_id && my_id != 0 {
            format!(
                "🎉 You conquered {} and earned {} Gold!",
                target_name,
                sow_ui::utils::format_number(gold_bounty as f64)
            )
        } else if assists.iter().any(|(id, _)| *id == my_id) {
            let assist_gold = assists
                .iter()
                .find(|(id, _)| *id == my_id)
                .map(|(_, g)| *g)
                .unwrap_or(0);
            format!(
                "🤝 Assist on {} (+{} Gold)",
                target_name,
                sow_ui::utils::format_number(assist_gold as f64)
            )
        } else {
            let conqueror_name = snap
                .players
                .iter()
                .find(|p| p.id == conqueror_id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| format!("Player {}", conqueror_id));
            let emoji = if by_nuke { "☢️" } else { "🕊️" };
            if assists.is_empty() {
                format!(
                    "{} {} was eliminated by {}!",
                    emoji, target_name, conqueror_name
                )
            } else {
                format!(
                    "{} {} was eliminated by {} (+{} assists)",
                    emoji,
                    target_name,
                    conqueror_name,
                    assists.len()
                )
            }
        };
        self.ui
            .app
            .hud_state
            .push_notification(msg, egui::Color32::from_rgb(255, 215, 0));
    }
}
