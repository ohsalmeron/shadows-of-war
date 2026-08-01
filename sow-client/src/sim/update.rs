use crate::app::SowApp;
use web_time::Instant;

impl SowApp {
    pub fn update_sim(&mut self, now: Instant) {
        if let Some(snap) = &self.sim.current_snapshot {
            if let Some(target_secs) = snap.spawn_timer_secs {
                if let Some(ref mut current) = self.ui.app.hud_state.spawn_timer_secs {
                    if (*current - target_secs).abs() >= 0.1 {
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
        if self.ui.app.phase == sow_ui_kit::ClientPhase::Playing {
            if self.net.client.is_some() {
                // Multiplayer: lockstep execution dictated by server
                let mut ticks_processed = 0;
                while let Some(turn) = self.sim.turn_queue.pop_front() {
                    self.dispatch_sim_command(sow_core::protocol::SimCommand::Turn(turn));

                    // Update UI HUD State from my player id
                    self.sync_hud_player_state();
                    // Show notifications for actual resource transfers only
                    if let Some(snap) = &self.sim.current_snapshot {
                        let my_id = self.sim.my_player_id.unwrap_or(1);
                        for tx in &snap.resource_transfers {
                            if tx.receiver_id == my_id {
                                let sender_name = snap
                                    .players
                                    .iter()
                                    .find(|p| p.id == tx.sender_id)
                                    .map(|p| p.name.as_str())
                                    .unwrap_or("Ally");
                                let msg = match (tx.gold > 0.0, tx.troops > 0.0) {
                                    (true, true) => format!(
                                        "🎁 {} sent 🪙{} & ⚔️{}!",
                                        sender_name,
                                        sow_ui_kit::utils::format_number(tx.gold),
                                        sow_ui_kit::utils::format_number(tx.troops)
                                    ),
                                    (true, false) => format!(
                                        "🪙 {} sent +{} Gold!",
                                        sender_name,
                                        sow_ui_kit::utils::format_number(tx.gold)
                                    ),
                                    (false, true) => format!(
                                        "⚔️ {} sent +{} Troops!",
                                        sender_name,
                                        sow_ui_kit::utils::format_number(tx.troops)
                                    ),
                                    _ => continue,
                                };
                                self.ui
                                    .app
                                    .hud_state
                                    .push_notification(msg, egui::Color32::from_rgb(74, 222, 128));

                                // ponytail: reuse FloatingNotice to show transferred resources visually
                                let mut wx = 0.5;
                                let mut wy = 0.5;
                                if let Some(me) = snap.players.iter().find(|p| p.id == my_id) {
                                    if me.centroid_x > 0.001 || me.centroid_y > 0.001 {
                                        wx = me.centroid_x + 0.5;
                                        wy = me.centroid_y + 0.5;
                                    }
                                }
                                let now_instant = web_time::Instant::now();
                                if tx.gold > 0.0 {
                                    self.ui.floating_notices.push(crate::app::FloatingNotice {
                                        text: format!(
                                            "🪙 +{}",
                                            sow_ui_kit::utils::format_number(tx.gold)
                                        ),
                                        world_x: wx,
                                        world_y: wy,
                                        start_time: now_instant,
                                        duration: web_time::Duration::from_millis(3000),
                                        color: egui::Color32::from_rgb(250, 204, 21),
                                    });
                                }
                                if tx.troops > 0.0 {
                                    self.ui.floating_notices.push(crate::app::FloatingNotice {
                                        text: format!(
                                            "⚔️ +{}",
                                            sow_ui_kit::utils::format_number(tx.troops)
                                        ),
                                        world_x: wx,
                                        world_y: wy + 0.5,
                                        start_time: now_instant,
                                        duration: web_time::Duration::from_millis(3000),
                                        color: egui::Color32::from_rgb(6, 182, 212), // cyan
                                    });
                                }
                            } else if tx.sender_id == my_id {
                                let receiver_name = snap
                                    .players
                                    .iter()
                                    .find(|p| p.id == tx.receiver_id)
                                    .map(|p| p.name.as_str())
                                    .unwrap_or("Ally");
                                let msg = match (tx.gold > 0.0, tx.troops > 0.0) {
                                    (true, true) => format!(
                                        "Sent 🪙{} & ⚔️{} to {}",
                                        sow_ui_kit::utils::format_number(tx.gold),
                                        sow_ui_kit::utils::format_number(tx.troops),
                                        receiver_name
                                    ),
                                    (true, false) => format!(
                                        "Sent 🪙{} Gold to {}",
                                        sow_ui_kit::utils::format_number(tx.gold),
                                        receiver_name
                                    ),
                                    (false, true) => format!(
                                        "Sent ⚔️{} Troops to {}",
                                        sow_ui_kit::utils::format_number(tx.troops),
                                        receiver_name
                                    ),
                                    _ => continue,
                                };
                                self.ui
                                    .app
                                    .hud_state
                                    .push_notification(msg, egui::Color32::from_rgb(220, 220, 220));
                            }
                        }
                        for rej in &snap.resource_rejections {
                            if rej.requester_id == my_id {
                                let rejector_name = snap
                                    .players
                                    .iter()
                                    .find(|p| p.id == rej.rejector_id)
                                    .map(|p| p.name.as_str())
                                    .unwrap_or("Ally");
                                let msg =
                                    format!("❌ {} rejected your request for help!", rejector_name);
                                self.ui
                                    .app
                                    .hud_state
                                    .push_notification(msg, egui::Color32::from_rgb(239, 68, 68));
                            }
                        }
                    }
                    self.sync_building_costs();

                    ticks_processed += 1;
                    if ticks_processed >= 10 {
                        break;
                    }
                }
            } else {
                // Singleplayer: pace ticks from wall clock + lobby tick_rate_ms (same as relay).
                const MAX_OFFLINE_TICKS_PER_FRAME: u32 = 10;
                const MAX_OFFLINE_CATCHUP_SECS: f32 = 0.25;

                let tick_secs = (self.sim.config.tick_rate_ms / 1000.0).max(0.001);
                let dt = now
                    .duration_since(self.sim.offline_last_update)
                    .as_secs_f32()
                    .min(MAX_OFFLINE_CATCHUP_SECS);
                self.sim.offline_last_update = now;
                self.sim.offline_tick_timer += dt;

                // Pause: stop advancing the sim (nobody moves) but keep rendering.
                if self.sim.paused {
                    self.sim.offline_tick_timer = 0.0;
                }

                let mut ticks_this_frame = 0u32;
                while self.sim.offline_tick_timer >= tick_secs
                    && ticks_this_frame < MAX_OFFLINE_TICKS_PER_FRAME
                {
                    self.sim.offline_tick_timer -= tick_secs;
                    ticks_this_frame += 1;

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

                self.sync_hud_player_state();
                // Show notifications for actual resource transfers only
                if let Some(snap) = &self.sim.current_snapshot {
                    let my_id = self.sim.my_player_id.unwrap_or(1);
                    for tx in &snap.resource_transfers {
                        if tx.receiver_id == my_id {
                            let sender_name = snap
                                .players
                                .iter()
                                .find(|p| p.id == tx.sender_id)
                                .map(|p| p.name.as_str())
                                .unwrap_or("Ally");
                            let msg = match (tx.gold > 0.0, tx.troops > 0.0) {
                                (true, true) => format!(
                                    "🎁 {} sent 🪙{} & ⚔️{}!",
                                    sender_name,
                                    sow_ui_kit::utils::format_number(tx.gold),
                                    sow_ui_kit::utils::format_number(tx.troops)
                                ),
                                (true, false) => format!(
                                    "🪙 {} sent +{} Gold!",
                                    sender_name,
                                    sow_ui_kit::utils::format_number(tx.gold)
                                ),
                                (false, true) => format!(
                                    "⚔️ {} sent +{} Troops!",
                                    sender_name,
                                    sow_ui_kit::utils::format_number(tx.troops)
                                ),
                                _ => continue,
                            };
                            self.ui
                                .app
                                .hud_state
                                .push_notification(msg, egui::Color32::from_rgb(74, 222, 128));

                            // ponytail: reuse FloatingNotice to show transferred resources visually
                            let mut wx = 0.5;
                            let mut wy = 0.5;
                            if let Some(me) = snap.players.iter().find(|p| p.id == my_id) {
                                if me.centroid_x > 0.001 || me.centroid_y > 0.001 {
                                    wx = me.centroid_x + 0.5;
                                    wy = me.centroid_y + 0.5;
                                }
                            }
                            let now_instant = web_time::Instant::now();
                            if tx.gold > 0.0 {
                                self.ui.floating_notices.push(crate::app::FloatingNotice {
                                    text: format!(
                                        "🪙 +{}",
                                        sow_ui_kit::utils::format_number(tx.gold)
                                    ),
                                    world_x: wx,
                                    world_y: wy,
                                    start_time: now_instant,
                                    duration: web_time::Duration::from_millis(3000),
                                    color: egui::Color32::from_rgb(250, 204, 21),
                                });
                            }
                            if tx.troops > 0.0 {
                                self.ui.floating_notices.push(crate::app::FloatingNotice {
                                    text: format!(
                                        "⚔️ +{}",
                                        sow_ui_kit::utils::format_number(tx.troops)
                                    ),
                                    world_x: wx,
                                    world_y: wy + 0.5,
                                    start_time: now_instant,
                                    duration: web_time::Duration::from_millis(3000),
                                    color: egui::Color32::from_rgb(6, 182, 212), // cyan
                                });
                            }
                        } else if tx.sender_id == my_id {
                            let receiver_name = snap
                                .players
                                .iter()
                                .find(|p| p.id == tx.receiver_id)
                                .map(|p| p.name.as_str())
                                .unwrap_or("Ally");
                            let msg = match (tx.gold > 0.0, tx.troops > 0.0) {
                                (true, true) => format!(
                                    "Sent 🪙{} & ⚔️{} to {}",
                                    sow_ui_kit::utils::format_number(tx.gold),
                                    sow_ui_kit::utils::format_number(tx.troops),
                                    receiver_name
                                ),
                                (true, false) => format!(
                                    "Sent 🪙{} Gold to {}",
                                    sow_ui_kit::utils::format_number(tx.gold),
                                    receiver_name
                                ),
                                (false, true) => format!(
                                    "Sent ⚔️{} Troops to {}",
                                    sow_ui_kit::utils::format_number(tx.troops),
                                    receiver_name
                                ),
                                _ => continue,
                            };
                            self.ui
                                .app
                                .hud_state
                                .push_notification(msg, egui::Color32::from_rgb(220, 220, 220));
                        }
                    }
                    for rej in &snap.resource_rejections {
                        if rej.requester_id == my_id {
                            let rejector_name = snap
                                .players
                                .iter()
                                .find(|p| p.id == rej.rejector_id)
                                .map(|p| p.name.as_str())
                                .unwrap_or("Ally");
                            let msg =
                                format!("❌ {} rejected your request for help!", rejector_name);
                            self.ui
                                .app
                                .hud_state
                                .push_notification(msg, egui::Color32::from_rgb(239, 68, 68));
                        }
                    }
                }
                self.sync_building_costs();
            }
        }
        if self.ui.app.phase == sow_ui_kit::ClientPhase::Playing
            && !self.input.has_snapped_camera_to_spawn
        {
            if let Some(pid) = self.sim.my_player_id {
                if let Some(snap) = &self.sim.current_snapshot {
                    if let Some(player) = snap.players.iter().find(|p| p.id == pid) {
                        let is_playing = matches!(snap.phase, sow_core::game::GamePhase::Playing);
                        if player.tile_count > 0 && player.alive && is_playing {
                            // If user is panning/zooming during the animation, abort the animation
                            if self.input.dragging
                                || self.input.last_pinch_state.is_some()
                                || !self.input.active_touches.is_empty()
                            {
                                self.input.has_snapped_camera_to_spawn = true;
                            } else {
                                let cx = player.centroid_x;
                                let cy = player.centroid_y;
                                let target_world_cx = cx + 0.5;
                                let target_world_cy = cy + 0.5;
                                let target_zoom = 20.0;

                                let current_world_cx = (self.input.screen_w * 0.5
                                    - self.input.camera_x)
                                    / self.input.camera_zoom;
                                let current_world_cy = (self.input.screen_h * 0.5
                                    - self.input.camera_y)
                                    / self.input.camera_zoom;

                                let speed = 0.01;
                                let next_world_cx =
                                    current_world_cx + (target_world_cx - current_world_cx) * speed;
                                let next_world_cy =
                                    current_world_cy + (target_world_cy - current_world_cy) * speed;
                                let next_zoom = self.input.camera_zoom
                                    + (target_zoom - self.input.camera_zoom) * speed;

                                self.input.camera_zoom = next_zoom;
                                self.input.target_zoom = next_zoom;
                                self.input.camera_x =
                                    self.input.screen_w * 0.5 - next_world_cx * next_zoom;
                                self.input.camera_y =
                                    self.input.screen_h * 0.5 - next_world_cy * next_zoom;

                                if (target_zoom - next_zoom).abs() < 0.2
                                    && (target_world_cx - next_world_cx).abs() < 0.1
                                    && (target_world_cy - next_world_cy).abs() < 0.1
                                {
                                    self.input.camera_zoom = target_zoom;
                                    self.input.target_zoom = target_zoom;
                                    self.input.camera_x =
                                        self.input.screen_w * 0.5 - target_world_cx * target_zoom;
                                    self.input.camera_y =
                                        self.input.screen_h * 0.5 - target_world_cy * target_zoom;
                                    self.input.has_snapped_camera_to_spawn = true;
                                    log::info!(
                                        "Game started! Camera smoothly arrived at player spawn at ({}, {}), zoom={}",
                                        target_world_cx,
                                        target_world_cy,
                                        self.input.camera_zoom
                                    );
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
        if self
            .time
            .last_debug_print
            .is_none_or(|t| now.duration_since(t).as_secs() >= 5)
        {
            self.time.last_debug_print = Some(now);
            if let Some(snap) = &self.sim.current_snapshot {
                if !snap.debug_mem_info.is_empty() {
                    log::info!(
                        "[MEM_PROFILER] Turn Queue: {} | Dirty Tiles: {} | {}",
                        self.sim.turn_queue.len(),
                        snap.dirty_tiles.len(),
                        snap.debug_mem_info
                    );
                }
            }
        }
    }

    fn sync_building_costs(&mut self) {
        let snap_tick = self.sim.current_snapshot.as_ref().map(|s| s.tick);
        if snap_tick.is_some() && snap_tick == self.sim.last_synced_cost_tick {
            return;
        }
        self.sim.last_synced_cost_tick = snap_tick;

        let my_player_id = self.sim.my_player_id.unwrap_or(1);
        let buildings = self.sim.current_snapshot.as_ref().map(|s| &s.buildings);

        for i in 0..self.ui.app.hud_state.building_costs.len() {
            if let Some(&kind) = sow_core::game::BuildingKind::ALL.get(i) {
                let count = if let Some(b_list) = buildings {
                    b_list
                        .iter()
                        .filter(|b| b.owner_id == my_player_id && b.kind == kind)
                        .map(|b| b.level as u32)
                        .sum()
                } else {
                    0
                };
                self.ui.app.hud_state.building_costs[i] =
                    sow_core::building::structure_build_cost_gold(kind, count, &self.sim.config);
            } else {
                self.ui.app.hud_state.building_costs[i] = self.sim.config.cost_city;
            }
        }
    }

    pub(crate) fn sync_hud_player_state(&mut self) {
        if let Some(player) = self.sim.current_snapshot.as_ref().and_then(|s| {
            s.players
                .iter()
                .find(|p| p.id == self.sim.my_player_id.unwrap_or(1))
        }) {
            self.ui.app.hud_state.gold = player.gold;
            self.ui.app.hud_state.troops = player.troops;
            self.ui.app.hud_state.max_troops = player.max_troops;

            // Compute correct actual troop rate
            if let Some(e) = &self.sim.engine {
                let my_pid = self.sim.my_player_id.unwrap_or(1);
                let agg = e
                    .building_aggregates
                    .get(my_pid as usize)
                    .copied()
                    .unwrap_or_default();
                self.ui.app.hud_state.troop_rate =
                    sow_core::execution::income_rates::troop_income_per_second(
                        player.tile_count,
                        agg,
                        player.leader,
                        &self.sim.config,
                    ) * self.sim.config.global_speed_multiplier;
            }
        }
    }
}
