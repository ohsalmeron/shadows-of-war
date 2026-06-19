#[cfg(test)]
mod bot_iq_alliance_tests {
    use crate::engine::SowEngine;
    use crate::game::{BuildingKind, GamePhase, GameState};
    use crate::intent::nation::structures::bot_structure_target_count;
    use crate::player::Player;
    use crate::water_components::WaterComponents;

    fn test_engine_two_players(seed: u64) -> SowEngine {
        let mut game = GameState::new(seed, 8, 8, crate::game_config::GameConfig::default());
        game.phase = GamePhase::Playing;

        // Player 1 (Bot, IQ 135 - High IQ)
        let mut p1 = Player::new_bot(
            1,
            "Bot1".into(),
            [1.0, 0.0, 0.0],
            &crate::game_config::GameConfig::default(),
        );
        p1.iq = 135;
        p1.iq_points = 50.0;
        p1.troops = 1000.0;
        p1.max_troops = 1500.0;
        p1.gold = 300_000.0;
        p1.tile_count = 10;
        p1.border_insert(0); // Tile (0, 0)
        game.players.push(p1);

        // Player 2 (Bot, IQ 85 - Low IQ)
        let mut p2 = Player::new_bot(
            2,
            "Bot2".into(),
            [0.0, 1.0, 0.0],
            &crate::game_config::GameConfig::default(),
        );
        p2.iq = 85;
        p2.iq_points = 50.0;
        p2.troops = 100.0;
        p2.max_troops = 200.0;
        p2.gold = 10_000.0;
        p2.tile_count = 5;
        p2.border_insert(1); // Tile (1, 0)
        game.players.push(p2);

        game.player_lookup = vec![None, Some(0), Some(1)];

        // Set map ownerships to make them neighbors
        game.map.set_owner_id(0, 0, 1);
        game.map.set_owner_id(1, 0, 2);

        // Make both land tiles
        let idx0 = game.map.ref_id(0, 0);
        game.map.terrain[idx0] = crate::map::MapTile::from_byte(0b1000_0000);
        let idx1 = game.map.ref_id(1, 0);
        game.map.terrain[idx1] = crate::map::MapTile::from_byte(0b1000_0000);

        SowEngine::new(game, WaterComponents::default())
    }

    #[test]
    fn test_execute_income_iq_points_accumulation() {
        let mut engine = test_engine_two_players(42);
        engine.state.config.global_speed_multiplier = 1.0;
        engine.state.config.tick_rate_ms = 100.0;

        // Prior to income
        assert_eq!(engine.state.player(1).unwrap().iq_points, 50.0);
        assert_eq!(engine.state.player(2).unwrap().iq_points, 50.0);

        // Tick income
        engine.execute_income();

        // High IQ (135): per_tick(1.35) = 1.35 * 0.1 * 1.0 = 0.135
        assert_eq!(engine.state.player(1).unwrap().iq_points, 50.135);
        // Low IQ (85): per_tick(0.85) = 0.85 * 0.1 * 1.0 = 0.085
        assert_eq!(engine.state.player(2).unwrap().iq_points, 50.085);
    }

    #[test]
    fn test_alliance_proposal_threshold_high_iq() {
        let mut engine = test_engine_two_players(42);
        // Ensure bot 1 can afford alliance
        engine.state.player_mut(1).unwrap().iq_points = 100.0;
        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        // Since bot 1 has IQ 135, it only proposes if target troops > 0.8 * me_troops.
        // Bot 2 has 100 troops, Bot 1 has 1000. It should NOT propose an alliance.
        assert!(
            engine.alliances_proposed.is_empty(),
            "High IQ bot should not propose to weak neighbor"
        );
    }

    #[test]
    fn test_attack_context_betrayal_not_timer_driven() {
        let mut engine = test_engine_two_players(42);
        let p1 = engine.state.player_mut(1).unwrap();
        p1.iq_points = 100.0;
        p1.player_type = crate::player::PlayerType::Nation;
        p1.alliances.push(2);
        p1.alliance_timers.insert(2, 100);
        p1.troops = 5000.0;
        let p2 = engine.state.player_mut(2).unwrap();
        p2.alliances.push(1);
        p2.alliance_timers.insert(1, 100);
        p2.troops = 500.0;
        // No neutral land — boxed in with ally only.
        let mut broke_alliance = false;
        for _ in 0..120 {
            engine.state.tick += 1;
            engine.execute_ai_think();
            if !engine.state.player(1).unwrap().alliances.contains(&2) {
                broke_alliance = true;
                break;
            }
        }
        assert!(
            broke_alliance,
            "strong nation should betray weak bordering ally via attack-context logic"
        );
    }

