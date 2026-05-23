use std::collections::HashMap;
use crate::building::BuildingAggregate;

pub const REGION_SIZE: u32 = 32;

#[derive(Default, Clone)]
pub struct RegionData {
    pub dominant_owner: u16,
    pub tile_counts: HashMap<u16, u32>,
    pub aggregate: BuildingAggregate,
}

#[derive(Default, Clone)]
pub struct RegionGrid {
    pub grid_w: u32,
    pub grid_h: u32,
    pub regions: Vec<RegionData>,
}

impl RegionGrid {
    pub fn rebuild(&mut self, map_w: u32, map_h: u32, map_state: &[u16], buildings: &[crate::building::Building]) {
        let cols = (map_w + REGION_SIZE - 1) / REGION_SIZE;
        let rows = (map_h + REGION_SIZE - 1) / REGION_SIZE;
        self.grid_w = cols;
        self.grid_h = rows;
        let total = (cols * rows) as usize;
        self.regions.clear();
        self.regions.resize(total, RegionData::default());

        // Process map tiles to find dominant owner
        for (idx, &owner_id) in map_state.iter().enumerate() {
            if owner_id == 0 { continue; }
            let x = (idx as u32) % map_w;
            let y = (idx as u32) / map_w;
            let cx = x / REGION_SIZE;
            let cy = y / REGION_SIZE;
            let chunk_idx = (cy * cols + cx) as usize;
            
            let count = self.regions[chunk_idx].tile_counts.entry(owner_id).or_insert(0);
            *count += 1;
        }

        for region in &mut self.regions {
            let mut best_owner = 0;
            let mut best_count = 0;
            for (&owner, &count) in &region.tile_counts {
                if count > best_count {
                    best_owner = owner;
                    best_count = count;
                }
            }
            region.dominant_owner = best_owner;
        }

        // Process buildings
        for b in buildings {
            let x = b.tile_idx % map_w;
            let y = b.tile_idx / map_w;
            let cx = x / REGION_SIZE;
            let cy = y / REGION_SIZE;
            let chunk_idx = (cy * cols + cx) as usize;
            
            let agg = &mut self.regions[chunk_idx].aggregate;
            match b.kind {
                crate::game::BuildingKind::City => {
                    agg.count_city += 1;
                    agg.city_levels += b.level as u32;
                }
                crate::game::BuildingKind::Factory => {
                    agg.count_factory += 1;
                    agg.factory_levels += b.level as u32;
                }
                crate::game::BuildingKind::MissileSilo => {
                    agg.count_silo += 1;
                }
                crate::game::BuildingKind::DefensePost => {
                    agg.count_defense += 1;
                }
                crate::game::BuildingKind::SamLauncher => {
                    agg.count_sam += 1;
                }
                crate::game::BuildingKind::Port => {
                    agg.count_port += 1;
                }
                _ => {}
            }
        }
    }
}
