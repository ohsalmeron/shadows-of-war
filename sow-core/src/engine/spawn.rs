use crate::engine::SowEngine;

pub struct HumanSpawn {
    pub player_id: u16,
    pub name: String,
    pub color: [f32; 3],
    pub team: Option<crate::protocol::Team>,
    pub civilization: crate::player::Civilization,
    pub leader: crate::player::Leader,
    pub is_ai_controlled: bool,
}

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

        // Geo-database candidates projected into this map's bounds. Empty when
        // the map carries no geography (fictional maps) → behavior unchanged.
        struct GeoCand {
            name: &'static str,
            x: u32,
            y: u32,
        }
        let mut geo_nations: Vec<GeoCand> = Vec::new();
        let mut geo_tribes: Vec<&'static str> = Vec::new();
        if let Some(bounds) = self.state.geo_bounds {
            let (map_w, map_h) = (self.state.map.width, self.state.map.height);
            for entity in crate::geo_entities::all() {
                if let Some((x, y)) =
                    bounds.project(entity.lat as f64, entity.lon as f64, map_w, map_h)
                {
                    if entity.kind == crate::geo_entities::EntityKind::Tribe {
                        geo_tribes.push(entity.name);
                    } else {
                        geo_nations.push(GeoCand {
                            name: entity.name,
                            x,
                            y,
                        });
                    }
                }
            }
            log::info!(
                "spawn_ai: geo candidates inside bounds: {} nations, {} tribes",
                geo_nations.len(),
                geo_tribes.len()
            );
        }
        let mut geo_nation_indices: Vec<usize> = (0..geo_nations.len()).collect();
        let mut geo_tribe_indices: Vec<usize> = (0..geo_tribes.len()).collect();

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
                let mut found_name = false;

                // Geo tier: historical entities inside the map bounds, placed
                // at (or nearest free land to) their real-world locations.
                while !found_name && !geo_nation_indices.is_empty() {
                    let idx = (rng.rand() as usize) % geo_nation_indices.len();
                    let cand = &geo_nations[geo_nation_indices[idx]];
                    geo_nation_indices.swap_remove(idx);
                    if used_names.contains(cand.name) {
                        continue;
                    }
                    name = cand.name.to_string();
                    used_names.insert(name.clone());
                    spawn_point = self.nearest_free_land(cand.x, cand.y);
                    found_name = true;
                }

                // We need extra nations! Grab from HISTORICAL_CIVILIZATIONS and ensure no duplicate of any used name
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
                // Teams are human-only (real + ai-controlled). In "Teams" mode
                // nations stay wild (team=None). In "HumansVsNations" the nations
                // form the opposing Blue team; humans are Red (forced at lobby).
                let team = if config.game_mode == "HumansVsNations" {
                    Some(crate::protocol::Team::Blue)
                } else {
                    None
                };
                let color = crate::player::human_shader_territory_rgb(bot_id);

                let mut player = Player::new_nation(bot_id, name, color, &config);
                player.team = team;
                self.state.spawn_player(player, sx, sy);
                spawned_city_states += 1;
            }
        }

        // Spawn tribes (IDs above city-states).
        // Tribes use historical/geo names for flavor, but spawn dynamically
        // across all available land tiles with distance separation (OpenFront-style)
        // rather than clustering on historical centroids.
        let tribe_start_id = 104 + total_city_states_to_spawn as u16;
        let fallback_pool = crate::tribes::FALLBACK_TRIBES;
        let mut fallback_indices: Vec<usize> = (0..fallback_pool.len()).collect();

        for i in 0..tribe_count {
            let bot_id = tribe_start_id + i as u16;

            let mut name = String::new();
            let mut found_name = false;
            let mut attempts = 0;

            // Geo tier: historical tribe names inside the map bounds.
            while !found_name && !geo_tribe_indices.is_empty() {
                let idx = (rng.rand() as usize) % geo_tribe_indices.len();
                let cand_name = geo_tribes[geo_tribe_indices[idx]];
                geo_tribe_indices.swap_remove(idx);
                if used_names.contains(cand_name) {
                    continue;
                }
                name = cand_name.to_string();
                used_names.insert(name.clone());
                found_name = true;
            }

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

            let spawn_point = self.find_valid_spawn(&mut rng);

            if let Some((sx, sy)) = spawn_point {
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

    pub fn spawn_human(&mut self, spawn: HumanSpawn) {
        let HumanSpawn {
            player_id,
            name,
            color,
            team,
            civilization,
            leader,
            is_ai_controlled,
        } = spawn;
        use crate::player::Player;
        use wyrand::WyRand;

        // Use a different seed offset for human to avoid clashing exactly with bots
        let mut rng = WyRand::new(self.state.seed.wrapping_add(player_id as u64));
        let config = self.state.config.clone();

        // Ghost (is_ai_controlled) Humans are TOP of the food chain — the
        // novice's guide. Single top band (above nations, far above tribes);
        // no id-based tiers. Deterministic per (seed, player_id) →
        // lockstep-safe across clients.
        let ghost_iq: Option<u32> = if is_ai_controlled {
            use crate::rng::NextIntExt;
            let mut iq_rng = WyRand::new(
                self.state
                    .seed
                    .wrapping_add(player_id as u64)
                    .wrapping_add(0xA11CE),
            );
            Some(iq_rng.next_int(160, 181) as u32)
        } else {
            None
        };

        // Campaign: place the human at a fixed homeland tile (auto-spawn, no picker).
        if let Some((tx, ty)) = config.player_spawn {
            if let Some((sx, sy)) = self.nearest_free_land(tx, ty) {
                let mut player = Player::new_human(player_id, name, color, &config);
                player.team = team;
                player.civilization = civilization;
                player.leader = leader;
                player.is_ai_controlled = is_ai_controlled;
                if let Some(iq) = ghost_iq {
                    player.iq = iq;
                }
                self.state.spawn_player(player, sx, sy);
                log::info!("spawn_human: scripted at ({},{})", sx, sy);
                return;
            }
            log::warn!("spawn_human: no land near scripted spawn ({},{})", tx, ty);
        }

        if !config.random_spawn {
            let mut player = Player::new_human(player_id, name, color, &config);
            player.team = team;
            player.civilization = civilization;
            player.leader = leader;
            player.is_ai_controlled = is_ai_controlled;
            if let Some(iq) = ghost_iq {
                player.iq = iq;
            }
            self.state.register_player(player);
            return;
        }

        // OF teamSpawnArea parity (GameImpl.ts:960 + per-map manifests): each
        // team owns a map half (Team {Red, Blue} → Red left, Blue right) and
        // EVERY member spawns inside it — the zone is the cohesion, the
        // member floor is the separation. Ring/anchor path kept as fallback.
        let spawn_point = team
            .as_ref()
            .map(|t| self.team_spawn_area(t))
            .and_then(|area| self.find_spawn_in_area(&mut rng, area))
            .or_else(|| {
                team.as_ref()
                    .and_then(|t| self.team_centroid(t))
                    .and_then(|(cx, cy)| self.find_valid_spawn_near(&mut rng, cx, cy, 12, 36))
            })
            .or_else(|| self.find_valid_spawn(&mut rng));

        if let Some((sx, sy)) = spawn_point {
            let mut player = Player::new_human(player_id, name, color, &config);
            player.team = team;
            player.civilization = civilization;
            player.leader = leader;
            player.is_ai_controlled = is_ai_controlled;
            if let Some(iq) = ghost_iq {
                player.iq = iq;
            }
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

    pub fn spawn_scripted(&mut self) {
        use crate::player::Player;
        let spawns = self.state.config.scripted_spawns.clone();
        if spawns.is_empty() {
            return;
        }
        let config = self.state.config.clone();
        let mut placed = 0;
        for (i, s) in spawns.iter().enumerate() {
            let bot_id = 104 + i as u16;
            let Some((sx, sy)) = self.nearest_free_land(s.x, s.y) else {
                log::warn!(
                    "spawn_scripted: no land near '{}' ({},{})",
                    s.name,
                    s.x,
                    s.y
                );
                continue;
            };
            let mut player = if s.is_nation {
                Player::new_nation(bot_id, s.name.clone(), s.color, &config)
            } else {
                Player::new_bot(bot_id, s.name.clone(), s.color, &config)
            };
            player.team = s.team;
            player.leader = s.leader;
            player.civilization = s.civilization;
            if let Some(t) = s.troops {
                player.troops = t;
            }
            if let Some(c) = s.troop_cap {
                player.max_troops = c;
                player.max_troops_cap = Some(c);
                player.troops = player.troops.min(c);
            }
            if let Some(iq) = s.iq {
                player.iq = iq;
            }
            self.state.spawn_player(player, sx, sy);
            placed += 1;
            log::info!(
                "spawn_scripted: [{}] '{}' team={:?} at ({},{})",
                bot_id,
                s.name,
                s.team,
                sx,
                sy
            );
        }
        log::info!("spawn_scripted: placed {}/{}", placed, spawns.len());
    }

    /// OF teamSpawnArea parity (GameImpl.ts:960 + per-map manifests): each
    /// team owns a map half — `Team {Red, Blue}` → Red left, Blue right — and
    /// every member spawns inside it (2 teams → left/right halves, the same
    /// split OF's manifests hand-author).
    pub(crate) fn team_spawn_area(&self, team: &crate::protocol::Team) -> (u32, u32, u32, u32) {
        let (w, h) = (self.state.map.width, self.state.map.height);
        let half = w / 2;
        match team {
            crate::protocol::Team::Red => (0, 0, half, h),
            crate::protocol::Team::Blue => (half, 0, w - half, h),
        }
    }

    /// Sample free land inside a team's area, keeping the member floor to
    /// every other home. If the floor can't be met the area still wins —
    /// floor is waived and any free land in the zone is taken. Zone cohesion
    /// outranks spacing; spacing never pushes a member out of its zone.
    pub(crate) fn find_spawn_in_area(
        &self,
        rng: &mut wyrand::WyRand,
        area: (u32, u32, u32, u32),
    ) -> Option<(u32, u32)> {
        use crate::rng::NextIntExt;
        let (ax, ay, aw, ah) = area;
        let sample = |rng: &mut wyrand::WyRand| -> Option<(u32, u32)> {
            if aw == 0 || ah == 0 {
                return None;
            }
            let nx = ax + rng.next_int(0, aw as i32) as u32;
            let ny = ay + rng.next_int(0, ah as i32) as u32;
            let ux = nx.min(ax + aw - 1);
            let uy = ny.min(ay + ah - 1);
            if self.state.map.owner_id(ux, uy) != 0
                || !self.state.map.terrain[self.state.map.ref_id(ux, uy)].is_land()
            {
                None
            } else {
                Some((ux, uy))
            }
        };
        for _ in 0..300 {
            if let Some((ux, uy)) = sample(rng)
                && self.home_clear(ux, uy)
            {
                return Some((ux, uy));
            }
        }
        for _ in 0..300 {
            if let Some(free) = sample(rng) {
                return Some(free);
            }
        }
        None
    }

    /// Member floor: ≥14 tiles between any two homes ("cerca sí, encimados
    /// no", widened after the Aug 27 follow-up screenshot — first cut was an
    /// 8-tile floor and blobs still rendered stacked).
    pub(crate) fn home_clear(&self, ux: u32, uy: u32) -> bool {
        const MEMBER_FLOOR_SQ: f64 = 196.0;
        self.state.players.iter().all(|p| {
            if p.tile_count == 0 {
                return true;
            }
            let hx = p.sum_x as f64 / p.tile_count as f64;
            let hy = p.sum_y as f64 / p.tile_count as f64;
            let (dx, dy) = (hx - ux as f64, hy - uy as f64);
            dx * dx + dy * dy >= MEMBER_FLOOR_SQ
        })
    }

    /// OpenFront teamSpawnArea fallback: same-team members cluster around
    /// their first spawned member instead of scattering across the map.
    pub(crate) fn find_valid_spawn_near(
        &self,
        rng: &mut wyrand::WyRand,
        cx: u32,
        cy: u32,
        min_d: i32,
        max_d: i32,
    ) -> Option<(u32, u32)> {
        use crate::rng::NextIntExt;
        for _ in 0..200 {
            let ang = rng.next_int(0, 360) as f64 * std::f64::consts::TAU / 360.0;
            let dist = rng.next_int(min_d, max_d.max(min_d + 1)) as f64;
            let nx = cx as i32 + (ang.cos() * dist).round() as i32;
            let ny = cy as i32 + (ang.sin() * dist).round() as i32;
            if !self.state.map.is_valid_coord(nx, ny) {
                continue;
            }
            let (ux, uy) = (nx as u32, ny as u32);
            if self.state.map.owner_id(ux, uy) != 0
                || !self.state.map.terrain[self.state.map.ref_id(ux, uy)].is_land()
            {
                continue;
            }
            if self.home_clear(ux, uy) {
                return Some((ux, uy));
            }
        }
        None
    }

    /// Centroid of an already-placed team (first member spawns randomly and
    /// becomes the anchor; everyone else rings around it).
    pub(crate) fn team_centroid(&self, team: &crate::protocol::Team) -> Option<(u32, u32)> {
        let mut sum = (0u64, 0u64, 0u64);
        for p in &self.state.players {
            if p.team.as_ref() == Some(team) && p.tile_count > 0 {
                sum.0 += p.sum_x;
                sum.1 += p.sum_y;
                sum.2 += p.tile_count as u64;
            }
        }
        if sum.2 == 0 {
            return None;
        }
        Some(((sum.0 / sum.2) as u32, (sum.1 / sum.2) as u32))
    }

    fn nearest_free_land(&self, tx: u32, ty: u32) -> Option<(u32, u32)> {
        let map = &self.state.map;
        let free = |x: i32, y: i32| -> bool {
            map.is_valid_coord(x, y)
                && map.owner_id(x as u32, y as u32) == 0
                && map.terrain[map.ref_id(x as u32, y as u32)].is_land()
        };
        let (cx, cy) = (tx as i32, ty as i32);
        if free(cx, cy) {
            return Some((tx, ty));
        }
        for r in 1..=120i32 {
            for dx in -r..=r {
                for dy in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    if free(cx + dx, cy + dy) {
                        return Some(((cx + dx) as u32, (cy + dy) as u32));
                    }
                }
            }
        }
        None
    }
}
