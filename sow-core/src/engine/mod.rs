use crate::building::{Building, BuildingAggregate, BuildingGrid, DefenseGrid};
use crate::diplomacy::{
    AllianceProposal, ALLIANCE_REQUEST_COOLDOWN_TICKS, ALLIANCE_REQUEST_TTL_TICKS,
    BETRAYAL_COOLDOWN_TICKS,
};
use crate::execution::AttackExecution;
use crate::game::GameState;
use crate::pathfinding::WaterPathfinderScratch;
use crate::player::PlayerId;
use crate::warp_fleet::WarpFleet;
use crate::water_components::WaterComponents;
use serde::{Deserialize, Serialize};

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
    pub alliance_request_cooldown_until: std::collections::HashMap<(PlayerId, PlayerId), u32>,
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
        self
            .alliance_request_cooldown_until
            .get(&(proposer, target))
            .is_none_or(|until| *until <= tick)
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

    pub fn eliminate_player(
        &mut self,
        victim_id: u16,
        conqueror_id: u16,
        ex: u32,
        ey: u32,
        by_nuke: bool,
    ) {
        let mut base_reward = 0.0;
        let mut is_alive = false;
        if let Some(target_player) = self.state.player(victim_id) {
            is_alive = target_player.alive;
            base_reward = match target_player.player_type {
                crate::player::PlayerType::Bot => 500.0,
                crate::player::PlayerType::Nation => 1250.0,
                crate::player::PlayerType::Human => 2500.0,
            };
        }

        if !is_alive {
            return;
        }

        let survived_ticks = self.state.tick;
        let bonus_percent = survived_ticks as f64 * 0.0001; // 0.01% per tick
        let total_reward = base_reward * (1.0 + bonus_percent);

        // Gather tile conquest contributions (deterministic by player id)
        let mut contributors: Vec<(u16, u32)> = self
            .state
            .players
            .iter()
            .filter_map(|p| {
                p.tile_conquests
                    .get(&victim_id)
                    .copied()
                    .filter(|&c| c > 0)
                    .map(|c| (p.id, c))
            })
            .collect();
        contributors.sort_by_key(|(id, _)| *id);

        let others: Vec<(u16, u32)> = contributors
            .iter()
            .filter(|(id, _)| *id != conqueror_id)
            .copied()
            .collect();
        let assist_tiles: u32 = others.iter().map(|(_, c)| c).sum();

        let (killer_gold, mut assist_rewards) = if assist_tiles == 0 {
            (total_reward, Vec::new())
        } else {
            let killer_share = total_reward * 0.5;
            let assist_pool = total_reward * 0.5;
            let mut rewards = Vec::new();
            for (id, count) in &others {
                let share = assist_pool * (*count as f64 / assist_tiles as f64);
                rewards.push((*id, share));
            }
            (killer_share, rewards)
        };

        // 1. Zero out defeated player and award death
        if let Some(target_player) = self.state.player_mut(victim_id) {
            target_player.gold = 0.0;
            target_player.alive = false;
            target_player.deaths += 1;
        }

        // 2. Transfer gold and award kill/assists
        let mut killer_final_gold = killer_gold;
        if let Some(attacker) = self.state.player_mut(conqueror_id) {
            let mut bounty_mult = 1.0;
            if attacker.leader == crate::player::Leader::GenghisKhan {
                bounty_mult = 1.5;
            }
            killer_final_gold *= bounty_mult;
            attacker.gold += killer_final_gold;
            attacker.kills += 1;
        }

        let mut assist_event_rewards = Vec::new();
        for (assist_id, share) in &mut assist_rewards {
            if let Some(p) = self.state.player_mut(*assist_id) {
                p.gold += *share;
                p.assists += 1;
                assist_event_rewards.push((*assist_id, *share as u32));
            }
        }

        // Clear conquest tallies for this victim
        for p in &mut self.state.players {
            p.tile_conquests.remove(&victim_id);
        }

        // 3. Emit elimination event
        self.state
            .events
            .push(crate::game::GameEvent::PlayerEliminated {
                player_id: victim_id,
                conqueror_id,
                gold_bounty: killer_final_gold as u32,
                elimination_x: ex,
                elimination_y: ey,
                assists: assist_event_rewards,
                by_nuke,
            });
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
}

