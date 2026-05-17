use sow_core::protocol::{SimCommand, SimSnapshot};

pub trait SimBridge {
    fn send_command(&self, cmd: SimCommand);
    fn try_recv_snapshot(&self) -> Option<SimSnapshot>;
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeSimBridge as PlatformSimBridge;


#[cfg(not(target_arch = "wasm32"))]
pub mod native {
    use super::*;
    use sow_core::engine::SowEngine;
    use crossbeam_channel::{Receiver, Sender};
    use std::thread;

    pub struct NativeSimBridge {
        cmd_tx: Sender<SimCommand>,
        snap_rx: Receiver<SimSnapshot>,
    }

    impl NativeSimBridge {
        pub fn spawn() -> Self {
            let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
            let (snap_tx, snap_rx) = crossbeam_channel::unbounded();

            thread::spawn(move || {
                let mut engine: Option<SowEngine> = None;
                
                // Track when to send snapshots. We want to send one per tick.
                // Or rather, we process all available commands, then if we ticked, we snapshot.
                
                let init_engine = |config: sow_core::game_config::GameConfig, seed: u64, map_bytes: Vec<u8>, players: Vec<sow_core::protocol::PlayerInfo>| -> SowEngine {
                    let map_w = config.map_width;
                    let map_h = config.map_height;
                    let mut state = sow_core::game::GameState::new(seed, map_w, map_h, config);
                    
                    if map_bytes.len() == state.map.terrain.len() {
                        let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
                        unsafe { std::ptr::copy_nonoverlapping(map_bytes.as_ptr(), dest_ptr, map_bytes.len()); }
                    } else {
                        for (i, &b) in map_bytes.iter().enumerate() {
                            if i < state.map.terrain.len() {
                                state.map.terrain[i] = sow_core::map::MapTile::from_byte(b);
                            }
                        }
                    }

                    let water = sow_core::water_components::WaterComponents::compute(&state.map, |_| {});
                    let mut new_engine = SowEngine::new(state, water);

                    for p in players {
                        if p.player_type == sow_core::player::PlayerType::Human {
                            new_engine.spawn_human(p.id, p.name, p.color);
                        }
                    }
                    
                    new_engine.spawn_ai(new_engine.state.config.nation_count, new_engine.state.config.bot_count);
                    new_engine
                };

                loop {
                    let mut processed_commands = false;
                    
                    if engine.is_none() {
                        match cmd_rx.recv() {
                            Ok(SimCommand::Init { config, seed, map_bytes, players }) => {
                                let mut e = init_engine(config, seed, map_bytes, players);
                                let snapshot = e.build_snapshot();
                                let _ = snap_tx.send(snapshot);
                                engine = Some(e);
                            }
                            Ok(SimCommand::Shutdown) => break,
                            Ok(SimCommand::Turn(_)) => {
                                log::warn!("Received Turn before Init in SimBridge");
                            }
                            Err(_) => break,
                        }
                    } else {
                        while let Ok(cmd) = cmd_rx.try_recv() {
                            match cmd {
                                SimCommand::Init { config, seed, map_bytes, players } => {
                                    log::info!("Re-initializing Native SimWorker");
                                    let mut e = init_engine(config, seed, map_bytes, players);
                                    let snapshot = e.build_snapshot();
                                    let _ = snap_tx.send(snapshot);
                                    engine = Some(e);
                                }
                                SimCommand::Turn(turn) => {
                                    if let Some(e) = &mut engine {
                                        e.apply_intents(&turn.intents);
                                        e.tick();
                                        processed_commands = true;
                                    }
                                }
                                SimCommand::Shutdown => return,
                            }
                        }
                        
                        if processed_commands {
                            if let Some(e) = &mut engine {
                                let snapshot = e.build_snapshot();

                                let _ = snap_tx.send(snapshot);
                            }
                        } else {
                            // Yield so we don't spin lock
                            thread::sleep(std::time::Duration::from_millis(1));
                        }
                    }
                }
            });

            Self { cmd_tx, snap_rx }
        }
    }

    impl SimBridge for NativeSimBridge {
        fn send_command(&self, cmd: SimCommand) {
            let _ = self.cmd_tx.send(cmd);
        }

