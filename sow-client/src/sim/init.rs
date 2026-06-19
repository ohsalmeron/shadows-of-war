use crate::app::SowApp;

impl SowApp {
    pub(crate) fn handle_sim_init(
        &mut self,
        config: Box<sow_core::game_config::GameConfig>,
        seed: u64,
        map_bytes: Vec<u8>,
        players: Vec<sow_core::protocol::PlayerInfo>,
    ) {
        self.reset_progress_session();
        self.sim.config = (*config).clone();
        let map_w = config.map_width;
        let map_h = config.map_height;
        let mut state = sow_core::game::GameState::new(seed, map_w, map_h, *config);

        if let Ok(map_file) = sow_core::maps::load_map_from_payload(&map_bytes) {
            state.total_land_tiles = map_file.num_land_tiles;
            state.map_spawns = map_file.spawns;
            if map_file.terrain.len() == state.map.terrain.len() {
                let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        map_file.terrain.as_ptr(),
                        dest_ptr,
                        map_file.terrain.len(),
                    );
                }
            }
        } else if map_bytes.len() == state.map.terrain.len() {
            let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
            unsafe {
                std::ptr::copy_nonoverlapping(map_bytes.as_ptr(), dest_ptr, map_bytes.len());
            }
        }

        let water = sow_core::water_components::WaterComponents::compute(&state.map, |_| {});
        let mut new_engine = sow_core::engine::SowEngine::new(state, water);

        for p in players {
            if p.player_type == sow_core::player::PlayerType::Human {
                new_engine.spawn_human(p.id, p.name, p.color, p.team, p.civilization, p.leader);
            }
        }

        new_engine.spawn_ai(
            new_engine.state.config.nation_count,
            new_engine.state.config.bot_count,
        );
        let snap = new_engine.build_snapshot();
        self.sim.current_snapshot = Some(snap);
        self.sim.engine = Some(new_engine);
        self.sim.tile_upgrades = vec![0; (map_w * map_h) as usize];
        self.time
            .interp
            .set_tick_dur_ms(self.sim.config.tick_rate_ms);
        self.time.interp.stamp_applied(web_time::Instant::now());
        self.sim.offline_tick_timer = 0.0;
        self.sim.offline_last_update = web_time::Instant::now();
        self.ui.mover_scene = crate::render::world::movers::MoverScene::new();

        self.input.camera_zoom = 0.5;
        self.input.target_zoom = 0.5;
        self.input.camera_x =
            self.input.screen_w * 0.5 - (map_w as f32 * 0.5) * self.input.camera_zoom;
        self.input.camera_y =
            self.input.screen_h * 0.5 - (map_h as f32 * 0.5) * self.input.camera_zoom;
        self.input.has_snapped_camera_to_spawn = false;
        self.ui.is_spectating = false;
        self.ui.endgame_cache = None;
        sow_audio::set_music_context(seed as u32, map_w as f32 * 0.5, map_h as f32 * 0.5);
    }
}