    #[test]
    fn test_proactive_two_x_betrayal_removed() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 100.0;
        engine.state.player_mut(1).unwrap().alliances.push(2);
        engine.state.player_mut(2).unwrap().alliances.push(1);
        engine
            .state
            .player_mut(1)
            .unwrap()
            .alliance_timers
            .insert(2, 500);
        engine
            .state
            .player_mut(2)
            .unwrap()
            .alliance_timers
            .insert(1, 500);
        engine.state.player_mut(1).unwrap().troops = 2500.0;
        engine.state.player_mut(2).unwrap().troops = 1000.0;
        // Give bot 1 neutral expansion option so diplomacy propose is skipped; 2x should not auto-break.
        engine.state.map.set_owner_id(2, 0, 0);
        let idx = engine.state.map.ref_id(2, 0);
        engine.state.map.terrain[idx] = crate::map::MapTile::from_byte(0b1000_0000);
        for _ in 0..20 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        assert!(
            engine.state.player(1).unwrap().alliances.contains(&2),
            "2x troop advantage alone must not trigger timer betrayal"
        );
    }

    #[test]
    fn test_density_upgrade_logic() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 10_000_000.0;
        engine.state.player_mut(1).unwrap().tile_count = 100; // Small area
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;

        // Add max structures to force upgrade
        for i in 0..15 {
            engine.buildings.push(crate::building::Building {
                id: i,
                owner_id: 1,
                tile_idx: 0,
                kind: crate::game::BuildingKind::City,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
                modules: crate::building::CityModules::default(),
            });
        }
        engine.refresh_building_grid();
        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        // As long as this executes without panic we're good
    }

    #[test]
    fn test_frontline_defense_post_prioritization() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;

        // Simulate under attack
        engine.attacks.push(crate::execution::AttackExecution {
            id: 1,
            owner_id: 2,
            target_owner: 1,
            troops: 5000.0,
            to_conquer: Default::default(),
            insert_seq_counter: 0,
            rng: wyrand::WyRand::new(42),
            retreating: false,
        });

        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
    }

    #[test]
    fn test_nuke_launch_sam_avoidance() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 100_000_000.0;
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;

        // Give bot 1 a silo
        let m1 = crate::building::CityModules {
            arsenal: 1,
            ..Default::default()
        };
        engine.buildings.push(crate::building::Building {
            id: 100,
            owner_id: 1,
            tile_idx: 0,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: m1,
        });

        // Give bot 2 a city
        engine.buildings.push(crate::building::Building {
            id: 101,
            owner_id: 2,
            tile_idx: 10,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: crate::building::CityModules::default(),
        });

        // Give bot 2 a SAM covering the city
        let m2 = crate::building::CityModules {
            shield: 1,
            ..Default::default()
        };
        engine.buildings.push(crate::building::Building {
            id: 102,
            owner_id: 2,
            tile_idx: 10,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: m2,
        });

        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        // Since the only target is covered by SAM, it shouldn't launch.
        assert!(engine.recent_nuke_targets.is_empty());
    }

    #[test]
    fn test_alliance_cap_enforced() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        // Bot 1 (IQ 135) allows max 1 alliance. Give it 1 alliance already.
        engine.state.player_mut(1).unwrap().alliances.push(3);

        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        assert!(
            engine.alliances_proposed.is_empty(),
            "High IQ bot should respect alliance cap of 1"
        );
    }

    #[test]
    fn test_nuke_launch_target_centroid() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 100_000_000.0;
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;

        // Give bot 1 a silo
        let m1 = crate::building::CityModules {
            arsenal: 1,
            ..Default::default()
        };
        engine.buildings.push(crate::building::Building {
            id: 100,
            owner_id: 1,
            tile_idx: 0,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: m1,
        });

        // Give bot 2 a city
        engine.buildings.push(crate::building::Building {
            id: 101,
            owner_id: 2,
            tile_idx: 1,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: crate::building::CityModules::default(),
        });

        // Make sure bot 2 actually owns tile 1 so they are neighbors!
        engine.state.map.set_owner_id(1, 0, 2);

        engine.refresh_building_grid();
        let mut decisions = Vec::new();
        engine.maybe_launch_nuke(1, &mut decisions, 135, &[2]);
        assert!(!decisions.is_empty());
        assert_eq!(engine.recent_nuke_targets[0].1, 1);
    }

    #[test]
    fn test_team_alliance_prohibited() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().team = Some(crate::protocol::Team::Red);
        engine.state.player_mut(2).unwrap().team = Some(crate::protocol::Team::Red);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;

        let stamped = crate::protocol::StampedIntent {
            player_id: 1,
            intent: crate::protocol::GameplayIntent::ProposeAlliance { target_player: 2 },
        };
        engine.apply_stamped_intent(&stamped, 0);
        assert!(
            engine.alliances_proposed.is_empty(),
            "Teammates should not be allowed to propose alliance"
        );
    }

    fn test_engine_nation_mid_game() -> SowEngine {
        let w = 64u32;
        let h = 64u32;
        let config = crate::game_config::GameConfig::default();
        let mut game = GameState::new(42, w, h, config.clone());
        game.phase = GamePhase::Playing;

        for t in game.map.terrain.iter_mut() {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }

        let owner = 1u16;
        let mut sum_x = 0u64;
        let mut sum_y = 0u64;
        let mut count = 0u32;
        for y in 10..50 {
            for x in 10..50 {
                game.map.set_owner_id(x, y, owner);
                sum_x += x as u64;
                sum_y += y as u64;
                count += 1;
            }
        }

        let mut nation = Player::new_nation(1, "Nation1".into(), [1.0, 0.0, 0.0], &config);
        nation.iq_points = 500.0;
        nation.gold = 100_000.0;
        nation.troops = 10_000.0;
        nation.tile_count = count;
        nation.sum_x = sum_x;
        nation.sum_y = sum_y;
        for x in 10..50 {
            nation.border_insert(10 * w + x);
            nation.border_insert(49 * w + x);
        }
        for y in 11..49 {
            nation.border_insert(y * w + 10);
            nation.border_insert(y * w + 49);
        }
        game.players.push(nation);
        game.player_lookup = vec![None, Some(0)];

        let mut engine = SowEngine::new(game, WaterComponents::default());
        let city_positions = [(15, 15), (15, 35), (35, 15), (25, 25), (20, 40), (40, 20)];
        for (i, (cx, cy)) in city_positions.iter().enumerate() {
            let tile_idx = cy * w + cx;
            engine.buildings.push(crate::building::Building {
                id: (i as u64) + 1,
                owner_id: 1,
                tile_idx,
                kind: crate::game::BuildingKind::City,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
                modules: crate::building::CityModules::default(),
            });
        }
        engine.refresh_building_grid();
        engine.building_aggregates_dirty = true;
        engine
    }

    fn test_engine_advanced_tribe() -> SowEngine {
        let w = 48u32;
        let h = 48u32;
        let config = crate::game_config::GameConfig::default();
        let mut game = GameState::new(42, w, h, config.clone());
        game.phase = GamePhase::Playing;

        for t in game.map.terrain.iter_mut() {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }

        let owner = 10u16;
        let mut sum_x = 0u64;
        let mut sum_y = 0u64;
        let mut count = 0u32;
        for y in 4..28 {
            for x in 4..28 {
                game.map.set_owner_id(x, y, owner);
                sum_x += x as u64;
                sum_y += y as u64;
                count += 1;
            }
        }

        let mut tribe = Player::new_bot(10, "Tribe10".into(), [0.0, 1.0, 0.0], &config);
        tribe.iq = 110;
        tribe.iq_points = 500.0;
        tribe.gold = 50_000.0;
        tribe.troops = 5_000.0;
        tribe.tile_count = count;
        tribe.sum_x = sum_x;
        tribe.sum_y = sum_y;
        for x in 4..28 {
            tribe.border_insert(4 * w + x);
            tribe.border_insert(27 * w + x);
        }
        for y in 5..27 {
            tribe.border_insert(y * w + 4);
            tribe.border_insert(y * w + 27);
        }
        game.players.push(tribe);
        let mut lookup = vec![None; 11];
        lookup[10] = Some(0);
        game.player_lookup = lookup;

        let mut engine = SowEngine::new(game, WaterComponents::default());
        engine.refresh_building_grid();
        engine.building_aggregates_dirty = true;
        engine
    }

    #[test]
    fn test_nation_keeps_building_mid_game() {
        let mut engine = test_engine_nation_mid_game();
        let initial_count = engine.buildings.len();
        engine.state.config.global_speed_multiplier = 1.0;
        for _ in 0..2000 {
            engine.state.tick += 1;
            engine.execute_income();
            engine.execute_ai_think();
        }
        assert!(
            engine.buildings.len() > initial_count,
            "nation should keep placing structures mid-game (had {initial_count}, now {})",
            engine.buildings.len()
        );
    }

    #[test]
    fn test_advanced_tribe_can_build() {
        let mut engine = test_engine_advanced_tribe();
        let initial_count = engine.buildings.len();
        engine.state.config.global_speed_multiplier = 1.0;
        for _ in 0..1000 {
            engine.state.tick += 1;
            engine.execute_income();
            engine.execute_ai_think();
        }
        assert!(
            engine.buildings.len() > initial_count,
            "advanced tribe (id % 10) should build structures (had {initial_count}, now {})",
            engine.buildings.len()
        );
    }

    fn run_building_sim_ticks(engine: &mut SowEngine, ticks: u64) {
        engine.state.config.global_speed_multiplier = 1.0;
        for _ in 0..ticks {
            engine.state.tick += 1;
            engine.execute_income();
            engine.execute_ai_think();
        }
    }

    type BuildingSimFingerprint = (usize, u64, f64, f64, Vec<(u64, u32, u8, u8)>);

    fn building_sim_fingerprint(engine: &SowEngine, player_id: u16) -> BuildingSimFingerprint {
        let mut snaps: Vec<(u64, u32, u8, u8)> = engine
            .buildings
            .iter()
            .filter(|b| b.owner_id == player_id)
            .map(|b| (b.id, b.tile_idx, b.kind as u8, b.level))
            .collect();
        snaps.sort_by_key(|s| s.0);
        let level_sum: u64 = snaps.iter().map(|s| s.3 as u64).sum();
        let gold = engine
            .state
            .player(player_id)
            .map(|p| p.gold)
            .unwrap_or(0.0);
        let iq_pts = engine
            .state
            .player(player_id)
            .map(|p| p.iq_points)
            .unwrap_or(0.0);
        (snaps.len(), level_sum, gold, iq_pts, snaps)
    }

    #[test]
    fn test_ai_building_simulation_is_deterministic() {
        let mut a = test_engine_nation_mid_game();
        run_building_sim_ticks(&mut a, 500);
        let fp_a = building_sim_fingerprint(&a, 1);

        let mut b = test_engine_nation_mid_game();
        run_building_sim_ticks(&mut b, 500);
        let fp_b = building_sim_fingerprint(&b, 1);

        assert_eq!(
            fp_a, fp_b,
            "identical seed/setup must produce identical building state after 500 ticks"
        );
    }

    #[test]
    fn test_bot_structure_target_count_floor_is_stable() {
        // Low IQ: factor 0.1 caps non-city kinds at 0, city at least 1
        assert_eq!(bot_structure_target_count(BuildingKind::City, 10, 85), 1);
        assert_eq!(bot_structure_target_count(BuildingKind::Bunker, 10, 85), 0);
        // Mid IQ: 50% of high-IQ quotas, deterministic floor
        assert_eq!(bot_structure_target_count(BuildingKind::Factory, 8, 110), 2);
        // High IQ: full quotas
        assert_eq!(bot_structure_target_count(BuildingKind::Port, 10, 140), 3);
    }

    #[test]
    fn test_bot_build_stacks_like_player() {
        let w = 32u32;
        let config = crate::game_config::GameConfig::default();
        let mut game = GameState::new(42, w, w, config.clone());
        game.phase = GamePhase::Playing;
        for t in game.map.terrain.iter_mut() {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }
        for y in 0..w {
            for x in 0..w {
                game.map.set_owner_id(x, y, 1);
            }
        }
        let mut nation = Player::new_nation(1, "N".into(), [1.0, 0.0, 0.0], &config);
        nation.gold = 1_000_000.0;
        nation.iq = 140;
        nation.iq_points = 500.0;
        nation.tile_count = w * w;
        game.players.push(nation);
        game.player_lookup = vec![None, Some(0)];

        let mut engine = SowEngine::new(game, WaterComponents::default());
        engine.buildings.push(crate::building::Building {
            id: 1,
            owner_id: 1,
            tile_idx: 16 * w + 16,
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: crate::building::CityModules::default(),
        });
        engine.refresh_building_grid();

        let city_tile = 16 * w + 16;
        engine.apply_stamped_intent(
            &crate::protocol::StampedIntent {
                player_id: 1,
                intent: crate::protocol::GameplayIntent::BuildStructure {
                    kind: BuildingKind::City,
                    target_tile: city_tile,
                },
            },
            0,
        );

        assert_eq!(
            engine.buildings.len(),
            1,
            "stack must not spawn a second city"
        );
        assert_eq!(engine.buildings[0].level, 2);
    }

    #[test]
    fn test_tribe_buildings_not_purged() {
        let mut engine = test_engine_two_players(42);
        engine.buildings.push(crate::building::Building {
            id: 50,
            owner_id: 2,
            tile_idx: 1,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: crate::building::CityModules::default(),
        });
        engine.building_aggregates_dirty = true;
        engine.execute_income();
        assert!(
            engine
                .buildings
                .iter()
                .any(|b| b.id == 50 && b.owner_id == 2),
            "standard tribe buildings must not be deleted by income tick"
        );
    }
}
