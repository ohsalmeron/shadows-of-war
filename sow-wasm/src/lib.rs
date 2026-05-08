use sow_core::engine::SowEngine;
use sow_core::game::GameState;
use sow_core::game_config::GameConfig;
use sow_core::water_components::WaterComponents;
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[derive(Serialize, Deserialize)]
pub struct WorkerMsg {
    pub message_type: String,
    pub payload: Option<String>,
}

#[wasm_bindgen]
pub struct SimulationWorker {
    engine: SowEngine,
}

#[wasm_bindgen]
impl SimulationWorker {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let config = GameConfig::default();
        // create a small 800x600 map
        let state = GameState::new(12345, 800, 600, config);
        let water = WaterComponents::compute(&state.map);
        let engine = SowEngine::new(state, water);

        Self { engine }
    }

    pub fn spawn_human(&mut self, player_id: u16) {
        self.engine.spawn_human(player_id);
    }

    pub fn spawn_random_bots(&mut self, bot_count: u32) {
        self.engine.spawn_random_bots(bot_count);
    }

    pub fn map_width(&self) -> u32 {
        self.engine.state.map.width
    }

    pub fn map_height(&self) -> u32 {
        self.engine.state.map.height
    }

    pub fn map_state_ptr(&self) -> *const u16 {
        self.engine.state.map.state.as_ptr()
    }

    pub fn map_terrain_ptr(&self) -> *const u8 {
        // terrain is Vec<MapTile>, MapTile is u8 wrapper
        self.engine.state.map.terrain.as_ptr() as *const u8
    }

    pub fn human_troops(&self) -> f64 {
        self.engine.state.player_lookup.get(1)
            .and_then(|&i| i)
            .map(|idx| self.engine.state.players[idx].troops)
            .unwrap_or(0.0)
    }

    pub fn human_gold(&self) -> f64 {
        self.engine.state.player_lookup.get(1)
            .and_then(|&i| i)
            .map(|idx| self.engine.state.players[idx].gold)
            .unwrap_or(0.0)
    }

    pub fn human_max_troops(&self) -> f64 {
        self.engine.state.player_lookup.get(1)
            .and_then(|&i| i)
            .map(|idx| self.engine.state.players[idx].max_troops)
            .unwrap_or(0.0)
    }

    pub fn tick(&mut self) -> JsValue {
        self.engine.tick();
        serde_wasm_bindgen::to_value(&self.engine.state.tick).unwrap()
    }

    pub fn handle_intent(&mut self, intent_js: JsValue) {
        if let Ok(intent) = serde_wasm_bindgen::from_value::<sow_core::protocol::GameplayIntent>(intent_js) {
            let stamped = sow_core::protocol::StampedIntent {
                player_id: 1, // hardcode human as 1
                intent,
            };
            self.engine.apply_intents(&[stamped]);
        }
    }
}
