use wasm_bindgen::prelude::*;
use js_sys::Uint8Array;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};
use sow_core::engine::SowEngine;
use sow_core::protocol::SimCommand;

// Thread-local variables are okay here because Web Workers are strictly single-threaded
thread_local! {
    static ENGINE: std::cell::RefCell<Option<SowEngine>> = std::cell::RefCell::new(None);
}

#[wasm_bindgen(start)]
pub fn main_js() -> Result<(), JsValue> {
    console_log::init_with_level(log::Level::Info).expect("error initializing logger");
    log::info!("SOW Sim Worker spawned!");

    // Get the global worker scope
    let global = js_sys::global().unchecked_into::<DedicatedWorkerGlobalScope>();

    // Create the onmessage handler
    let onmessage_callback = Closure::wrap(Box::new(move |event: MessageEvent| {
        let array = Uint8Array::new(&event.data());
        let mut bytes = vec![0; array.length() as usize];
        array.copy_to(&mut bytes);

        match bincode::deserialize::<SimCommand>(&bytes) {
            Ok(SimCommand::Init { config, seed, map_bytes, players }) => {
                log::debug!("Sim Worker received Init: map size {}x{}", config.map_width, config.map_height);
                let mut state = sow_core::game::GameState::new(seed, config.map_width, config.map_height, config);
                
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

                // Build and send the initial snapshot to break the loader deadlock
                let snapshot = new_engine.build_snapshot();
                if let Ok(snap_bytes) = bincode::serialize(&snapshot) {
                    let array = Uint8Array::from(&snap_bytes[..]);
                    let global = js_sys::global().unchecked_into::<DedicatedWorkerGlobalScope>();
                    let _ = global.post_message(&array);
                }

                ENGINE.with(|e| *e.borrow_mut() = Some(new_engine));
            }
            Ok(SimCommand::Turn(turn)) => {
                ENGINE.with(|e_cell| {
                    if let Some(e) = e_cell.borrow_mut().as_mut() {
                        for intent in &turn.intents {
                            e.apply_stamped_intent(intent, 0);
                        }
                        e.tick();

                        // Build and send snapshot back
                        let snapshot = e.build_snapshot();
                        if let Ok(snap_bytes) = bincode::serialize(&snapshot) {
                            let array = Uint8Array::from(&snap_bytes[..]);
                            let global = js_sys::global().unchecked_into::<DedicatedWorkerGlobalScope>();
                            let _ = global.post_message(&array);
                        }
                    }
                });
            }
            Ok(SimCommand::Shutdown) => {
                let global = js_sys::global().unchecked_into::<DedicatedWorkerGlobalScope>();
                global.close();
            }
            Err(e) => {
                log::error!("Failed to deserialize SimCommand in worker: {:?}", e);
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    global.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
    onmessage_callback.forget(); // Keep closure alive

    Ok(())
}
