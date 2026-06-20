use crate::engine::SowEngine;

impl SowEngine {
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

        let map_spawns_snapshot: Vec<crate::map_file::MapSpawn> = self.state.map_spawns.clone();

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
                    while !found_fallback
                        && attempts_fallback < 100
                        && !fallback_nations_indices.is_empty()
                    {
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

                        if !used_names.contains(&formatted_name)
                            && !used_names.contains(raw_tribe_name)
                        {
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

    pub(crate) fn find_valid_spawn(&self, rng: &mut wyrand::WyRand) -> Option<(u32, u32)> {
        use crate::rng::NextIntExt;
        let mut tries = 0;

        while tries < 1000 {
            let sx = rng.next_int(0, self.state.map.width as i32) as u32;
            let sy = rng.next_int(0, self.state.map.height as i32) as u32;

            if self.state.map.terrain[self.state.map.ref_id(sx, sy)].is_water() {
                tries += 1;
                continue;
            }

            let dist: i32 = if tries < 300 {
                15
            } else if tries < 600 {
                10
            } else if tries < 900 {
                5
            } else {
                0
            };

            let mut valid = true;

            for dy in -dist..=dist {
                for dx in -dist..=dist {
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

        // Absolute fallback: scan the map for any land tile (preferably unowned)
        let mut first_land = None;
        for y in 0..self.state.map.height {
            for x in 0..self.state.map.width {
                if self.state.map.terrain[self.state.map.ref_id(x, y)].is_land() {
                    if self.state.map.owner_id(x, y) == 0 {
                        return Some((x, y));
                    }
                    if first_land.is_none() {
                        first_land = Some((x, y));
                    }
                }
            }
        }
        if let Some(pos) = first_land {
            return Some(pos);
        }

        // Extreme fallback: scan the map for any unowned tile
        for y in 0..self.state.map.height {
            for x in 0..self.state.map.width {
                if self.state.map.owner_id(x, y) == 0 {
                    return Some((x, y));
                }
            }
        }

        None
    }
}
