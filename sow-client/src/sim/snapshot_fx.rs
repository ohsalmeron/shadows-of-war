use crate::app::SowApp;
use sow_core::protocol::SimSnapshot;

impl SowApp {
    pub(crate) fn apply_snapshot_fx(&mut self, snap: &mut SimSnapshot, my_id: u16) {
        let mut being_attacked_triggered = false;
        if let Some(mut existing) = self.sim.current_snapshot.take() {
            if my_id != 0 {
                // 1. Detect incoming attacks (UnderAttack)
                for attack in &snap.attacks {
                    if attack.target_owner == my_id && attack.troops > 0.0 {
                        let is_new_or_increased = if let Some(old_atk) = existing.attacks.iter().find(|a| a.id == attack.id) {
                            attack.troops > old_atk.troops
                        } else {
                            true
                        };
                        if is_new_or_increased {
                            being_attacked_triggered = true;
                        }
                    }
                }

                // 2. Detect new alliance requests targeting us
                if let Some(my_info_new) = snap.players.iter().find(|p| p.id == my_id) {
                    if let Some(my_info_old) = existing.players.iter().find(|p| p.id == my_id) {
                        for req in &my_info_new.alliance_requests {
                            if !my_info_old.alliance_requests.contains(req) {
                                self.ui.trigger_viewport_alert(crate::app::ViewportAlertKind::AllianceRequest);
                            }
                        }
                    }
                }

                // 3. Detect betrayals (ally breaks alliance and is marked traitor)
                if let Some(my_info_new) = snap.players.iter().find(|p| p.id == my_id) {
                    if let Some(my_info_old) = existing.players.iter().find(|p| p.id == my_id) {
                        for ally_id in &my_info_old.alliances {
                            if !my_info_new.alliances.contains(ally_id) {
                                if let Some(other_player) = snap.players.iter().find(|p| p.id == *ally_id) {
                                    if other_player.traitor || other_player.active_emoji.as_deref() == Some("🗡️") {
                                        self.ui.trigger_viewport_alert(crate::app::ViewportAlertKind::Betrayal);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Count unique attackers targeting us in the new snapshot
            let unique_attackers = if my_id != 0 {
                snap.attacks.iter()
                    .filter(|a| a.target_owner == my_id && a.troops > 0.0)
                    .map(|a| a.owner_id)
                    .collect::<std::collections::HashSet<_>>()
                    .len()
            } else {
                0
            };

            if unique_attackers > 0 {
                let now = web_time::Instant::now();
                let should_flash = self.ui.last_player_attack_flash_time.get(&my_id).copied()
                    .map(|last| now.duration_since(last).as_secs_f32() >= 1.0)
                    .unwrap_or(true);

                if should_flash || being_attacked_triggered {
                    // Compose intensity: 1.0 base + 0.2 per attacker, clamped between 1.0 and 1.5
                    let intensity = (1.0 + (unique_attackers as f32 - 1.0) * 0.2).clamp(1.0, 1.5);
                    self.ui.border_flashes.push(crate::app::BorderFlashInstance {
                        player_id: my_id,
                        start_time: now,
                        max_intensity: intensity,
                    });
                    self.ui.last_player_attack_flash_time.insert(my_id, now);
                }
            } else {
                self.ui.last_player_attack_flash_time.remove(&my_id);
            }

            if being_attacked_triggered {
                self.ui.trigger_viewport_alert(crate::app::ViewportAlertKind::UnderAttack);
            }

            // Detect building level upgrades and completions
            for b_new in &snap.buildings {
                if let Some(b_old) = existing.buildings.iter().find(|b| b.id == b_new.id) {
                    if b_new.level > b_old.level
                        || (b_old.under_construction && !b_new.under_construction)
                    {
                        // ponytail: active_upgrades animations removed as they were dead code
                    }
                    // ponytail: only play building completion sound for the local player
                    if b_old.under_construction
                        && !b_new.under_construction
                        && b_new.owner_id == my_id
                        && my_id != 0
                    {
                        let wx = (b_new.tile_idx % self.sim.map_w) as f32 + 0.5;
                        let wy = (b_new.tile_idx / self.sim.map_w) as f32 + 0.5;
                        sow_audio::play_building_completed_sound(
                            crate::building_sound_kind(b_new.kind),
                            self.spatial_sound_params(wx, wy),
                        );
                    }
                }
            }

            if !existing.dirty_tiles.is_empty() {
                existing.dirty_tiles.append(&mut snap.dirty_tiles);
                snap.dirty_tiles = existing.dirty_tiles;
            }
        }
    }

    pub(crate) fn process_nuke_alerts(&mut self, snap: &SimSnapshot) {
        // Process nuke alerts into HUD notifications
        let my_id = self.sim.my_player_id.unwrap_or(0);
        for alert in &snap.nuke_alerts {
            let attacker_name = snap
                .players
                .iter()
                .find(|p| p.id == alert.owner_id)
                .map(|p| sow_core::player::display_name(p.id, &p.name, p.player_type))
                .unwrap_or_else(|| format!("Player {}", alert.owner_id));

            // Determine victim from tile ownership in previous snapshot state
            let tile_idx = alert.tile_y * self.sim.map_w + alert.tile_x;
            let victim_id = self
                .gfx
                .map_renderer
                .as_ref()
                .and_then(|mr| mr.owners.get(tile_idx as usize).copied())
                .unwrap_or(0);
            let victim_name = if victim_id == 0 {
                "unclaimed territory".to_string()
            } else {
                snap.players
                    .iter()
                    .find(|p| p.id == victim_id)
                    .map(|p| sow_core::player::display_name(p.id, &p.name, p.player_type))
                    .unwrap_or_else(|| format!("Player {}", victim_id))
            };

            let kind_str = match alert.kind {
                sow_core::game::NukeKind::AtomBomb => "Tactical Nuke",
            };

            let (message, color) = if victim_id == my_id && my_id != 0 {
                // You got nuked
                (
                    format!("{} launched {} on YOUR territory!", attacker_name, kind_str),
                    egui::Color32::from_rgb(239, 68, 68),
                )
            } else if alert.owner_id == my_id {
                // You nuked someone
                (
                    format!("Your {} detonated on {}", kind_str, victim_name),
                    egui::Color32::from_rgb(74, 222, 128),
                )
            } else if my_id != 0
                && snap
                    .players
                    .iter()
                    .find(|p| p.id == my_id)
                    .map(|p| p.alliances.contains(&victim_id))
                    .unwrap_or(false)
                && victim_id != 0
            {
                // Ally got nuked
                (
                    format!(
                        "{} launched {} on ally {}!",
                        attacker_name, kind_str, victim_name
                    ),
                    egui::Color32::from_rgb(251, 191, 36),
                )
            } else {
                // Enemy vs enemy / neutral
                (
                    format!("{} launched {} on {}", attacker_name, kind_str, victim_name),
                    egui::Color32::from_rgb(180, 180, 200),
                )
            };

            self.ui.app.hud_state.push_notification(message, color);
        }
    }
}

