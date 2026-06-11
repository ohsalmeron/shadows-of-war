use crate::building::{
    resolve_structure_spawn_tile, structure_build_cost_gold, structure_kind_enabled, Building,
    CityModules, ModuleKind,
};
use crate::engine::SowEngine;
use crate::game::{BuildingKind, GameEvent, GamePhase};

impl SowEngine {
    pub(super) fn apply_build_structure_intent(
        &mut self,
        player_id: u16,
        kind: BuildingKind,
        target_tile: u32,
    ) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        let Some(player) = self.state.player(player_id) else {
            return;
        };
        if !player.alive {
            return;
        }
        if !structure_kind_enabled(kind) {
            return;
        }
        let w = self.state.map.width;
        let area = w.saturating_mul(self.state.map.height);
        if area == 0 || target_tile >= area {
            return;
        }

        // Check for a nearby same-kind building to stack onto.
        if let Some(target_id) = crate::building::find_upgrade_target_id(
            &self.state.map,
            player_id,
            kind,
            target_tile,
            &self.buildings,
        ) {
            let count = crate::building::count_kind(&self.buildings, player_id, kind);
            let cost = structure_build_cost_gold(kind, count, &self.state.config);
            let Some(player_mut) = self.state.player_mut(player_id) else {
                return;
            };
            if player_mut.gold < cost || !cost.is_finite() {
                return;
            }
            player_mut.gold = (player_mut.gold - cost).max(0.0);

            let idx = self
                .buildings
                .binary_search_by_key(&target_id, |b| b.id)
                .unwrap();
            let b = &mut self.buildings[idx];
            b.level = b.level.saturating_add(1);
            let dur = crate::building::core::upgrade_duration_ticks(b.kind, b.level);
            b.under_construction = true;
            b.ticks_until_complete = b.ticks_until_complete.saturating_add(dur);
            self.building_aggregates_dirty = true;

            self.state.events.push(GameEvent::StructureUpgraded {
                id: b.id,
                tile_idx: b.tile_idx,
                kind: b.kind,
                level: b.level,
            });
            return;
        }

        // No stackable candidate — normal placement.
        self.refresh_building_grid();
        let Some(spawn_idx) = resolve_structure_spawn_tile(
            &self.state.map,
            player_id,
            kind,
            target_tile,
            &self.building_grid,
            &self.buildings,
            &mut self.placement_scratch,
        ) else {
            return;
        };

        let count = crate::building::count_kind(&self.buildings, player_id, kind);
        let cost = structure_build_cost_gold(kind, count, &self.state.config);
        let Some(player_mut) = self.state.player_mut(player_id) else {
            return;
        };
        if player_mut.gold < cost || !cost.is_finite() {
            return;
        }
        player_mut.gold = (player_mut.gold - cost).max(0.0);
        let id = self.state.next_building_id;
        self.state.next_building_id = self.state.next_building_id.wrapping_add(1).max(1);
        let dur = kind.construction_duration_ticks();
        let under = dur > 0;
        let ticks = if under { dur } else { 0 };
        self.add_building(Building {
            id,
            owner_id: player_id,
            tile_idx: spawn_idx,
            kind,
            level: 1,
            under_construction: under,
            ticks_until_complete: ticks,
            modules: CityModules::default(),
        });
        self.state.events.push(GameEvent::StructureSpawned {
            id,
            owner_id: player_id,
            tile_idx: spawn_idx,
            kind,
            level: 1,
        });
    }

    pub(super) fn apply_upgrade_city_module_intent(
        &mut self,
        player_id: u16,
        building_id: u64,
        module: ModuleKind,
    ) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        let Some(player) = self.state.player(player_id) else {
            return;
        };
        if !player.alive {
            return;
        }
        let mut found: Option<(usize, u8, u8)> = None;
        if let Ok(idx) = self.buildings.binary_search_by_key(&building_id, |b| b.id) {
            let b = &self.buildings[idx];
            if b.owner_id == player_id && b.kind == BuildingKind::City && !b.under_construction {
                let current_level = b.modules.get_level(module);
                found = Some((idx, current_level, b.level));
            }
        }
        let Some((idx, current_level, city_level)) = found else {
            return;
        };

        let new_level = current_level.saturating_add(1);
        if new_level > 5 {
            return;
        }
        if module == ModuleKind::Arsenal && new_level > 3 {
            return;
        }

        match module {
            ModuleKind::Intel => {
                if city_level < 2 {
                    return;
                }
            }
            ModuleKind::Arsenal | ModuleKind::Shield if city_level < 3 => {
                return;
            }
            _ => {}
        }

        if module == ModuleKind::Port {
            let city_tile = self.buildings[idx].tile_idx;
            let (cx, cy) = crate::building::idx_xy(city_tile, self.state.map.width);
            if !crate::building::is_shore_land_tile(&self.state.map, cx, cy) {
                return;
            }
        }

        if module == ModuleKind::Arsenal && current_level == 0 {
            let has_arsenal = self
                .buildings
                .iter()
                .any(|b| b.owner_id == player_id && b.modules.arsenal > 0);
            if has_arsenal {
                return;
            }
        }

        let cost = crate::building::module_upgrade_cost_gold(module, new_level);
        let Some(player_mut) = self.state.player_mut(player_id) else {
            return;
        };
        if player_mut.gold < cost || !cost.is_finite() {
            return;
        }
        player_mut.gold = (player_mut.gold - cost).max(0.0);

        let b = &mut self.buildings[idx];
        b.modules.set_level(module, new_level);
        self.building_aggregates_dirty = true;

        if module == ModuleKind::Port {
            self.sea_lanes_dirty = true;
        }

        self.state.events.push(GameEvent::StructureUpgraded {
            id: building_id,
            tile_idx: b.tile_idx,
            kind: b.kind,
            level: b.level,
        });
    }

    pub(super) fn apply_upgrade_tile_intent(&mut self, player_id: u16, tile_idx: u32) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        let Some(player) = self.state.player(player_id) else {
            return;
        };
        if !player.alive {
            return;
        }

        let w = self.state.map.width;
        let h = self.state.map.height;
        if tile_idx >= w * h {
            return;
        }

        if self.state.map.owner_id(tile_idx % w, tile_idx / w) != player_id {
            return;
        }

        let current_level = self.state.map.tile_upgrades[tile_idx as usize];
        let new_level = current_level.saturating_add(1);

        let s = crate::config::GOLD_SCALE.max(1.0);
        let cost = (1000.0 * 1.5f64.powi(current_level as i32)) / s;

        let Some(player_mut) = self.state.player_mut(player_id) else {
            return;
        };
        if player_mut.gold < cost || !cost.is_finite() {
            return;
        }
        player_mut.gold = (player_mut.gold - cost).max(0.0);

        self.state.map.tile_upgrades[tile_idx as usize] = new_level;
        self.state.map.dirty_tiles.push(tile_idx as usize);

        self.state.events.push(GameEvent::TileUpgraded {
            tile_idx,
            level: new_level,
        });
    }
}