mod snapshot;
mod spawn;
mod tick;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameState;
    use crate::game_config::GameConfig;
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
            engine.state.players.iter().all(|p| p.name == "Testland"),
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

    /// All-land 1000x800 state with the (approx) europe bbox stamped.
    fn geo_test_state(seed: u64) -> GameState {
        let config = GameConfig {
            map_name: "europe_test".to_string(),
            map_width: 1000,
            map_height: 800,
            ..Default::default()
        };
        let mut state = GameState::new(seed, 1000, 800, config);
        for t in &mut state.map.terrain {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }
        state.geo_bounds = Some(crate::map_file::GeoBounds::from_degrees(
            -25.47, 29.00, 47.75, 72.56,
        ));
        state
    }

    fn owned_tile_near(state: &GameState, pid: u16, x: u32, y: u32, radius: i32) -> bool {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if state.map.is_valid_coord(nx, ny)
                    && state.map.owner_id(nx as u32, ny as u32) == pid
                {
                    return true;
                }
            }
        }
        false
    }

    #[test]
    fn test_spawn_ai_geo_names_and_positions() {
        let state = geo_test_state(7);
        let bounds = state.geo_bounds.unwrap();
        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.spawn_ai(20, 30);
        assert_eq!(engine.state.players.len(), 50);

        for player in &engine.state.players {
            let entity = crate::geo_entities::all()
                .find(|e| e.name == player.name)
                .unwrap_or_else(|| {
                    panic!(
                        "'{}' not in geo database (geo pools should cover 20+30 on europe bounds)",
                        player.name
                    )
                });
            let is_tribe_kind = entity.kind == crate::geo_entities::EntityKind::Tribe;
            assert_eq!(
                player.player_type == crate::player::PlayerType::Bot,
                is_tribe_kind,
                "kind mismatch for {}",
                player.name
            );
            let (x, y) = bounds
                .project(entity.lat as f64, entity.lon as f64, 1000, 800)
                .expect("spawned geo entity must project inside bounds");
            // place_spawn paints a radius-5 disc; allow drift from collisions.
            assert!(
                owned_tile_near(&engine.state, player.id, x, y, 12),
                "{} spawned far from its homeland tile ({x}, {y})",
                player.name
            );
        }
    }

    #[test]
    fn test_spawn_ai_geo_deterministic() {
        let names = |seed: u64| -> Vec<String> {
            let mut engine = SowEngine::new(geo_test_state(seed), WaterComponents::default());
            engine.spawn_ai(15, 25);
            engine
                .state
                .players
                .iter()
                .map(|p| p.name.clone())
                .collect()
        };
        assert_eq!(names(1234), names(1234), "same seed must give same spawns");
        assert_ne!(names(1234), names(5678), "different seed should differ");
    }

    #[test]
    fn test_spawn_ai_geo_overflow_to_fallback() {
        // Europe bounds hold well under 500 geo tribes; the rest must come
        // from the fallback pools without panicking or duplicating names.
        let mut engine = SowEngine::new(geo_test_state(9), WaterComponents::default());
        engine.spawn_ai(0, 500);
        assert_eq!(engine.state.players.len(), 500);
        let mut seen = std::collections::HashSet::new();
        for p in &engine.state.players {
            assert!(seen.insert(p.name.clone()), "duplicate name {}", p.name);
        }
    }

    #[test]
    fn test_team_map_control_winner() {
        use crate::protocol::Team;

        let config = GameConfig {
            game_mode: "Teams".to_string(),
            map_control_win_percentage: 0.50,
            ..Default::default()
        };
        let mut state = GameState::new(42, 10, 10, config);
        for t in &mut state.map.terrain {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }
        state.total_land_tiles = 100;

        let mut red =
            crate::player::Player::new_human(1, "Red".into(), [1.0, 0.2, 0.2], &state.config);
        red.team = Some(Team::Red);
        red.tile_count = 55;
        red.alive = true;

        let mut blue =
            crate::player::Player::new_human(2, "Blue".into(), [0.2, 0.5, 1.0], &state.config);
        blue.team = Some(Team::Blue);
        blue.tile_count = 10;
        blue.alive = true;

        state.register_player(red);
        state.register_player(blue);

        let mut engine = SowEngine::new(state, WaterComponents::default());
        engine.check_team_winner(50);

        assert_eq!(engine.state.winning_team, Some(Team::Red));
        assert_eq!(engine.state.winner, Some(1));
    }
}
