use crate::game::GameState;
use crate::execution::AttackExecution;
use crate::warp_fleet::WarpFleet;
use crate::building::{Building, BuildingGrid, DefenseGrid, BuildingAggregate};
use crate::pathfinding::WaterPathfinderScratch;
use crate::water_components::WaterComponents;

#[derive(Default, Clone)]
pub struct PlacementScratch {
    pub visited_stamp: Vec<u32>,
    pub stamp: u32,
    pub queue: Vec<u32>,
    pub border_scratch: Vec<(u32, u32)>,
}

#[derive(Clone)]
pub struct SowEngine {
    pub state: GameState,
    pub attacks: Vec<AttackExecution>,
    pub fleets: Vec<WarpFleet>,
    pub buildings: Vec<Building>,
    pub water: WaterComponents,
    pub path_scratch: WaterPathfinderScratch,
    pub placement_scratch: PlacementScratch,
    pub defense_grid: DefenseGrid,
    pub defense_grid_dirty: bool,
    pub render_defense_dirty: bool,
    pub building_grid: BuildingGrid,
    pub building_aggregates: Vec<BuildingAggregate>,
    pub building_aggregates_dirty: bool,
    /// Round-robin cursor for the unified AI pipeline.
    /// Ensures fair distribution of bot/nation think work across ticks.
    pub ai_round_robin: usize,
}

impl SowEngine {
    pub fn new(state: GameState, water: WaterComponents) -> Self {
        Self {
            state,
            attacks: Vec::new(),
            fleets: Vec::new(),
            buildings: Vec::new(),
            water,
            path_scratch: WaterPathfinderScratch::default(),
            placement_scratch: PlacementScratch::default(),
            defense_grid: DefenseGrid::default(),
            defense_grid_dirty: true,
            render_defense_dirty: true,
            building_grid: BuildingGrid::default(),
            building_aggregates: Vec::new(),
            building_aggregates_dirty: true,
            ai_round_robin: 0,
        }
    }

    pub fn refresh_building_grid(&mut self) {
        if !self.building_grid.dirty && self.building_grid.grid_w > 0 {
            return;
        }
        let w = self.state.map.width;
        let h = self.state.map.height;
        self.building_grid.rebuild(self.buildings.iter(), w, h);
    }

    pub fn kill_player(&mut self, player_id: u16) {
        if let Some(player) = self.state.player_mut(player_id) {
            player.alive = false;
        }
        let mut to_clear = Vec::new();
        for (i, &owner) in self.state.map.state.iter().enumerate() {
            if owner == player_id {
                let x = (i % self.state.map.width as usize) as u32;
                let y = (i / self.state.map.width as usize) as u32;
                to_clear.push((x, y));
            }
        }
        for (x, y) in to_clear {
            self.state.set_tile_owner(x, y, 0);
        }
        self.attacks.retain(|a| a.owner_id != player_id);
        self.fleets.retain(|f| f.owner_id != player_id);
    }

    #[inline]
    pub fn add_building(&mut self, b: Building) {
        let is_ready_defense =
            b.kind == crate::game::BuildingKind::DefensePost && !b.under_construction;
        let pos = self.buildings.partition_point(|x| x.id < b.id);
        self.buildings.insert(pos, b);
        self.building_grid.mark_dirty();
        self.building_aggregates_dirty = true;
        if is_ready_defense {
            self.defense_grid_dirty = true;
            self.render_defense_dirty = true;
        }
    }

    #[inline]
    pub fn add_attack(&mut self, a: AttackExecution) {
        let pos = self.attacks.partition_point(|x| x.id < a.id);
        self.attacks.insert(pos, a);
    }

    #[inline]
    pub fn add_fleet(&mut self, f: WarpFleet) {
        let pos = self.fleets.partition_point(|x| x.id < f.id);
        self.fleets.insert(pos, f);
    }

