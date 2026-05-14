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
                                engine = Some(init_engine(config, seed, map_bytes, players));
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
                                    engine = Some(init_engine(config, seed, map_bytes, players));
                                }
                                SimCommand::Turn(turn) => {
                                    if let Some(e) = &mut engine {
                                        for intent in &turn.intents {
                                            e.apply_stamped_intent(intent, 0);
                                        }
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
                                // Drop old snapshots if we are falling behind.
                                // It's a jitter buffer.
                                while snap_tx.len() > 1 {
                                    // we can't easily drain from tx, but we can just let it queue and let the receiver drain.
                                    // But wait, the Receiver is draining in `try_recv_snapshot` anyway!
                                    // Let's just let it be.
                                }
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
            // Get the latest snapshot, dropping older ones
            let mut latest = None;
            while let Ok(snap) = self.snap_rx.try_recv() {
                latest = Some(snap);
            }
            latest
        }
    }
}


#[cfg(target_arch = "wasm32")]
pub use wasm::WasmSimBridge as PlatformSimBridge;

#[cfg(target_arch = "wasm32")]
pub mod wasm {
    use super::*;
    use web_sys::{Worker, MessageEvent};
    use js_sys::Uint8Array;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use std::cell::RefCell;
    use std::rc::Rc;

    pub struct WasmSimBridge {
        worker: Worker,
        latest_snapshot: Rc<RefCell<Option<SimSnapshot>>>,
    }

    impl WasmSimBridge {
        pub fn spawn() -> Self {
            let worker = Worker::new("/assets/sow_sim_worker_boot.js").expect("failed to load worker script");
            let latest_snapshot = Rc::new(RefCell::new(None));
            let latest_snapshot_clone = latest_snapshot.clone();

            let onmessage_callback = Closure::wrap(Box::new(move |event: MessageEvent| {
                let array = Uint8Array::new(&event.data());
                let mut bytes = vec![0; array.length() as usize];
                array.copy_to(&mut bytes);
                
                if let Ok(snap) = bincode::deserialize::<SimSnapshot>(&bytes) {
                    *latest_snapshot_clone.borrow_mut() = Some(snap);
                }
            }) as Box<dyn FnMut(MessageEvent)>);

            worker.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
            onmessage_callback.forget();

            Self { worker, latest_snapshot }
        }
    }

    impl SimBridge for WasmSimBridge {
        fn send_command(&self, cmd: SimCommand) {
            if let SimCommand::Init { .. } = &cmd {
                self.latest_snapshot.borrow_mut().take();
            }
            if let Ok(bytes) = bincode::serialize(&cmd) {
                let array = Uint8Array::from(&bytes[..]);
                let _ = self.worker.post_message(&array);
            }
        }

        fn try_recv_snapshot(&self) -> Option<SimSnapshot> {
            self.latest_snapshot.borrow_mut().take()
        }
    }
}
