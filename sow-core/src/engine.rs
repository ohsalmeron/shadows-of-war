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
    /// When true, [`DefenseGrid::rebuild`] must run before combat queries (new/changed defense posts).
    pub defense_grid_dirty: bool,
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
            building_grid: BuildingGrid::default(),
            building_aggregates: Vec::new(),
            building_aggregates_dirty: true,
            ai_round_robin: 0,
        }
    }

    /// Rebuilds [`BuildingGrid`] when dirty or first use (`grid_w == 0`).
    pub fn refresh_building_grid(&mut self) {
        if !self.building_grid.dirty && self.building_grid.grid_w > 0 {
            return;
        }
        let w = self.state.map.width;
        let h = self.state.map.height;
        self.building_grid.rebuild(self.buildings.iter(), w, h);
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
        self.state.tick();
        self.execute_income();
        self.tick_bots();
        self.execute_combat();
        // TODO: buildings, fleets, pending turns...
    }
    pub fn spawn_ai(&mut self, nation_count: u32, tribe_count: u32) {
        let mut spawned_nations = 0;
        let mut spawned_tribes = 0;
        use wyrand::WyRand;
        use crate::rng::NextIntExt;
        use crate::player::Player;

        let mut rng = WyRand::new(self.state.seed);
        let config = self.state.config.clone();
        
        // Spawn Nations (IDs 104 to 199)
        for i in 0..nation_count {
            let bot_id = 104 + i as u16;
            let mut tries = 0;
            let (mut sx, mut sy) = (0, 0);

            while tries < 1000 {
                sx = rng.next_int(0, self.state.map.width as i32) as u32;
                sy = rng.next_int(0, self.state.map.height as i32) as u32;
                
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
                if valid { break; }
                tries += 1;
            }

            if tries < 1000 {
                // Nations have a starting advantage? Or just distinct colors for now.
                let player = Player::new_bot(bot_id, format!("Nation {}", i+1), [0.8, 0.8, 0.8], &config);
                self.state.spawn_player(player, sx, sy);
                spawned_nations += 1;
            }
        }

        // Spawn Tribes (IDs 200+)
        for i in 0..tribe_count {
            let bot_id = 200 + i as u16;
            let mut tries = 0;
            let (mut sx, mut sy) = (0, 0);

            while tries < 1000 {
                sx = rng.next_int(0, self.state.map.width as i32) as u32;
                sy = rng.next_int(0, self.state.map.height as i32) as u32;
                
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
                if valid { break; }
                tries += 1;
            }

            if tries < 1000 {
                let player = Player::new_bot(bot_id, format!("Tribe {}", i+1), [0.4, 0.4, 0.4], &config);
                self.state.spawn_player(player, sx, sy);
                spawned_tribes += 1;
            }
        }
        log::info!("Spawned {} nations and {} tribes successfully.", spawned_nations, spawned_tribes);
    }

    pub fn spawn_human(&mut self, player_id: u16, name: String, color: [f32; 3]) {
        use wyrand::WyRand;
        use crate::rng::NextIntExt;
        use crate::player::Player;

        // Use a different seed offset for human to avoid clashing exactly with bots
        let mut rng = WyRand::new(self.state.seed.wrapping_add(player_id as u64));
        let config = self.state.config.clone();
        
        let mut tries = 0;
        let (mut sx, mut sy) = (0, 0);

        while tries < 1000 {
            sx = rng.next_int(0, self.state.map.width as i32) as u32;
            sy = rng.next_int(0, self.state.map.height as i32) as u32;
            
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
            if valid { break; }
            tries += 1;
        }

        if tries < 1000 {
            let player = Player::new_human(player_id, name, color, &config);
            self.state.spawn_player(player, sx, sy);
        } else {
            log::warn!("Failed to spawn Human {} - no room!", player_id);
        }
    }
}