    pub fn tick(&mut self) {
        if let crate::game::GamePhase::Spawning { end_tick } = self.state.phase {
            self.state.tick += 1;
            if self.state.tick >= end_tick {
                self.state.phase = crate::game::GamePhase::Playing;
                // Auto-spawn players who missed the window
                let unspawned: Vec<u16> = self.state.players.iter()
                    .filter(|p| !p.has_spawned)
                    .map(|p| p.id)
                    .collect();
                
                for pid in unspawned {
                    use wyrand::WyRand;
                    let mut rng = WyRand::new(self.state.seed.wrapping_add(pid as u64));
                    if let Some((sx, sy)) = self.find_valid_spawn(&mut rng) {
                        self.state.place_spawn(pid, sx, sy);
                        log::info!("Auto-spawned missing player {} at {}, {}", pid, sx, sy);
                    }
                }
            }
            return;
        }

        self.state.events.clear(); // Prevent unbounded memory leak (was growing infinitely on tile capture)
        self.state.tick();
        self.execute_income();
        self.execute_ai_think();
        self.execute_construction();
        self.execute_combat();
        self.execute_fleets();
        self.check_winner();
    }
    
    fn check_winner(&mut self) {
        if self.state.winner.is_some() { return; }
        
        if self.state.total_land_tiles == 0 {
            self.state.total_land_tiles = self.state.map.terrain.iter().filter(|t| t.is_land()).count() as u32;
            if self.state.total_land_tiles == 0 {
                self.state.total_land_tiles = 1; // Prevent division by zero
            }
        }
        
        let win_threshold = (self.state.total_land_tiles as f32 * self.state.config.map_control_win_percentage) as u32;

        let mut alive_players = 0;
        let mut last_alive_id = None;
        let mut map_control_winner = None;
        
        for p in &self.state.players {
            if p.alive && p.tile_count > 0 {
                alive_players += 1;
                last_alive_id = Some(p.id);
                if p.tile_count >= win_threshold {
                    map_control_winner = Some(p.id);
                }
            }
        }
        
        if let Some(wid) = map_control_winner {
            self.state.winner = Some(wid);
            self.state.phase = crate::game::GamePhase::GameOver;
            self.state.events.push(crate::game::GameEvent::GameOver { winner_id: wid });
        } else if alive_players == 1 {
            self.state.winner = last_alive_id;
            self.state.phase = crate::game::GamePhase::GameOver;
            if let Some(wid) = last_alive_id {
                self.state.events.push(crate::game::GameEvent::GameOver { winner_id: wid });
            }
        } else if alive_players == 0 && !self.state.players.is_empty() {
            // Everyone died? Rare but possible.
            self.state.phase = crate::game::GamePhase::GameOver;
        }
    }
    pub fn spawn_ai(&mut self, nation_count: u32, tribe_count: u32) {
        let mut spawned_nations = 0;
        let mut spawned_tribes = 0;
        use wyrand::WyRand;
        use crate::player::Player;

        let mut rng = WyRand::new(self.state.seed);
        let config = self.state.config.clone();
        
        // Spawn Nations (IDs 104 to 199)
        for i in 0..nation_count {
            let bot_id = 104 + i as u16;
            if let Some((sx, sy)) = self.find_valid_spawn(&mut rng) {
                // Nations have a starting advantage? Or just distinct colors for now.
                let player = Player::new_bot(bot_id, format!("Nation {}", i+1), [0.8, 0.8, 0.8], &config);
                self.state.spawn_player(player, sx, sy);
                spawned_nations += 1;
            }
        }

        // Spawn Tribes (IDs 200+)
        for i in 0..tribe_count {
            let bot_id = 200 + i as u16;
            if let Some((sx, sy)) = self.find_valid_spawn(&mut rng) {
                let player = Player::new_bot(bot_id, format!("Tribe {}", i+1), [0.4, 0.4, 0.4], &config);
                self.state.spawn_player(player, sx, sy);
                spawned_tribes += 1;
            }
        }
        log::info!("Spawned {} nations and {} tribes successfully.", spawned_nations, spawned_tribes);
    }

    pub fn spawn_human(&mut self, player_id: u16, name: String, color: [f32; 3]) {
        use wyrand::WyRand;
        use crate::player::Player;

        // Use a different seed offset for human to avoid clashing exactly with bots
        let mut rng = WyRand::new(self.state.seed.wrapping_add(player_id as u64));
        let config = self.state.config.clone();
        
        if !config.random_spawn {
            let player = Player::new_human(player_id, name, color, &config);
            self.state.register_player(player);
            return;
        }

        if let Some((sx, sy)) = self.find_valid_spawn(&mut rng) {
            let player = Player::new_human(player_id, name, color, &config);
            self.state.spawn_player(player, sx, sy);
        } else {
            log::warn!("Failed to spawn Human {} - no room!", player_id);
        }
    }

