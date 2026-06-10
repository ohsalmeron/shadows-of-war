use crate::building::{Building, BuildingAggregate, BuildingGrid, DefenseGrid};
use crate::diplomacy::{
    AllianceProposal, ALLIANCE_REQUEST_COOLDOWN_TICKS, ALLIANCE_REQUEST_TTL_TICKS,
    BETRAYAL_COOLDOWN_TICKS,
};
use crate::execution::AttackExecution;
use crate::game::GameState;
use crate::player::PlayerId;
use crate::pathfinding::WaterPathfinderScratch;
use crate::warp_fleet::WarpFleet;
use crate::water_components::WaterComponents;
use serde::{Serialize, Deserialize};


#[derive(Clone)]
pub struct PlacementScratch {
    pub visited_stamp: [u32; 1024],
    pub stamp: u32,
    pub queue: Vec<u32>,
    pub border_scratch: Vec<(u32, u32)>,
}

impl Default for PlacementScratch {
    fn default() -> Self {
        Self {
            visited_stamp: [0; 1024],
            stamp: 0,
            queue: Vec::new(),
            border_scratch: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResourceRequestProposed {
    pub proposer: crate::player::PlayerId,
    pub target: crate::player::PlayerId,
    pub gold: f64,
    pub troops: f64,
}

#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct SowEngine {
    pub state: GameState,
    pub attacks: Vec<AttackExecution>,
    pub fleets: Vec<WarpFleet>,
    pub buildings: Vec<Building>,
    pub water: WaterComponents,
    pub path_scratch: WaterPathfinderScratch,
    pub flow_field_cache: crate::pathfinding::FlowFieldCache,
    pub placement_scratch: PlacementScratch,
    pub defense_grid: DefenseGrid,
    pub defense_grid_dirty: bool,
    pub render_defense_dirty: bool,
    pub building_grid: BuildingGrid,
    pub building_aggregates: Vec<BuildingAggregate>,
    pub building_aggregates_dirty: bool,
    pub sea_lanes_dirty: bool,
    pub sea_lane_calc: Option<(usize, Vec<crate::sea_lane::SeaLane>, Vec<(u64, u32, u32)>)>,

    pub alliances_proposed: Vec<AllianceProposal>,
    /// `(proposer, target)` → tick when another outgoing request is allowed.
    pub alliance_request_cooldown_until:
        std::collections::HashMap<(PlayerId, PlayerId), u32>,
    /// Bot id → tick when another betrayal is allowed.
    pub alliance_betray_cooldown_until: std::collections::HashMap<PlayerId, u32>,
    pub resource_requests_proposed: Vec<ResourceRequestProposed>,
    pub port_queues:
        std::collections::HashMap<u64, std::collections::VecDeque<crate::game::ShipProduction>>,
    pub projectiles: Vec<crate::game::Projectile>,
    pub silo_cooldowns: std::collections::HashMap<u64, u32>,
    pub mirv_launches: std::collections::HashMap<u16, u32>,
    pub recent_nuke_targets: Vec<(u16, u32, u64)>,
    pub mirv_cooldown_targets: std::collections::HashMap<u16, u64>,
}

impl SowEngine {
    pub fn new(mut state: GameState, water: WaterComponents) -> Self {
        state.map.compute_shorelines();
        let w = state.map.width;
        let h = state.map.height;
        let area = (w * h) as usize;

        let mut path_scratch = WaterPathfinderScratch::default();
        if w > 0 && h > 0 {
            path_scratch.astar.ensure_capacity(&state.map);
            path_scratch.bfs_visited.resize(area, 0);
        }

        let mut placement_scratch = PlacementScratch::default();
        placement_scratch.queue.reserve(1024);
        placement_scratch.border_scratch.reserve(1024);

        Self {
            state,
            attacks: Vec::with_capacity(1024),
            fleets: Vec::with_capacity(256),
            buildings: Vec::with_capacity(4096),
            water,
            path_scratch,
            flow_field_cache: crate::pathfinding::FlowFieldCache::default(),
            placement_scratch,
            defense_grid: DefenseGrid::default(),
            defense_grid_dirty: true,
            render_defense_dirty: true,
            building_grid: BuildingGrid::default(),
            building_aggregates: Vec::with_capacity(256),
            building_aggregates_dirty: true,
            sea_lanes_dirty: true,
            sea_lane_calc: None,

            alliances_proposed: Vec::new(),
            alliance_request_cooldown_until: std::collections::HashMap::new(),
            alliance_betray_cooldown_until: std::collections::HashMap::new(),
            resource_requests_proposed: Vec::new(),
            port_queues: std::collections::HashMap::new(),
            projectiles: Vec::new(),
            silo_cooldowns: std::collections::HashMap::new(),
            mirv_launches: std::collections::HashMap::new(),
            recent_nuke_targets: Vec::new(),
            mirv_cooldown_targets: std::collections::HashMap::new(),
        }
    }

    #[inline]
    pub fn current_tick_u32(&self) -> u32 {
        self.state.tick as u32
    }

    /// Expire stale proposals, cooldown entries, traitor flags, and emoji timers run elsewhere.
    pub fn prune_alliance_diplomacy(&mut self) {
        let tick = self.current_tick_u32();
        let mut expired = Vec::new();
        self.alliances_proposed.retain(|p| {
            let alive = tick.saturating_sub(p.created_tick) <= ALLIANCE_REQUEST_TTL_TICKS;
            if !alive {
                expired.push(*p);
            }
            alive
        });
        for p in expired {
            self.mark_alliance_request_cooldown(p.proposer, p.target);
        }
        self.alliance_request_cooldown_until
            .retain(|_, until| *until > tick);
        self.alliance_betray_cooldown_until
            .retain(|_, until| *until > tick);
        for player in &mut self.state.players {
            if player.traitor && player.traitor_tick > 0 && tick >= player.traitor_tick {
                player.traitor = false;
                player.traitor_tick = 0;
            }
        }
    }

    #[inline]
    pub fn has_alliance_proposal(&self, proposer: PlayerId, target: PlayerId) -> bool {
        self.alliances_proposed
            .iter()
            .any(|p| p.proposer == proposer && p.target == target)
    }

    #[inline]
    pub fn can_send_alliance_request(&self, proposer: PlayerId, target: PlayerId) -> bool {
        if self.has_alliance_proposal(proposer, target) {
            return false;
        }
        let tick = self.current_tick_u32();
        !self
            .alliance_request_cooldown_until
            .get(&(proposer, target))
            .is_some_and(|until| *until > tick)
    }

    pub fn mark_alliance_request_cooldown(&mut self, proposer: PlayerId, target: PlayerId) {
        let until = self
            .current_tick_u32()
            .saturating_add(ALLIANCE_REQUEST_COOLDOWN_TICKS);
        self.alliance_request_cooldown_until
            .insert((proposer, target), until);
    }

    pub fn mark_betrayal_cooldown(&mut self, bot_id: PlayerId) {
        let until = self
            .current_tick_u32()
            .saturating_add(BETRAYAL_COOLDOWN_TICKS);
        self.alliance_betray_cooldown_until.insert(bot_id, until);
    }

    pub fn push_alliance_proposal(&mut self, proposer: PlayerId, target: PlayerId) {
        if self.has_alliance_proposal(proposer, target) {
            return;
        }
        self.alliances_proposed.push(AllianceProposal {
            proposer,
            target,
            created_tick: self.current_tick_u32(),
        });
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
        let is_ready_defense = b.kind == crate::game::BuildingKind::Bunker && !b.under_construction;
        let pos = self.buildings.partition_point(|x| x.id < b.id);
        self.buildings.insert(pos, b);
        self.building_grid.mark_dirty();
        self.building_aggregates_dirty = true;
        if !b.under_construction && b.kind == crate::game::BuildingKind::City {
            self.sea_lanes_dirty = true;
        }
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
                let unspawned: Vec<u16> = self
                    .state
                    .players
                    .iter()
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

        if self.sea_lane_calc.is_some() {
            crate::sea_lane::update_sea_lanes(self);
        }

        self.execute_income();
        self.prune_alliance_diplomacy();
        self.execute_ai_think();
        self.execute_construction();
        self.execute_ship_production();
        self.execute_projectiles();
        self.execute_sam();
        self.execute_combat();

        // Sync building ownership with tile ownership
        for b in &mut self.buildings {
            let col = b.tile_idx % self.state.map.width;
            let row = b.tile_idx / self.state.map.width;
            let tile_owner = self.state.map.owner_id(col, row);

            // Only transfer if the tile has a new valid owner
            if tile_owner != 0 && tile_owner != b.owner_id {
                let old_owner = b.owner_id;
                let new_owner = tile_owner;
                let kind = b.kind;

                // Transfer ownership
                b.owner_id = new_owner;

                // Update player counts if necessary
                if kind == crate::game::BuildingKind::City {
                    if old_owner != 0 {
                        if let Some(p) = self.state.player_mut(old_owner) {
                            p.cities = p.cities.saturating_sub(1);
                        }
                    }
                    if new_owner != 0 {
                        if let Some(p) = self.state.player_mut(new_owner) {
                            p.cities += 1;
                        }
                    }
                }
            }
        }

        self.execute_fleets();
        self.check_winner();

        let mut expired_alliances = Vec::new();
        for player in &mut self.state.players {
            let pid = player.id;
            if player.emoji_timer > 0 && !player.emoji_pinned {
                player.emoji_timer -= 1;
                if player.emoji_timer == 0 {
                    player.active_emoji = None;
                }
            }

            // Decay alliance timers
            let mut expired_for_player = Vec::new();
            for (&ally_id, timer) in &mut player.alliance_timers {
                if *timer > 0 {
                    *timer -= 1;
                    if *timer == 0 {
                        expired_for_player.push(ally_id);
                    }
                }
            }
            for ally_id in expired_for_player {
                player.alliances.retain(|&id| id != ally_id);
                player.alliance_timers.remove(&ally_id);
                expired_alliances.push((pid, ally_id));
            }
        }

        // Mutual expiration enforcement
        for (a, b) in expired_alliances {
            if let Some(p_b) = self.state.player_mut(b) {
                p_b.alliances.retain(|&id| id != a);
                p_b.alliance_timers.remove(&a);
            }
        }
    }

    fn check_winner(&mut self) {
        if self.state.winner.is_some() {
            return;
        }

        if self.state.total_land_tiles == 0 {
            self.state.total_land_tiles = self
                .state
                .map
                .terrain
                .iter()
                .filter(|t| t.is_land())
                .count() as u32;
            if self.state.total_land_tiles == 0 {
                self.state.total_land_tiles = 1; // Prevent division by zero
            }
        }

        let win_threshold = (self.state.total_land_tiles as f32
            * self.state.config.map_control_win_percentage) as u32;

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
            self.state
                .events
                .push(crate::game::GameEvent::GameOver { winner_id: wid });
        } else if alive_players == 1 {
            self.state.winner = last_alive_id;
            self.state.phase = crate::game::GamePhase::GameOver;
            if let Some(wid) = last_alive_id {
                self.state
                    .events
                    .push(crate::game::GameEvent::GameOver { winner_id: wid });
            }
        } else if alive_players == 0 && !self.state.players.is_empty() {
            // Everyone died? Rare but possible.
            self.state.phase = crate::game::GamePhase::GameOver;
        }
    }
    pub fn spawn_ai(&mut self, city_state_count: u32, tribe_count: u32) {
        let mut spawned_city_states = 0;
        let mut spawned_tribes = 0;
        use crate::player::Player;
        use wyrand::WyRand;

        let mut rng = WyRand::new(self.state.seed);
        let config = self.state.config.clone();

        let anchor_count = self.state.map_spawns.len();
        if anchor_count > 0 || city_state_count > 0 || tribe_count > 0 {
            log::info!(
                "spawn_ai: map='{}' city_state_anchors={} city_state_count={} tribe_count={}",
                self.state.config.map_name,
                anchor_count,
                city_state_count,
                tribe_count
            );
        }

        let total_city_states_to_spawn = city_state_count;

        // Keep track of names already used to prevent duplicates
        let mut used_names = std::collections::HashSet::new();

        let map_spawns_snapshot: Vec<crate::map_file::MapSpawn> =
            self.state.map_spawns.clone();

        // Prepare the fallback historical civilizations pool for extra city-states
        let extra_nations_pool = crate::tribes::HISTORICAL_CIVILIZATIONS;
        let mut extra_nations_indices: Vec<usize> = (0..extra_nations_pool.len()).collect();

        // Prepare fallback tribes in case extra nations run out
        let fallback_nations_pool = crate::tribes::FALLBACK_TRIBES;
        let mut fallback_nations_indices: Vec<usize> = (0..fallback_nations_pool.len()).collect();

        for i in 0..total_city_states_to_spawn {
            let bot_id = 104 + i as u16;

            let anchored = map_spawns_snapshot.get(i as usize);
            let mut spawn_point = None;
            let mut name = String::new();

            if let Some(spawn) = anchored {
                let nx = spawn.x;
                let ny = spawn.y;
                if self.state.map.is_valid_coord(nx as i32, ny as i32)
                    && self.state.map.owner_id(nx, ny) == 0
                    && self.state.map.terrain[self.state.map.ref_id(nx, ny)].is_land()
                {
                    spawn_point = Some((nx, ny));
                }
                name = spawn.name.clone();
                used_names.insert(name.clone());
            } else {
                // We need extra nations! Grab from HISTORICAL_CIVILIZATIONS and ensure no duplicate of any used name
                let mut found_name = false;
                let mut attempts = 0;
                while !found_name && attempts < 100 && !extra_nations_indices.is_empty() {
                    let idx = (rng.rand() as usize) % extra_nations_indices.len();
                    let pool_idx = extra_nations_indices[idx];
                    let potential_name = extra_nations_pool[pool_idx].to_string();
                    if !used_names.contains(&potential_name) {
                        name = potential_name;
                        used_names.insert(name.clone());
                        extra_nations_indices.swap_remove(idx);
                        found_name = true;
                    } else {
                        // Remove from indices since it's already used
                        extra_nations_indices.swap_remove(idx);
                    }
                    attempts += 1;
                }

                if !found_name {
                    let mut found_fallback = false;
                    let mut attempts_fallback = 0;
                    while !found_fallback && attempts_fallback < 100 && !fallback_nations_indices.is_empty() {
                        let idx = (rng.rand() as usize) % fallback_nations_indices.len();
                        let pool_idx = fallback_nations_indices[idx];
                        let raw_tribe_name = fallback_nations_pool[pool_idx];
                        
                        let name_style = (rng.rand() as usize) % 9;
                        let formatted_name = match name_style {
                            0 => format!("{} Empire", raw_tribe_name),
                            1 => format!("Kingdom of {}", raw_tribe_name),
                            2 => format!("{} Dynasty", raw_tribe_name),
                            3 => format!("Republic of {}", raw_tribe_name),
                            4 => format!("{} Confederacy", raw_tribe_name),
                            5 => format!("{} Sultanate", raw_tribe_name),
                            6 => format!("Principality of {}", raw_tribe_name),
                            7 => format!("Grand Duchy of {}", raw_tribe_name),
                            _ => format!("{} Alliance", raw_tribe_name),
                        };
                        
                        if !used_names.contains(&formatted_name) && !used_names.contains(&raw_tribe_name.to_string()) {
                            name = formatted_name;
                            used_names.insert(name.clone());
                            used_names.insert(raw_tribe_name.to_string());
                            fallback_nations_indices.swap_remove(idx);
                            found_fallback = true;
                        } else {
                            fallback_nations_indices.swap_remove(idx);
                        }
                        attempts_fallback += 1;
                    }

                    if !found_fallback {
                        name = format!("Empire {}", bot_id);
                    }
                }
            }

            if i < 5 {
                log::info!(
                    "spawn_ai city_state[{}]: name='{}' anchored={}",
                    i,
                    name,
                    anchored.is_some()
                );
            }

            if spawn_point.is_none() {
                spawn_point = self.find_valid_spawn(&mut rng);
            }

            if let Some((sx, sy)) = spawn_point {
                let (team, color) = if config.game_mode == "Teams" {
                    if i % 2 == 0 {
                        (Some(crate::protocol::Team::Red), [1.0, 0.2, 0.2])
                    } else {
                        (Some(crate::protocol::Team::Blue), [0.2, 0.5, 1.0])
                    }
                } else {
                    (None, crate::player::human_shader_territory_rgb(bot_id))
                };

                let mut player = Player::new_nation(bot_id, name, color, &config);
                player.team = team;
                self.state.spawn_player(player, sx, sy);
                spawned_city_states += 1;
            }
        }

        // Spawn tribes (IDs above city-states)
        let tribe_start_id = 104 + total_city_states_to_spawn as u16;
        let fallback_pool = crate::tribes::FALLBACK_TRIBES;
        let mut fallback_indices: Vec<usize> = (0..fallback_pool.len()).collect();

        for i in 0..tribe_count {
            let bot_id = tribe_start_id + i as u16;

            let mut name = String::new();
            let mut found_name = false;
            let mut attempts = 0;

            while !found_name && attempts < 100 {
                if fallback_indices.is_empty() {
                    fallback_indices = (0..fallback_pool.len()).collect();
                }
                let idx = (rng.rand() as usize) % fallback_indices.len();
                let pool_idx = fallback_indices[idx];
                let potential_name = fallback_pool[pool_idx].to_string();
                if !used_names.contains(&potential_name) {
                    name = potential_name;
                    used_names.insert(name.clone());
                    fallback_indices.swap_remove(idx);
                    found_name = true;
                } else {
                    fallback_indices.swap_remove(idx);
                }
                attempts += 1;
            }

            if !found_name {
                name = format!("Tribe {}", bot_id);
            }

            if let Some((sx, sy)) = self.find_valid_spawn(&mut rng) {
                let color = crate::player::bot_territory_color(self.state.seed, bot_id);
                let player = Player::new_bot(bot_id, name, color, &config);
                self.state.spawn_player(player, sx, sy);
                spawned_tribes += 1;
            }
        }

        if total_city_states_to_spawn > 0 || tribe_count > 0 {
            log::info!(
                "Spawned {} city-states and {} tribes successfully.",
                spawned_city_states,
                spawned_tribes
            );
        }
    }

    pub fn spawn_human(
        &mut self,
        player_id: u16,
        name: String,
        color: [f32; 3],
        team: Option<crate::protocol::Team>,
        civilization: crate::player::Civilization,
        leader: crate::player::Leader,
    ) {
        use crate::player::Player;
        use wyrand::WyRand;

        // Use a different seed offset for human to avoid clashing exactly with bots
        let mut rng = WyRand::new(self.state.seed.wrapping_add(player_id as u64));
        let config = self.state.config.clone();

        if !config.random_spawn {
            let mut player = Player::new_human(player_id, name, color, &config);
            player.team = team;
            player.civilization = civilization;
            player.leader = leader;
            self.state.register_player(player);
            return;
        }

        if let Some((sx, sy)) = self.find_valid_spawn(&mut rng) {
            let mut player = Player::new_human(player_id, name, color, &config);
            player.team = team;
            player.civilization = civilization;
            player.leader = leader;
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
                    if self.state.map.is_valid_coord(nx, ny)
                        && self.state.map.owner_id(nx as u32, ny as u32) != 0
                    {
                        valid = false;
                        break;
                    }
                }
                if !valid {
                    break;
                }
            }
            if valid {
                return Some((sx, sy));
            }
            tries += 1;
        }
        None
    }

    /// Build a lightweight snapshot of the current state for the render thread.
    /// Drains `map.dirty_tiles` so each tile is reported exactly once.
    pub fn build_snapshot(&mut self) -> crate::protocol::SimSnapshot {
        for p in &mut self.state.players {
            if p.alive && p.tile_count > 0 {
                // Desynchronize calculation using player ID as a phase offset
                let should_recalculate = self.state.tick < 3 || (p.id as usize + self.state.tick as usize) % 15 == 0;
                if should_recalculate {
                    p.calculate_nameplate(&self.state.map);
                } else {
                    let cx = (p.sum_x / p.tile_count as u64) as f32;
                    let cy = (p.sum_y / p.tile_count as u64) as f32;
                    p.nameplate_x = cx + p.nameplate_offset_x;
                    p.nameplate_y = cy + p.nameplate_offset_y;
                }
            }
        }

        let dirty_tiles: Vec<crate::protocol::DirtyTile> = self
            .state
            .map
            .dirty_tiles
            .drain(..)
            .map(|i| crate::protocol::DirtyTile {
                index: i as u32,
                new_owner: self.state.map.state[i],
                upgrade_level: self.state.map.tile_upgrades[i],
            })
            .collect();

        let proposed = &self.alliances_proposed;
        let proposed_resources = &self.resource_requests_proposed;
        let players = self
            .state
            .players
            .iter()
            .map(|p| {
                let (cx, cy) = if p.tile_count > 0 {
                    (
                        (p.sum_x / p.tile_count as u64) as f32,
                        (p.sum_y / p.tile_count as u64) as f32,
                    )
                } else {
                    (0.0, 0.0)
                };

                let name = p.name.clone();

                let alliance_requests = proposed
                    .iter()
                    .filter(|prop| prop.target == p.id)
                    .map(|prop| prop.proposer)
                    .collect();

                let resource_requests = proposed_resources
                    .iter()
                    .filter(|r| r.target == p.id)
                    .map(|r| crate::protocol::ResourceRequest {
                        requester: r.proposer,
                        gold: r.gold,
                        troops: r.troops,
                    })
                    .collect();

                crate::protocol::PlayerSnapshot {
                    id: p.id,
                    name,
                    troops: p.troops,
                    max_troops: p.max_troops,
                    gold: p.gold,
                    tile_count: p.tile_count,
                    centroid_x: cx,
                    centroid_y: cy,
                    nameplate_x: p.nameplate_x,
                    nameplate_y: p.nameplate_y,
                    nameplate_size: p.nameplate_size,
                    player_type: p.player_type,
                    color: p.color,
                    team: p.team,
                    has_spawned: p.has_spawned,
                    alive: p.alive,
                    iq: p.iq,
                    alliances: p.alliances.clone(),
                    alliance_timers: p.alliance_timers.clone(),
                    alliance_requests,
                    resource_requests,
                    disconnected: p.disconnected,
                    active_emoji: p.active_emoji.clone(),
                    civilization: p.civilization,
                    leader: p.leader,
                }
            })
            .collect();

        let fleets = self
            .fleets
            .iter()
            .map(|f| crate::protocol::FleetSnapshot {
                id: f.id,
                owner_id: f.owner_id,
                unit_type: f.unit_type,
                troops: f.troops,
                current_tile: f.current_tile,
                path: f.path.clone(),
                path_cursor: f.path_cursor,
                retreating: f.retreating,
            })
            .collect();

        let attacks = self
            .attacks
            .iter()
            .map(|a| {
                let (fcx, fcy) = if a.target_owner != 0 {
                    a.frontier_centroid()
                } else {
                    (0.0, 0.0)
                };
                crate::protocol::AttackSnapshot {
                    id: a.id,
                    owner_id: a.owner_id,
                    target_owner: a.target_owner,
                    troops: a.troops,
                    retreating: a.retreating,
                    front_cx: fcx,
                    front_cy: fcy,
                }
            })
            .collect();

        let mut defense_posts = Vec::new();
        if self.render_defense_dirty {
            for b in &self.buildings {
                if b.kind == crate::game::BuildingKind::Bunker && !b.under_construction {
                    defense_posts.push(b.tile_idx);
                }
            }
        }
        let defense_dirty = self.render_defense_dirty;
        self.render_defense_dirty = false;

        let spawn_timer_secs =
            if let crate::game::GamePhase::Spawning { end_tick } = &self.state.phase {
                Some(
                    end_tick.saturating_sub(self.state.tick) as f32
                        * (self.state.config.tick_rate_ms / 1000.0),
                )
            } else {
                None
            };

        let buildings: Vec<crate::protocol::BuildingSnapshot> = self
            .buildings
            .iter()
            .map(|b| crate::protocol::BuildingSnapshot {
                id: b.id,
                tile_idx: b.tile_idx,
                owner_id: b.owner_id,
                kind: b.kind,
                level: b.level,
                under_construction: b.under_construction,
                ticks_until_complete: b.ticks_until_complete,
                modules: b.modules,
            })
            .collect();

        crate::protocol::SimSnapshot {
            tick: self.state.tick,
            phase: self.state.phase.clone(),
            spawn_timer_secs,
            players,
            dirty_tiles,
            fleets,
            attacks,
            buildings,
            projectiles: self
                .projectiles
                .iter()
                .filter(|p| p.active)
                .map(|p| crate::protocol::ProjectileSnapshot {
                    id: p.id,
                    kind: p.kind,
                    owner_id: p.owner_id,
                    src_tile: p.src_tile,
                    dst_tile: p.dst_tile,
                    path: p.path.clone(),
                    path_cursor: p.path_cursor,
                    steps_per_tick: p.steps_per_tick,
                })
                .collect(),
            nuke_alerts: self
                .state
                .events
                .iter()
                .filter_map(|e| {
                    if let crate::game::GameEvent::NukeDetonated {
                        tile_x,
                        tile_y,
                        owner_id,
                        inner_radius: _,
                        outer_radius: _,
                    } = e
                    {
                        let kind = crate::game::NukeKind::AtomBomb;
                        Some(crate::protocol::NukeAlert {
                            owner_id: *owner_id,
                            kind,
                            tile_x: *tile_x,
                            tile_y: *tile_y,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            resource_transfers: self
                .state
                .events
                .iter()
                .filter_map(|e| {
                    if let crate::game::GameEvent::ResourceTransferred {
                        sender_id,
                        receiver_id,
                        gold,
                        troops,
                    } = e
                    {
                        Some(crate::protocol::ResourceTransfer {
                            sender_id: *sender_id,
                            receiver_id: *receiver_id,
                            gold: *gold,
                            troops: *troops,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            resource_rejections: self
                .state
                .events
                .iter()
                .filter_map(|e| {
                    if let crate::game::GameEvent::ResourceRequestRejected {
                        rejector_id,
                        requester_id,
                    } = e
                    {
                        Some(crate::protocol::ResourceRejection {
                            rejector_id: *rejector_id,
                            requester_id: *requester_id,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            winner: self.state.winner,
            total_land_tiles: self.state.total_land_tiles,
            defense_posts,
            defense_dirty,
            sea_lanes: self.state.sea_lanes.clone(),
            debug_mem_info: if cfg!(feature = "mem_profiler") {
                format!(
                    "Engine [Attacks: {}/{} | Fleets: {}/{} | Buildings: {}/{} | Events: {}/{} | Players: {}/{} | DirtyTilesCap: {}] Pathfinder [AStarHeapCap: {} | AStarCameCap: {} | BFSQueueCap: {} | BFSVisitedCap: {}] Placement [VisitedCap: {} | QueueCap: {} | BorderCap: {}]",
                    self.attacks.len(), self.attacks.capacity(),
                    self.fleets.len(), self.fleets.capacity(),
                    self.buildings.len(), self.buildings.capacity(),
                    self.state.events.len(), self.state.events.capacity(),
                    self.state.players.len(), self.state.players.capacity(),
                    self.state.map.dirty_tiles.capacity(),
                    self.path_scratch.astar.heap.capacity(),
                    self.path_scratch.astar.came_from.capacity(),
                    self.path_scratch.bfs_queue.capacity(),
                    self.path_scratch.bfs_visited.capacity(),
                    self.placement_scratch.visited_stamp.len(),
                    self.placement_scratch.queue.capacity(),
                    self.placement_scratch.border_scratch.capacity(),
                )
            } else {
                String::new()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_config::GameConfig;
    use crate::game::GameState;
    use crate::water_components::WaterComponents;
    #[test]
    fn test_spawn_ai_city_states() {
        let config = GameConfig {
            map_name: crate::maps::DEFAULT_MAP_KEY.to_string(),
            map_width: 1000,
            map_height: 800,
            ..Default::default()
        };
        let mut state = GameState::new(42, 1000, 800, config.clone());
        for t in &mut state.map.terrain {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }
        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.spawn_ai(0, 0);
        assert_eq!(engine.state.players.len(), 0);

        let mut state = GameState::new(42, 1000, 800, config.clone());
        for t in &mut state.map.terrain {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }
        state.map_spawns = vec![
            crate::map_file::MapSpawn {
                name: "Testland".to_string(),
                flag: "xx".to_string(),
                x: 10,
                y: 10,
            },
            crate::map_file::MapSpawn {
                name: "Testland".to_string(),
                flag: "xx".to_string(),
                x: 20,
                y: 20,
            },
        ];

        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.spawn_ai(2, 0);
        assert_eq!(engine.state.players.len(), 2);
        assert!(
            engine
                .state
                .players
                .iter()
                .all(|p| p.name == "Testland"),
            "anchored spawns use map.bin spawn names"
        );

        let mut state = GameState::new(42, 1000, 800, config);
        for t in &mut state.map.terrain {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }
        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.spawn_ai(3, 0);
        assert_eq!(engine.state.players.len(), 3);
    }
}
