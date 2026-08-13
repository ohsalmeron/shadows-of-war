use super::placement::resolve_build_target_tile;
use crate::app::SowApp;

impl SowApp {
    fn show_observer_notice(&mut self, x: f64, y: f64) {
        let messages = [
            "Spectators don't click! 🍿",
            "Just watch the show! 🎬",
            "Popcorn duty, no buttons! 🍿",
            "No controls in the front row! 🎟️",
            "Sit back, it's live! 📺",
            "You watch, they click! 👀",
            "Hands off, you're spectating! 🙌",
            "This seat is for watching! 💺",
        ];
        let click_seed = (x + y) as usize;
        let msg = messages[click_seed % messages.len()];

        let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
        let offset_mouse_y = y as f32 - 60.0;
        let world_y = (offset_mouse_y - self.input.camera_y) / self.input.camera_zoom;

        self.ui.floating_notices.push(crate::app::FloatingNotice {
            text: msg.to_string(),
            world_x,
            world_y,
            start_time: web_time::Instant::now(),
            duration: web_time::Duration::from_millis(1500),
            color: egui::Color32::from_rgb(203, 213, 225), // slate
        });
    }

    pub(crate) fn try_begin_hold_attack(&mut self, x: f64, y: f64, is_touch: bool) {
        if self.ui.observing {
            self.show_observer_notice(x, y);
            return;
        }
        if self.ui.app.hud_state.selected_nuke_kind.is_some() {
            return;
        }

        let phase = self
            .sim
            .current_snapshot
            .as_ref()
            .map(|s| &s.phase)
            .unwrap_or(&sow_core::game::GamePhase::Lobby);
        if matches!(phase, sow_core::game::GamePhase::Spawning { .. }) {
            return;
        }

        let (col, row) = match self.mouse_to_tile(x, y) {
            Some(res) => res,
            None => return,
        };
        let idx = (row * self.sim.map_w as i32 + col) as usize;
        let owner = self
            .gfx
            .map_renderer
            .as_ref()
            .map(|mr| mr.owners[idx])
            .unwrap_or(0);
        let terrain_byte = self
            .gfx
            .map_renderer
            .as_ref()
            .map(|mr| mr.terrain[idx])
            .unwrap_or(0);
        let is_land = (terrain_byte & 0x80) != 0;
        let my_id = self.sim.my_player_id.unwrap_or(0);

        if is_land && owner != my_id {
            // Verify we actually share a border with the target owner
            let mut shares_border = false;
            if let Some(mr) = self.gfx.map_renderer.as_ref() {
                let map_w = self.sim.map_w as i32;
                let map_h = self.sim.map_h as i32;
                let owners = &mr.owners;
                let terrain = &mr.terrain;

                'outer: for r in 0..map_h {
                    for c in 0..map_w {
                        let idx = (r * map_w + c) as usize;
                        if owners[idx] == my_id {
                            let neighbors = [
                                (1, 0),
                                (-1, 0),
                                (0, -1),
                                (0, 1),
                                (1, -1),
                                (-1, -1),
                                (1, 1),
                                (-1, 1),
                            ];
                            for &(dc, dr) in &neighbors {
                                let nc = c + dc;
                                let nr = r + dr;
                                if nc >= 0 && nc < map_w && nr >= 0 && nr < map_h {
                                    let n_idx = (nr * map_w + nc) as usize;
                                    if owners[n_idx] == owner && (terrain[n_idx] & 0x80) != 0 {
                                        shares_border = true;
                                        break 'outer;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if !shares_border {
                let messages = [
                    "Too far! 🌌",
                    "Out of reach! 🏃‍♂️",
                    "No border, no battle! ⚔️",
                    "Teleportation not researched! 📡",
                    "Build a path first! 🗺️",
                ];
                let click_seed = (x + y) as usize;
                let msg = messages[click_seed % messages.len()];

                let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
                let offset_mouse_y = y as f32 - 60.0;
                let world_y = (offset_mouse_y - self.input.camera_y) / self.input.camera_zoom;

                self.ui.floating_notices.push(crate::app::FloatingNotice {
                    text: msg.to_string(),
                    world_x,
                    world_y,
                    start_time: web_time::Instant::now(),
                    duration: web_time::Duration::from_millis(1500),
                    color: egui::Color32::from_rgb(248, 113, 113), // soft red
                });
                return;
            }

            let is_betrayer = self
                .sim
                .current_snapshot
                .as_ref()
                .and_then(|s| s.players.iter().find(|p| p.id == owner))
                .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
                .unwrap_or(false);
            let is_allied = self
                .sim
                .current_snapshot
                .as_ref()
                .and_then(|s| s.players.iter().find(|p| p.id == my_id))
                .map(|p| p.alliances.contains(&owner) && !is_betrayer)
                .unwrap_or(false);
            let is_teammate = self
                .sim
                .current_snapshot
                .as_ref()
                .map(|s| {
                    let my_team = s
                        .players
                        .iter()
                        .find(|p| p.id == my_id)
                        .and_then(|p| p.team);
                    let other_team = s
                        .players
                        .iter()
                        .find(|p| p.id == owner)
                        .and_then(|p| p.team);
                    my_team.is_some() && my_team == other_team
                })
                .unwrap_or(false);

            let troops = self.ui.app.hud_state.troops * (self.ui.app.hud_state.attack_ratio as f64);
            let attack = sow_core::protocol::AttackIntent {
                target_owner: owner,
                troops: Some(troops),
            };
            let intent = sow_core::protocol::GameplayIntent::Attack(attack);

            if is_allied || is_teammate {
                // Do not attack nor open menu on press; handled on release (click) instead
                return;
            } else {
                if !is_touch {
                    // Desktop: fire immediately
                    self.send_intent(intent);
                    self.input.hold_attack_target =
                        Some((owner, web_time::Instant::now(), x, y, true));
                } else {
                    // Mobile: wait for hold to distinguish from tap (context menu)
                    self.input.hold_attack_target =
                        Some((owner, web_time::Instant::now(), x, y, false));
                }
            }
            self.input.hold_attack_accum = 0.0;
        }
    }

    pub(crate) fn open_context_menu_at(&mut self, x: f64, y: f64) {
        if let Some((col, row)) = self.mouse_to_tile(x, y) {
            let idx = (row * self.sim.map_w as i32 + col) as u32;

            // Clear any prior menu state first to avoid animation caching issues
            self.input.map_context_menu = None;
            self.input.map_context_menu_active = None;
            self.input.context_menu_timer = 0.0;
            self.input.context_menu_open_time = Some(web_time::Instant::now());
            self.input.map_context_menu_session += 1;

            self.input.map_context_menu = Some((x as f32, y as f32, idx));
        }
    }

    pub(crate) fn handle_map_click(&mut self, x: f64, y: f64) {
        if self.ui.observing {
            self.show_observer_notice(x, y);
            return;
        }
        let phase = self
            .sim
            .current_snapshot
            .as_ref()
            .map(|s| &s.phase)
            .unwrap_or(&sow_core::game::GamePhase::Lobby);

        let (col, row) = match self.mouse_to_tile(x, y) {
            Some(res) => res,
            None => return,
        };

        if matches!(phase, sow_core::game::GamePhase::Spawning { .. }) {
            let idx = (row * self.sim.map_w as i32 + col) as usize;
            let terrain_byte = self
                .gfx
                .map_renderer
                .as_ref()
                .map(|mr| mr.terrain[idx])
                .unwrap_or(0);
            let is_land = (terrain_byte & 0x80) != 0;

            if !is_land {
                let wx = col as f32 + 0.5;
                let wy = row as f32 + 0.5;
                self.ui.click_markers.push(crate::app::ClickMarker {
                    world_x: wx,
                    world_y: wy,
                    start_time: web_time::Instant::now(),
                });

                let messages = [
                    "Splat! That's water! 🌊",
                    "Do you have gills? 🐠",
                    "Boats are for later! 🚢",
                    "Cannot build Atlantis yet! 🏛️",
                    "Water deployment failed! 💧",
                    "Too wet! ☔",
                    "Glug glug... ⚓",
                ];
                let click_seed = (x + y) as usize;
                let msg = messages[click_seed % messages.len()];

                let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
                let offset_mouse_y = y as f32 - 60.0;
                let world_y = (offset_mouse_y - self.input.camera_y) / self.input.camera_zoom;

                self.ui.floating_notices.push(crate::app::FloatingNotice {
                    text: msg.to_string(),
                    world_x,
                    world_y,
                    start_time: web_time::Instant::now(),
                    duration: web_time::Duration::from_millis(1500),
                    color: egui::Color32::from_rgb(96, 165, 250), // soft blue
                });
                return;
            }

            let owner = self
                .gfx
                .map_renderer
                .as_ref()
                .map(|mr| mr.owners[idx])
                .unwrap_or(0);

            let mut target_col = col;
            let mut target_row = row;

            if owner != 0 {
                let mut best_tile = None;
                let mut best_dist = i32::MAX;
                let search_radius = 5;

                for dy in -search_radius..=search_radius {
                    for dx in -search_radius..=search_radius {
                        let tx = col + dx;
                        let ty = row + dy;
                        if tx >= 0
                            && tx < self.sim.map_w as i32
                            && ty >= 0
                            && ty < self.sim.map_h as i32
                        {
                            let dist = sow_core::building::hex_distance(col, row, tx, ty);
                            if dist <= search_radius {
                                let n_idx = (ty * self.sim.map_w as i32 + tx) as usize;
                                let n_owner = self
                                    .gfx
                                    .map_renderer
                                    .as_ref()
                                    .map(|mr| mr.owners[n_idx])
                                    .unwrap_or(0);
                                let n_terrain = self
                                    .gfx
                                    .map_renderer
                                    .as_ref()
                                    .map(|mr| mr.terrain[n_idx])
                                    .unwrap_or(0);
                                let n_is_land = (n_terrain & 0x80) != 0;

                                if n_owner == 0 && n_is_land {
                                    if dist < best_dist {
                                        best_dist = dist;
                                        best_tile = Some((tx, ty));
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some((bx, by)) = best_tile {
                    target_col = bx;
                    target_row = by;
                } else {
                    let messages = [
                        "Hey! Too close to another player! 🛡️",
                        "Respect boundaries! 🤝",
                        "Get your own space! 🏕️",
                        "Social distancing! ↔️",
                        "Spawning blocked! 🛑",
                        "Private property! 🚫",
                    ];
                    let click_seed = (x + y) as usize;
                    let msg = messages[click_seed % messages.len()];

                    let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
                    let offset_mouse_y = y as f32 - 60.0;
                    let world_y = (offset_mouse_y - self.input.camera_y) / self.input.camera_zoom;

                    self.ui.floating_notices.push(crate::app::FloatingNotice {
                        text: msg.to_string(),
                        world_x,
                        world_y,
                        start_time: web_time::Instant::now(),
                        duration: web_time::Duration::from_millis(1500),
                        color: egui::Color32::from_rgb(248, 113, 113), // soft red
                    });

                    let wx = col as f32 + 0.5;
                    let wy = row as f32 + 0.5;
                    self.ui.click_markers.push(crate::app::ClickMarker {
                        world_x: wx,
                        world_y: wy,
                        start_time: web_time::Instant::now(),
                    });
                    return;
                }
            }

            let intent = sow_core::protocol::GameplayIntent::Spawn {
                x: target_col as u32,
                y: target_row as u32,
            };
            self.send_intent(intent);
        } else if let Some(nuke_kind) = self.ui.app.hud_state.selected_nuke_kind {
            let tile_idx = (row * self.sim.map_w as i32 + col) as u32;
            let intent = sow_core::protocol::GameplayIntent::LaunchNuke {
                kind: nuke_kind,
                target_tile: tile_idx,
            };
            self.send_intent(intent);
            self.ui.app.hud_state.selected_nuke_kind = None;
        } else if let Some(kind) = self.ui.app.hud_state.selected_building_kind {
            if let Some(snap) = &self.sim.current_snapshot {
                let my_id = self.sim.my_player_id.unwrap_or(0);
                let owners = self
                    .gfx
                    .map_renderer
                    .as_ref()
                    .map(|mr| mr.owners.as_slice())
                    .unwrap_or(&[]);
                let terrain = self
                    .gfx
                    .map_renderer
                    .as_ref()
                    .map(|mr| mr.terrain.as_slice())
                    .unwrap_or(&[]);

                let target_res = resolve_build_target_tile(&super::placement::PlacementQuery {
                    kind,
                    click_x: col,
                    click_y: row,
                    map_w: self.sim.map_w,
                    map_h: self.sim.map_h,
                    owners,
                    terrain,
                    my_id,
                    buildings: &snap.buildings,
                });

                let cost = {
                    let i = sow_core::game::BuildingKind::ALL
                        .iter()
                        .position(|&k| k == kind)
                        .unwrap_or(0);
                    self.ui.app.hud_state.building_costs[i]
                };

                let mut valid = true;
                let mut err_msg = String::new();

                if self.ui.app.hud_state.gold < cost {
                    valid = false;
                    let lang = self.ui.app.settings_state.language;
                    err_msg = sow_i18n::get(lang)
                        .hud
                        .err_need_gold
                        .replace("{}", &sow_ui_kit::utils::format_number(cost));
                } else {
                    match target_res {
                        Ok(_) => {}
                        Err(msg) => {
                            valid = false;
                            err_msg = msg.to_string();
                        }
                    }
                }

                if !valid {
                    let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
                    let offset_mouse_y = y as f32 - 60.0;
                    let world_y = (offset_mouse_y - self.input.camera_y) / self.input.camera_zoom;
                    self.ui.floating_notices.push(crate::app::FloatingNotice {
                        text: err_msg,
                        world_x,
                        world_y,
                        start_time: web_time::Instant::now(),
                        duration: web_time::Duration::from_millis(2000),
                        color: egui::Color32::from_rgb(248, 113, 113),
                    });
                } else {
                    let target_tile = target_res.unwrap();
                    let intent =
                        sow_core::protocol::GameplayIntent::BuildStructure { kind, target_tile };
                    self.send_intent(intent);
                    self.ui.last_build_confirm_time = Some(web_time::Instant::now());
                }
            }
        } else {
            // Check if we clicked on a Warship we own
            let mut clicked_warships = Vec::new();
            if let Some(snap) = &self.sim.current_snapshot {
                let my_pid = self.sim.my_player_id.unwrap_or(0);
                let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
                let world_y = (y as f32 - self.input.camera_y) / self.input.camera_zoom;
                for f in &snap.fleets {
                    if f.unit_type == sow_core::game::UnitType::Warship && f.owner_id == my_pid {
                        let col = (f.current_tile % self.sim.map_w) as f32;
                        let row = (f.current_tile / self.sim.map_w) as f32;
                        let wx = col + 0.5;
                        let wy = row + 0.5;
                        // Click tolerance (half a tile)
                        if (wx - world_x).abs() < 0.5 && (wy - world_y).abs() < 0.5 {
                            clicked_warships.push(f.id);
                        }
                    }
                }
            }
            if !clicked_warships.is_empty() {
                self.input.selected_warships = clicked_warships;
            } else {
                self.input.selected_warships.clear();

                // If not selecting warships, check if we clicked on allied territory to open context menu on release
                let idx = (row * self.sim.map_w as i32 + col) as usize;
                let owner = self
                    .gfx
                    .map_renderer
                    .as_ref()
                    .map(|mr| mr.owners[idx])
                    .unwrap_or(0);
                let my_id = self.sim.my_player_id.unwrap_or(0);
                let is_betrayer = self
                    .sim
                    .current_snapshot
                    .as_ref()
                    .and_then(|s| s.players.iter().find(|p| p.id == owner))
                    .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
                    .unwrap_or(false);
                let is_allied = self
                    .sim
                    .current_snapshot
                    .as_ref()
                    .and_then(|s| s.players.iter().find(|p| p.id == my_id))
                    .map(|p| p.alliances.contains(&owner) && !is_betrayer)
                    .unwrap_or(false);
                let is_teammate = self
                    .sim
                    .current_snapshot
                    .as_ref()
                    .map(|s| {
                        let my_team = s
                            .players
                            .iter()
                            .find(|p| p.id == my_id)
                            .and_then(|p| p.team);
                        let other_team = s
                            .players
                            .iter()
                            .find(|p| p.id == owner)
                            .and_then(|p| p.team);
                        my_team.is_some() && my_team == other_team
                    })
                    .unwrap_or(false);

                if owner != 0 && owner != my_id && (is_allied || is_teammate) {
                    self.open_context_menu_at(x, y);
                }
            }
        }
    }
}