    fn find_valid_spawn(&self, rng: &mut wyrand::WyRand) -> Option<(u32, u32)> {
        use crate::rng::NextIntExt;
        let mut tries = 0;

        while tries < 1000 {
            let sx = rng.next_int(0, self.state.map.width as i32) as u32;
            let sy = rng.next_int(0, self.state.map.height as i32) as u32;
            
            if self.state.map.terrain[self.state.map.ref_id(sx, sy)].is_water() {
                tries += 1;
                continue;
            }

            let mut valid = true;

            for dy in -15..=15 {
                for dx in -15..=15 {
                    let nx = sx as i32 + dx;
                    let ny = sy as i32 + dy;
                    if self.state.map.is_valid_coord(nx, ny) && self.state.map.owner_id(nx as u32, ny as u32) != 0 {
                        valid = false;
                        break;
                    }
                }
                if !valid { break; }
            }
            if valid { return Some((sx, sy)); }
            tries += 1;
        }
        None
    }

    /// Build a lightweight snapshot of the current state for the render thread.
    /// Drains `map.dirty_tiles` so each tile is reported exactly once.
    pub fn build_snapshot(&mut self) -> crate::protocol::SimSnapshot {
        let dirty_tiles: Vec<crate::protocol::DirtyTile> = self.state.map.dirty_tiles
            .drain(..)
            .map(|i| crate::protocol::DirtyTile {
                index: i as u32,
                new_owner: self.state.map.state[i],
            })
            .collect();

        let players = self.state.players.iter().map(|p| {
            let (cx, cy) = if p.tile_count > 0 {
                (
                    (p.sum_x / p.tile_count as u64) as f32,
                    (p.sum_y / p.tile_count as u64) as f32,
                )
            } else {
                (0.0, 0.0)
            };
            
            // Optimization: avoid string cloning and bincode serialization for 600+ bots every tick
            let name = if p.player_type == crate::player::PlayerType::Human {
                p.name.clone()
            } else {
                String::new()
            };

            crate::protocol::PlayerSnapshot {
                id: p.id,
                name,
                troops: p.troops,
                max_troops: p.max_troops,
                gold: p.gold,
                tile_count: p.tile_count,
                centroid_x: cx,
                centroid_y: cy,
                player_type: p.player_type,
                color: p.color,
                has_spawned: p.has_spawned,
                alive: p.alive,
            }
        }).collect();

        let fleets = self.fleets.iter().map(|f| crate::protocol::FleetSnapshot {
            id: f.id,
            owner_id: f.owner_id,
            troops: f.troops,
            current_tile: f.current_tile,
            path: f.path.clone(),
            path_cursor: f.path_cursor,
            retreating: f.retreating,
        }).collect();

        let attacks = self.attacks.iter().map(|a| crate::protocol::AttackSnapshot {
            id: a.id,
            owner_id: a.owner_id,
            target_owner: a.target_owner,
            troops: a.troops,
            retreating: a.retreating,
        }).collect();

        let mut defense_posts = Vec::new();
        if self.render_defense_dirty {
            for b in &self.buildings {
                if b.kind == crate::game::BuildingKind::DefensePost && !b.under_construction {
                    defense_posts.push(b.tile_idx);
                }
            }
        }
        let defense_dirty = self.render_defense_dirty;
        self.render_defense_dirty = false;

        let spawn_timer_secs = if let crate::game::GamePhase::Spawning { end_tick } = &self.state.phase {
            Some(end_tick.saturating_sub(self.state.tick) as f32 * (self.state.config.tick_rate_ms / 1000.0))
        } else {
            None
        };

        crate::protocol::SimSnapshot {
            tick: self.state.tick,
            phase: self.state.phase.clone(),
            spawn_timer_secs,
            players,
            dirty_tiles,
            fleets,
            attacks,
            winner: self.state.winner,
            total_land_tiles: self.state.total_land_tiles,
            defense_posts,
            defense_dirty,
            debug_mem_info: format!("Engine [Attacks: {}, Fleets: {}, Buildings: {}, Events: {}, Players: {}, DirtyTiles: {}]", self.attacks.len(), self.fleets.len(), self.buildings.len(), self.state.events.len(), self.state.players.len(), self.state.map.dirty_tiles.capacity()),
        }
    }
}