        fn try_recv_snapshot(&self) -> Option<SimSnapshot> {
            // Get the latest snapshot, dropping older ones, but merging dirty tiles
            let mut latest: Option<SimSnapshot> = None;
            while let Ok(mut snap) = self.snap_rx.try_recv() {
                if let Some(mut existing) = latest {
                    if !existing.dirty_tiles.is_empty() {
                        existing.dirty_tiles.append(&mut snap.dirty_tiles);
                        snap.dirty_tiles = existing.dirty_tiles;
                    }
                }
                latest = Some(snap);
            }
            latest
        }
    }
}


#[cfg(target_arch = "wasm32")]
pub use wasm::SyncSimBridge as PlatformSimBridge;

#[cfg(target_arch = "wasm32")]
pub mod wasm {
    use super::*;
    use sow_core::engine::SowEngine;
    use std::cell::RefCell;

    pub struct SyncSimBridge {
        engine: RefCell<Option<SowEngine>>,
        latest_snapshot: RefCell<Option<SimSnapshot>>,
        snapshot_dirty: std::cell::Cell<bool>,
    }

    impl SyncSimBridge {
        pub fn spawn() -> Self {
            Self {
                engine: RefCell::new(None),
                latest_snapshot: RefCell::new(None),
                snapshot_dirty: std::cell::Cell::new(false),
            }
        }
    }

    impl SimBridge for SyncSimBridge {
        fn send_command(&self, cmd: SimCommand) {
            match cmd {
                SimCommand::Init { config, seed, map_bytes, players } => {
                    let map_w = config.map_width;
                    let map_h = config.map_height;
                    let mut state = sow_core::game::GameState::new(seed, map_w, map_h, config);
                    
                    if map_bytes.len() == state.map.terrain.len() {
                        let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
                        unsafe { std::ptr::copy_nonoverlapping(map_bytes.as_ptr(), dest_ptr, map_bytes.len()); }
                    } else {
                        for (i, &b) in map_bytes.iter().enumerate() {
                            if i < state.map.terrain.len() {
                                state.map.terrain[i] = sow_core::map::MapTile::from_byte(b);
                            }
                        }
                    }

                    let water = sow_core::water_components::WaterComponents::compute(&state.map, |_| {});
                    let mut new_engine = SowEngine::new(state, water);

                    for p in players {
                        if p.player_type == sow_core::player::PlayerType::Human {
                            new_engine.spawn_human(p.id, p.name, p.color);
                        }
                    }
                    
                    new_engine.spawn_ai(new_engine.state.config.nation_count, new_engine.state.config.bot_count);
                    
                    let snap = new_engine.build_snapshot();
                    *self.latest_snapshot.borrow_mut() = Some(snap);
                    *self.engine.borrow_mut() = Some(new_engine);
                }
                SimCommand::Turn(turn) => {
                    if let Some(e) = self.engine.borrow_mut().as_mut() {
                        e.apply_intents(&turn.intents);
                        e.tick();
                        self.snapshot_dirty.set(true);
                    }
                }
                SimCommand::Shutdown => {
                    *self.engine.borrow_mut() = None;
                    *self.latest_snapshot.borrow_mut() = None;
                }
            }
        }

