use crate::app::SowApp;
use web_time::Instant;

impl SowApp {
    pub fn update_sim(&mut self, now: Instant) {
                self.ui.app.hud_state.is_mobile = self.input.screen_w < 900.0;
                if let Some(snap) = &self.sim.current_snapshot {
                    if let Some(target_secs) = snap.spawn_timer_secs {
                        if let Some(ref mut current) = self.ui.app.hud_state.spawn_timer_secs {
                            if (*current - target_secs).abs() > 0.3 {
                                *current = target_secs;
                            }
                        } else {
                            self.ui.app.hud_state.spawn_timer_secs = Some(target_secs);
                        }
                    } else {
                        self.ui.app.hud_state.spawn_timer_secs = None;
                    }
                } else {
                    self.ui.app.hud_state.spawn_timer_secs = None;
                }
                if self.ui.app.phase == sow_ui::app::ClientPhase::Playing {
                    if self.net.client.is_some() {
                        // Multiplayer: lockstep execution dictated by server
                        let mut ticks_processed = 0;
                        while let Some(turn) = self.sim.turn_queue.pop_front() {
                            self.dispatch_sim_command(sow_core::protocol::SimCommand::Turn(turn));
                            
                            // Update UI HUD State from my player id
                            if let Some(player) = self.sim.current_snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.id == self.sim.my_player_id.unwrap_or(1))) {
                                self.ui.app.hud_state.gold = player.gold;
                                self.ui.app.hud_state.troops = player.troops;
                                let owned_tiles = player.tile_count as f64;
                                self.ui.app.hud_state.max_troops = owned_tiles * 50.0;
                            }

                            ticks_processed += 1;
                            if ticks_processed >= 10 {
                                break;
                            }
                        }
                        self.time.last_tick = now;
                    } else {
                        // Singleplayer: HUD updates based on local timer (ticks are handled by mod.rs)
                        if now.duration_since(self.time.last_tick) >= self.time.tick_interval {
                            self.time.last_tick = now;
                            
                            if let Some(player) = self.sim.current_snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.id == self.sim.my_player_id.unwrap_or(1))) {
                                self.ui.app.hud_state.gold = player.gold;
                                self.ui.app.hud_state.troops = player.troops;
                                let owned_tiles = player.tile_count as f64;
                                self.ui.app.hud_state.max_troops = owned_tiles * 50.0;
                            }
                        }
                    }
                } else {
                    self.time.last_tick = now;
                }
                // Snapshot is now instantly updated by dispatch_sim_command
                if self.ui.app.splash_state.gpu_load_step == 3 && self.sim.current_snapshot.is_some() {
                    self.ui.app.splash_state.gpu_load_step = 4;
                    self.ui.app.phase = sow_ui::app::ClientPhase::Playing;
                    
                    // Clear pending init data to completely finish EnterGame phase
                    self.tasks.pending_engine_init_data = None;
                    log::info!("First snapshot received, releasing loader!");
                    
                    if let Some(c) = self.net.client.as_ref() {
                        if let (Some(lid), Some(pid)) = (self.sim.my_lobby_id, self.sim.my_player_id) {
                            let ready_msg = sow_core::protocol::ClientMessage::Ready { lobby_id: lid, player_id: pid };
                            let json = bincode::serialize(&ready_msg).unwrap();
                            c.send(json);
                        }
                    }
                }
                
                if self.ui.app.phase == sow_ui::app::ClientPhase::Playing && !self.input.has_snapped_camera_to_spawn {
                    if let Some(pid) = self.sim.my_player_id {
                        if let Some(snap) = &self.sim.current_snapshot {
                            if let Some(player) = snap.players.iter().find(|p| p.id == pid) {
                                let is_playing = matches!(snap.phase, sow_core::game::GamePhase::Playing);
                                if player.tile_count > 0 && player.alive && is_playing {
                                    let world_cx = player.centroid_x + 0.5;
                                    let world_cy = player.centroid_y + 0.5;
                                    self.input.camera_zoom = 20.0;
                                    self.input.camera_x = self.input.screen_w * 0.5 - world_cx * self.input.camera_zoom;
                                    self.input.camera_y = self.input.screen_h * 0.5 - world_cy * self.input.camera_zoom;
                                    self.input.has_snapped_camera_to_spawn = true;
                                    log::info!("Game started! Camera snapped to player spawn at ({}, {}), zoom={}", world_cx, world_cy, self.input.camera_zoom);
                                }
                            }
                        }
                    }
                }
                
                if let Some(win) = self.gfx.window.as_ref() {
                    win.request_redraw();
                }

                // Periodic memory profiler print
                let now = web_time::Instant::now();
                if self.time.last_debug_print.is_none_or(|t| now.duration_since(t).as_secs() >= 5) {
                    self.time.last_debug_print = Some(now);
                    if let Some(snap) = &self.sim.current_snapshot {
                        log::info!("[MEM_PROFILER] Turn Queue: {} | Dirty Tiles: {} | {}", self.sim.turn_queue.len(), snap.dirty_tiles.len(), snap.debug_mem_info);
                    }
                }

    }
}
