use crate::app::SowApp;

impl SowApp {
    pub(crate) fn send_intent(&mut self, intent: sow_core::protocol::GameplayIntent) {
        match &intent {
            sow_core::protocol::GameplayIntent::LaunchFleet { target_tile, .. }
            | sow_core::protocol::GameplayIntent::MoveWarships { target_tile, .. } => {
                let wx = (*target_tile % self.sim.map_w) as f32 + 0.5;
                let wy = (*target_tile / self.sim.map_w) as f32 + 0.5;
                self.ui.click_markers.push(crate::app::ClickMarker {
                    world_x: wx,
                    world_y: wy,
                    start_time: web_time::Instant::now(),
                });
            }
            sow_core::protocol::GameplayIntent::Spawn { x, y } => {
                let wx = *x as f32 + 0.5;
                let wy = *y as f32 + 0.5;
                self.ui.click_markers.push(crate::app::ClickMarker {
                    world_x: wx,
                    world_y: wy,
                    start_time: web_time::Instant::now(),
                });
                sow_audio::play_deploy_sound(self.spatial_sound_params(wx, wy));
                let seed = self
                    .sim
                    .engine
                    .as_ref()
                    .map(|e| e.state.seed as u32)
                    .unwrap_or(0);
                sow_audio::set_music_context(seed, wx, wy);
            }
            sow_core::protocol::GameplayIntent::BuildStructure { kind, target_tile } => {
                let wx = (*target_tile % self.sim.map_w) as f32 + 0.5;
                let wy = (*target_tile / self.sim.map_w) as f32 + 0.5;
                self.ui.click_markers.push(crate::app::ClickMarker {
                    world_x: wx,
                    world_y: wy,
                    start_time: web_time::Instant::now(),
                });
                sow_audio::play_building_placement_sound(
                    crate::building_sound_kind(*kind),
                    self.spatial_sound_params(wx, wy),
                );
            }
            sow_core::protocol::GameplayIntent::Attack(attack) => {
                // Flash enemy borders red
                self.ui
                    .border_flashes
                    .push(crate::app::BorderFlashInstance {
                        player_id: attack.target_owner,
                        start_time: web_time::Instant::now(),
                        max_intensity: 1.0,
                    });

                let world_x =
                    (self.input.last_mouse_x as f32 - self.input.camera_x) / self.input.camera_zoom;
                // Offset up by 60 screen pixels to keep the notice from being covered by a finger/mouse
                let offset_mouse_y = self.input.last_mouse_y as f32 - 60.0;
                let world_y = (offset_mouse_y - self.input.camera_y) / self.input.camera_zoom;
                if let Some(troops) = attack.troops {
                    if troops > 0.0 {
                        self.ui.floating_notices.push(crate::app::FloatingNotice {
                            text: format!("⚔️ +{}", sow_ui_kit::utils::format_number(troops)),
                            world_x,
                            world_y,
                            start_time: web_time::Instant::now(),
                            duration: web_time::Duration::from_millis(1500),
                            color: egui::Color32::from_rgb(6, 182, 212), // cyan
                        });
                    }
                }
            }
            _ => {}
        }

        if let Some(c) = self.net.client.as_ref() {
            let msg = sow_core::protocol::ClientMessage::Gameplay {
                intent: intent.clone(),
            };
            if let Ok(json) = bincode::serialize(&msg) {
                c.send(json);
            }
        } else {
            self.sim.offline_intents.push(intent);
        }
    }
}
