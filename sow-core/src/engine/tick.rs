use crate::engine::SowEngine;

impl SowEngine {
    pub fn tick(&mut self) {
        if let crate::game::GamePhase::Spawning { end_tick } = self.state.phase {
            self.state.tick += 1;

            // ── Stagger ghost (is_ai_controlled) spawns across the deploy window ──
            // Each unspawned ghost has a deterministic spawn moment, uniformly and
            // independently placed in [1, end_tick-1] via WyRand(seed ^ pid). When
            // the current tick reaches that moment, the ghost is placed via the
            // same find_valid_spawn + place_spawn path used by the safety net
            // below. Middle ground between the two old extremes:
            //   - legacy socketed backfill: continuous scatter-storm ("too crazy")
            //   - pre-fix ghosts: invisible all window, then mass-pop at end_tick
            // Each ghost fires AT MOST once; any that miss (no valid tile) are
            // caught by the safety net at the phase transition. Deterministic →
            // lockstep-safe across all clients (same seed, same pid → same moment).
            if end_tick >= 2 {
                use crate::rng::NextIntExt;
                use wyrand::WyRand;
                let now = self.state.tick;
                let seed = self.state.seed;
                // Collect due ghosts in ascending pid order for deterministic
                // tile ownership progression (find_valid_spawn sees prior spawns).
                let due: Vec<u16> = self
                    .state
                    .players
                    .iter()
                    .filter(|p| {
                        p.is_ai_controlled
                            && !p.has_spawned
                            && {
                                let mut m = WyRand::new(seed.wrapping_add(p.id as u64));
                                m.next_int(1, (end_tick as i32) - 1) as u64 == now
                            }
                    })
                    .map(|p| p.id)
                    .collect();
                for pid in due {
                    let mut rng =
                        WyRand::new(seed.wrapping_add(pid as u64).wrapping_add(now));
                    if let Some((sx, sy)) = self.find_valid_spawn(&mut rng) {
                        self.state.place_spawn(pid, sx, sy);
                    }
                }
            }

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

        if self.state.config.game_mode == "Teams"
            || self.state.config.game_mode == "HumansVsNations"
        {
            self.check_team_winner(win_threshold);
        } else {
            self.check_ffa_winner(win_threshold);
        }
    }

    fn check_ffa_winner(&mut self, win_threshold: u32) {
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
            self.end_game(wid, None);
        } else if alive_players == 1 {
            if let Some(wid) = last_alive_id {
                self.end_game(wid, None);
            }
        } else if alive_players == 0 && !self.state.players.is_empty() {
            self.state.phase = crate::game::GamePhase::GameOver;
        }
    }

    pub(crate) fn check_team_winner(&mut self, win_threshold: u32) {
        use crate::protocol::Team;
        use std::collections::HashMap;

        let mut team_tiles: HashMap<Team, u32> = HashMap::new();
        let mut best_player_on_team: HashMap<Team, (u16, u32)> = HashMap::new();
        let mut unaffiliated_with_land = 0;

        for p in &self.state.players {
            if !p.alive || p.tile_count == 0 {
                continue;
            }
            let Some(team) = p.team else {
                unaffiliated_with_land += 1;
                continue;
            };
            *team_tiles.entry(team).or_insert(0) += p.tile_count;
            best_player_on_team
                .entry(team)
                .and_modify(|(best_id, best_tiles)| {
                    if p.tile_count > *best_tiles {
                        *best_id = p.id;
                        *best_tiles = p.tile_count;
                    }
                })
                .or_insert((p.id, p.tile_count));
        }

        let mut map_control_team = None;
        let mut teams_with_land = 0;
        let mut last_team_with_land = None;

        for (&team, &tiles) in &team_tiles {
            if tiles == 0 {
                continue;
            }
            teams_with_land += 1;
            last_team_with_land = Some(team);
            if tiles >= win_threshold {
                map_control_team = Some(team);
            }
        }

        if let Some(team) = map_control_team {
            if let Some(&(wid, _)) = best_player_on_team.get(&team) {
                self.end_game(wid, Some(team));
            }
        } else if teams_with_land == 1 && unaffiliated_with_land == 0 {
            if let Some(team) = last_team_with_land {
                if let Some(&(wid, _)) = best_player_on_team.get(&team) {
                    self.end_game(wid, Some(team));
                }
            }
        } else if teams_with_land == 0
            && unaffiliated_with_land == 0
            && !self.state.players.is_empty()
        {
            self.state.phase = crate::game::GamePhase::GameOver;
        }
    }

    fn end_game(&mut self, winner_id: u16, winning_team: Option<crate::protocol::Team>) {
        self.state.winner = Some(winner_id);
        self.state.winning_team = winning_team;
        self.state.phase = crate::game::GamePhase::GameOver;
        self.state.events.push(crate::game::GameEvent::GameOver {
            winner_id,
            winning_team,
        });
    }
}
