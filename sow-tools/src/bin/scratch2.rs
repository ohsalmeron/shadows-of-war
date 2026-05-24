use sow_core::engine::SowEngine;
use sow_core::game::{GamePhase, GameState};
use sow_core::game_config::GameConfig;
use sow_core::water_components::WaterComponents;

fn main() {
    let mut config = GameConfig::default();
    config.map_width = 2000;
    config.map_height = 1000;

    let state = GameState::new(42, 2000, 1000, config.clone());
    let water = WaterComponents::default();
    let mut engine = SowEngine::new(state, water);

    // Fill the map with some land so spawn works
    for i in 0..2000 * 1000 {
        // Set all to land
        engine.state.map.terrain[i] = sow_core::map::MapTile::from_byte(0b10000000);
    }

    // Spawn human
    engine.spawn_human(
        1,
        "Bizkit".to_string(),
        [1.0, 0.0, 0.0],
        None,
        sow_core::player::Civilization::Rome,
        sow_core::player::Leader::Caesar,
    );

    // If it's spawning phase, let's fast forward so they actually spawn
    if let GamePhase::Spawning { end_tick } = engine.state.phase {
        for _ in 0..end_tick {
            engine.tick();
        }
    }

    let p = engine.state.player(1).unwrap();
    println!("Spawned Player!");
    println!("Tiles Owned: {}", p.tile_count);
    println!("Base Max Troops: {}", p.max_troops);
    println!("Cities: {}", p.cities);

    // Also, let's print out what the bot counts do
    // Because bots spawn too!
}