        fn try_recv_snapshot(&self) -> Option<SimSnapshot> {
            if self.snapshot_dirty.get() {
                self.snapshot_dirty.set(false);
                if let Some(e) = self.engine.borrow_mut().as_mut() {
                    let snap = e.build_snapshot();
                    *self.latest_snapshot.borrow_mut() = Some(snap);
                }
            }
            self.latest_snapshot.borrow_mut().take()
        }
    }
}

use crate::app::SowApp;
use web_time::Instant;

impl SowApp {
    pub fn update_sim(&mut self, now: Instant) {
                self.app.hud_state.is_mobile = self.screen_w < 900.0;
                if let Some(snap) = &self.current_snapshot {
                    if let Some(target_secs) = snap.spawn_timer_secs {
                        if let Some(ref mut current) = self.app.hud_state.spawn_timer_secs {
                            if (*current - target_secs).abs() > 0.3 {
                                *current = target_secs;
                            }
                        } else {
                            self.app.hud_state.spawn_timer_secs = Some(target_secs);
                        }
                    } else {
                        self.app.hud_state.spawn_timer_secs = None;
                    }
                } else {
                    self.app.hud_state.spawn_timer_secs = None;
                }
                if self.app.phase == sow_ui::app::ClientPhase::Playing {
                    if self.net_client.is_some() {
                        // Multiplayer: lockstep execution dictated by server
                        let mut ticks_processed = 0;
                        while let Some(turn) = self.turn_queue.pop_front() {
                            self.bridge.send_command(SimCommand::Turn(turn));
                            
                            // Update UI HUD State from my player id
                            if let Some(player) = self.current_snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.id == self.my_player_id.unwrap_or(1))) {
                                self.app.hud_state.gold = player.gold;
                                self.app.hud_state.troops = player.troops;
                                let owned_tiles = player.tile_count as f64;
                                self.app.hud_state.max_troops = owned_tiles * 50.0;
                            }

                            ticks_processed += 1;
                            if ticks_processed >= 10 {
                                break;
                            }
                        }
                        self.last_tick = now;
                    } else {
                        // Singleplayer: HUD updates based on local timer (ticks are handled by mod.rs)
                        if now.duration_since(self.last_tick) >= self.tick_interval {
                            self.last_tick = now;
                            
                            if let Some(player) = self.current_snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.id == self.my_player_id.unwrap_or(1))) {
                                self.app.hud_state.gold = player.gold;
                                self.app.hud_state.troops = player.troops;
                                let owned_tiles = player.tile_count as f64;
                                self.app.hud_state.max_troops = owned_tiles * 50.0;
                            }
                        }
                    }
                } else {
                    self.last_tick = now;
                }
                if let Some(mut snap) = self.bridge.try_recv_snapshot() {

                    if let Some(mut existing) = self.current_snapshot.take() {
                        if !existing.dirty_tiles.is_empty() {
                            existing.dirty_tiles.append(&mut snap.dirty_tiles);
                            snap.dirty_tiles = existing.dirty_tiles;
                        }
                    }
                    
                    
                    self.current_snapshot = Some(snap);
                }
                    
                if self.app.splash_state.gpu_load_step == 3 && self.current_snapshot.is_some() {
                    self.app.splash_state.gpu_load_step = 4;
                    self.app.phase = sow_ui::app::ClientPhase::Playing;
                    
                    // Clear pending init data to completely finish EnterGame phase
                    self.pending_engine_init_data = None;
                    log::info!("First snapshot received, releasing loader!");
                    
                    if let Some(pid) = self.my_player_id {
                        if let Some(snap) = &self.current_snapshot {
                            if let Some(player) = snap.players.iter().find(|p| p.id == pid) {
                                if player.tile_count > 0 && player.alive {
                                    let cx = player.centroid_x;
                                    let cy = player.centroid_y;
                                    self.camera_zoom = 1.5;
                                    self.camera_x = self.screen_w * 0.5 - cx * self.camera_zoom;
                                    self.camera_y = self.screen_h * 0.5 - cy * self.camera_zoom;
                                }
                            }
                        }
                    }

                    if let Some(c) = self.net_client.as_ref() {
                        if let (Some(lid), Some(pid)) = (self.my_lobby_id, self.my_player_id) {
                            let ready_msg = sow_core::protocol::ClientMessage::Ready { lobby_id: lid, player_id: pid };
                            let json = bincode::serialize(&ready_msg).unwrap();
                            c.send(json);
                        }
                    }
                }
                
                if let Some(win) = self.window.as_ref() {
                    win.request_redraw();
                }

                // Periodic memory profiler print
                let now = web_time::Instant::now();
                if self.last_debug_print.is_none_or(|t| now.duration_since(t).as_secs() >= 5) {
                    self.last_debug_print = Some(now);
                    if let Some(snap) = &self.current_snapshot {
                        log::info!("[MEM_PROFILER] Turn Queue: {} | Dirty Tiles: {} | {}", self.turn_queue.len(), snap.dirty_tiles.len(), snap.debug_mem_info);
                    }
                }

    }
}
