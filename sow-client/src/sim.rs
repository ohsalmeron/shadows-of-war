use crate::app::SowApp;
use web_time::Instant;

impl SowApp {
    pub fn update_sim(&mut self, now: Instant) {
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
                                self.ui.app.hud_state.max_troops = player.max_troops;
                            }

                            ticks_processed += 1;
                            if ticks_processed >= 10 {
                                break;
                            }
                        }
                        self.time.last_tick = now;
                    } else {
                        // Singleplayer: offline tick generation and HUD updates
                        let dt = now.duration_since(self.time.last_tick).as_secs_f32();
                        self.time.last_tick = now;
                        
                        let mut safe_dt = dt;
                        if safe_dt > 0.1 { safe_dt = 0.05; } // Clamp to prevent tick burst
                        self.sim.offline_tick_timer += safe_dt;
                        
                        while self.sim.offline_tick_timer >= 0.05 { // 20 TPS (50ms)
                            self.sim.offline_tick_timer -= 0.05;
                            
                            let raw_intents = std::mem::take(&mut self.sim.offline_intents);
                            let mut stamped_intents = Vec::with_capacity(raw_intents.len());
                            for intent in raw_intents {
                                stamped_intents.push(sow_core::protocol::StampedIntent {
                                    player_id: self.sim.my_player_id.unwrap_or(1),
                                    intent,
                                });
                            }
                            
                            let turn = sow_core::protocol::Turn {
                                turn_number: 0, // Ignored by client simulation
                                intents: stamped_intents,
                            };
                            self.dispatch_sim_command(sow_core::protocol::SimCommand::Turn(turn));
                        }

                        if let Some(player) = self.sim.current_snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.id == self.sim.my_player_id.unwrap_or(1))) {
                            self.ui.app.hud_state.gold = player.gold;
                            self.ui.app.hud_state.troops = player.troops;
                            self.ui.app.hud_state.max_troops = player.max_troops;
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
                                    // If user is panning/zooming during the animation, abort the animation
                                    if self.input.dragging || self.input.last_pinch_state.is_some() || !self.input.active_touches.is_empty() {
                                        self.input.has_snapped_camera_to_spawn = true;
                                    } else {
                                        let target_world_cx = player.centroid_x + 0.5;
                                        let target_world_cy = player.centroid_y + 0.5;
                                        let target_zoom = 20.0;
                                        
                                        let current_world_cx = (self.input.screen_w * 0.5 - self.input.camera_x) / self.input.camera_zoom;
                                        let current_world_cy = (self.input.screen_h * 0.5 - self.input.camera_y) / self.input.camera_zoom;

                                        let speed = 0.01;
                                        let next_world_cx = current_world_cx + (target_world_cx - current_world_cx) * speed;
                                        let next_world_cy = current_world_cy + (target_world_cy - current_world_cy) * speed;
                                        let next_zoom = self.input.camera_zoom + (target_zoom - self.input.camera_zoom) * speed;

                                        self.input.camera_zoom = next_zoom;
                                        self.input.camera_x = self.input.screen_w * 0.5 - next_world_cx * next_zoom;
                                        self.input.camera_y = self.input.screen_h * 0.5 - next_world_cy * next_zoom;

                                        if (target_zoom - next_zoom).abs() < 0.2 && (target_world_cx - next_world_cx).abs() < 0.1 && (target_world_cy - next_world_cy).abs() < 0.1 {
                                            self.input.camera_zoom = target_zoom;
                                            self.input.camera_x = self.input.screen_w * 0.5 - target_world_cx * target_zoom;
                                            self.input.camera_y = self.input.screen_h * 0.5 - target_world_cy * target_zoom;
                                            self.input.has_snapped_camera_to_spawn = true;
                                            log::info!("Game started! Camera smoothly arrived at player spawn at ({}, {}), zoom={}", target_world_cx, target_world_cy, self.input.camera_zoom);
                                        }
                                        
                                        // Request redraw while animating
                                        if let Some(win) = self.gfx.window.as_ref() {
                                            win.request_redraw();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }


                // Periodic memory profiler print
                let now = web_time::Instant::now();
                if self.time.last_debug_print.is_none_or(|t| now.duration_since(t).as_secs() >= 5) {
                    self.time.last_debug_print = Some(now);
                    if let Some(snap) = &self.sim.current_snapshot {
                        if !snap.debug_mem_info.is_empty() {
                            log::info!("[MEM_PROFILER] Turn Queue: {} | Dirty Tiles: {} | {}", self.sim.turn_queue.len(), snap.dirty_tiles.len(), snap.debug_mem_info);
                        }
                    }
                }

    }
}
