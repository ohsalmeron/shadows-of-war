pub mod construction;
pub mod core;
pub mod cost;
pub mod placement;
pub mod upgrade;

pub use core::*;
pub use cost::*;
pub use placement::*;
pub use upgrade::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::BuildingKind;
    use crate::map::{GameMap, MapTile};

    fn tiny_owned_map() -> (GameMap, u16) {
        let mut m = GameMap::new(5, 1);
        let owner = 1u16;
        for x in 0..5 {
            m.set_owner_id(x, 0, owner);
            let ri = m.ref_id(x, 0);
            m.terrain[ri] = MapTile::from_byte(0b1000_0000); // land
        }
        // Shore at ends for port tests
        let r0 = m.ref_id(0, 0);
        m.terrain[r0] = MapTile::from_byte(0b1100_0000);
        let r4 = m.ref_id(4, 0);
        m.terrain[r4] = MapTile::from_byte(0b1100_0000);
        (m, owner)
    }

    #[test]
    fn valid_land_center_click() {
        let (map, owner) = tiny_owned_map();
        let grid = BuildingGrid::rebuild_empty(map.width, map.height);
        let mut scratch = crate::engine::PlacementScratch::default();
        let v = valid_land_structure_indices(
            &map,
            owner,
            2,
            BuildingKind::City,
            &grid,
            &[],
            &mut scratch,
        );
        assert!(v.contains(&2));
    }

    #[test]
    fn spacing_excludes_nearby() {
        let w = 32u32;
        let mut m = GameMap::new(w, 1);
        let owner = 1u16;
        for x in 0..w {
            m.set_owner_id(x, 0, owner);
            let ri = m.ref_id(x, 0);
            m.terrain[ri] = MapTile::from_byte(0b1000_0000);
        }
        let click = 16u32;
        // Structure one tile away from click: click tile is excluded (too close).
        let mut grid = BuildingGrid::default();
        grid.rebuild_from_pairs(w, 1, &[(15u32, 0u32)]);
        let mut scratch = crate::engine::PlacementScratch::default();
        let v = valid_land_structure_indices(
            &m,
            owner,
            click,
            BuildingKind::City,
            &grid,
            &[],
            &mut scratch,
        );
        assert!(!v.contains(&15));
        assert!(!v.is_empty());
    }

    #[test]
    fn upgrade_closest_by_manhattan() {
        let w = 20u32;
        let mut b1 = Building {
            id: 1,
            owner_id: 1,
            tile_idx: xy_idx(5, 5, w),
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: crate::building::CityModules::default(),
        };
        let b2 = Building {
            id: 2,
            owner_id: 1,
            tile_idx: xy_idx(6, 5, w),
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: crate::building::CityModules::default(),
        };
        let map = GameMap::new(w, 20);
        let click = xy_idx(5, 5, w);
        let id = find_upgrade_target_id(&map, 1, BuildingKind::City, click, &[b1, b2]);
        assert_eq!(id, Some(1));

        b1.under_construction = true;
        let id2 = find_upgrade_target_id(&map, 1, BuildingKind::City, click, &[b1, b2]);
        assert_eq!(id2, Some(1));
    }

    #[test]
    fn construction_tick_emits_structure_ready() {
        use crate::engine::SowEngine;
        use crate::game::{GameEvent, GamePhase, GameState};
        use crate::player::Player;
        use crate::water_components::WaterComponents;

        let mut game = GameState::new(3, 5, 1, crate::game_config::GameConfig::default());
        game.phase = GamePhase::Playing;
        let (map, owner) = tiny_owned_map();
        game.map = map;
        game.players.push(Player::new_human(
            owner,
            "c".into(),
            [1.0, 0.0, 0.0],
            &crate::game_config::GameConfig::default(),
        ));
        game.player_lookup.resize(owner as usize + 1, None);
        game.player_lookup[owner as usize] = Some(0);
        let water = WaterComponents::compute(&game.map, |_| {});
        let mut engine = SowEngine::new(game, water);
        engine.buildings.push(Building {
            id: 7,
            owner_id: owner,
            tile_idx: 2,
            kind: BuildingKind::City,
            level: 1,
            under_construction: true,
            ticks_until_complete: 1,
            modules: crate::building::CityModules::default(),
        });
        engine.execute_construction();
        assert!(engine
            .state
            .events
            .iter()
            .any(|e| matches!(e, GameEvent::StructureReady { id: 7, .. })));
    }

    #[test]
    fn aggregate_ignores_under_construction() {
        let b = [
            Building {
                id: 1,
                owner_id: 1,
                tile_idx: 0,
                kind: BuildingKind::City,
                level: 1,
                under_construction: true,
                ticks_until_complete: 3,
                modules: crate::building::CityModules::default(),
            },
            Building {
                id: 2,
                owner_id: 1,
                tile_idx: 1,
                kind: BuildingKind::City,
                level: 2,
                under_construction: false,
                ticks_until_complete: 0,
                modules: crate::building::CityModules::default(),
            },
        ];
        let aggs = aggregate_buildings_per_player(b.into_iter(), 2);
        assert_eq!(aggs[1].city_levels, 2);
        assert_eq!(aggs[1].count_city, 2);
        assert_eq!(aggs[1].ready_city_count, 1);
    }

    #[test]
    fn defense_post_bonus_scales_with_level() {
        let w = 20u32;
        let b = Building {
            id: 1,
            owner_id: 2,
            tile_idx: xy_idx(10, 10, w),
            kind: BuildingKind::Bunker,
            level: 2,
            under_construction: false,
            ticks_until_complete: 0,
            modules: crate::building::CityModules::default(),
        };
        let cfg = crate::game_config::GameConfig::default();
        let bonus = defense_post_priority_bonus(&[b], 10, 10, w, &cfg);
        assert_eq!(bonus, cfg.bunker_priority as i64 * 2);
        let bonus_far = defense_post_priority_bonus(&[b], 0, 0, w, &cfg);
        assert_eq!(bonus_far, 0);
    }
}
