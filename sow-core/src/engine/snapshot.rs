use crate::engine::SowEngine;

impl SowEngine {
    pub fn build_snapshot(&mut self) -> crate::protocol::SimSnapshot {
        let dirty_tiles: Vec<crate::protocol::DirtyTile> = self
            .state
            .map
            .dirty_tiles
            .drain(..)
            .map(|i| crate::protocol::DirtyTile {
                index: i as u32,
                new_owner: self.state.map.state[i],
                upgrade_level: self.state.map.tile_upgrades[i],
            })
            .collect();

        let proposed = &self.alliances_proposed;
        let proposed_resources = &self.resource_requests_proposed;
        let players = self
            .state
            .players
            .iter()
            .map(|p| {
                let (cx, cy) = if p.tile_count > 0 {
                    (
                        (p.sum_x / p.tile_count as u64) as f32,
                        (p.sum_y / p.tile_count as u64) as f32,
                    )
                } else {
                    (0.0, 0.0)
                };

                let name = p.name.clone();

                let alliance_requests = proposed
                    .iter()
                    .filter(|prop| prop.target == p.id)
                    .map(|prop| prop.proposer)
                    .collect();

                let resource_requests = proposed_resources
                    .iter()
                    .filter(|r| r.target == p.id)
                    .map(|r| crate::protocol::ResourceRequest {
                        requester: r.proposer,
                        gold: r.gold,
                        troops: r.troops,
                    })
                    .collect();

                crate::protocol::PlayerSnapshot {
                    id: p.id,
                    name,
                    troops: p.troops,
                    max_troops: p.max_troops,
                    gold: p.gold,
                    tile_count: p.tile_count,
                    centroid_x: cx,
                    centroid_y: cy,
                    player_type: p.player_type,
                    color: p.color,
                    team: p.team,
                    has_spawned: p.has_spawned,
                    alive: p.alive,
                    iq: p.iq,
                    alliances: p.alliances.clone(),
                    alliance_timers: p.alliance_timers.clone(),
                    alliance_requests,
                    resource_requests,
                    disconnected: p.disconnected,
                    active_emoji: p.active_emoji.clone(),
                    traitor: p.traitor,
                    civilization: p.civilization,
                    leader: p.leader,
                    kills: p.kills,
                    deaths: p.deaths,
                    assists: p.assists,
                }
            })
            .collect();

        let fleets = self
            .fleets
            .iter()
            .map(|f| crate::protocol::FleetSnapshot {
                id: f.id,
                owner_id: f.owner_id,
                unit_type: f.unit_type,
                troops: f.troops,
                current_tile: f.current_tile,
                path: f.path.clone(),
                path_cursor: f.path_cursor,
                retreating: f.retreating,
            })
            .collect();

        let attacks = self
            .attacks
            .iter()
            .map(|a| {
                let (fcx, fcy) = if a.target_owner != 0 {
                    a.frontier_centroid()
                } else {
                    (0.0, 0.0)
                };
                crate::protocol::AttackSnapshot {
                    id: a.id,
                    owner_id: a.owner_id,
                    target_owner: a.target_owner,
                    troops: a.troops,
                    retreating: a.retreating,
                    front_cx: fcx,
                    front_cy: fcy,
                }
            })
            .collect();

        let mut defense_posts = Vec::new();
        if self.render_defense_dirty {
            for b in &self.buildings {
                if b.kind == crate::game::BuildingKind::Bunker && !b.under_construction {
                    defense_posts.push(b.tile_idx);
                }
            }
        }
        let defense_dirty = self.render_defense_dirty;
        self.render_defense_dirty = false;

        let spawn_timer_secs =
            if let crate::game::GamePhase::Spawning { end_tick } = &self.state.phase {
                Some(
                    end_tick.saturating_sub(self.state.tick) as f32
                        * (self.state.config.tick_rate_ms / 1000.0),
                )
            } else {
                None
            };

        let buildings: Vec<crate::protocol::BuildingSnapshot> = self
            .buildings
            .iter()
            .map(|b| crate::protocol::BuildingSnapshot {
                id: b.id,
                tile_idx: b.tile_idx,
                owner_id: b.owner_id,
                kind: b.kind,
                level: b.level,
                under_construction: b.under_construction,
                ticks_until_complete: b.ticks_until_complete,
                modules: b.modules,
            })
            .collect();

        crate::protocol::SimSnapshot {
            tick: self.state.tick,
            phase: self.state.phase.clone(),
            spawn_timer_secs,
            players,
            dirty_tiles,
            fleets,
            attacks,
            buildings,
            projectiles: self
                .projectiles
                .iter()
                .filter(|p| p.active)
                .map(|p| crate::protocol::ProjectileSnapshot {
                    id: p.id,
                    kind: p.kind,
                    owner_id: p.owner_id,
                    src_tile: p.src_tile,
                    dst_tile: p.dst_tile,
                    path: p.path.clone(),
                    path_cursor: p.path_cursor,
                    steps_per_tick: p.steps_per_tick,
                })
                .collect(),
            nuke_alerts: self
                .state
                .events
                .iter()
                .filter_map(|e| {
                    if let crate::game::GameEvent::NukeDetonated {
                        tile_x,
                        tile_y,
                        owner_id,
                        inner_radius: _,
                        outer_radius: _,
                    } = e
                    {
                        let kind = crate::game::NukeKind::AtomBomb;
                        Some(crate::protocol::NukeAlert {
                            owner_id: *owner_id,
                            kind,
                            tile_x: *tile_x,
                            tile_y: *tile_y,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            resource_transfers: self
                .state
                .events
                .iter()
                .filter_map(|e| {
                    if let crate::game::GameEvent::ResourceTransferred {
                        sender_id,
                        receiver_id,
                        gold,
                        troops,
                    } = e
                    {
                        Some(crate::protocol::ResourceTransfer {
                            sender_id: *sender_id,
                            receiver_id: *receiver_id,
                            gold: *gold,
                            troops: *troops,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            resource_rejections: self
                .state
                .events
                .iter()
                .filter_map(|e| {
                    if let crate::game::GameEvent::ResourceRequestRejected {
                        rejector_id,
                        requester_id,
                    } = e
                    {
                        Some(crate::protocol::ResourceRejection {
                            rejector_id: *rejector_id,
                            requester_id: *requester_id,
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            winner: self.state.winner,
            winning_team: self.state.winning_team,
            total_land_tiles: self.state.total_land_tiles,
            defense_posts,
            defense_dirty,
            sea_lanes: self.state.sea_lanes.clone(),
            debug_mem_info: if cfg!(feature = "mem_profiler") {
                format!(
                    "Engine [Attacks: {}/{} | Fleets: {}/{} | Buildings: {}/{} | Events: {}/{} | Players: {}/{} | DirtyTilesCap: {}] Pathfinder [AStarHeapCap: {} | AStarCameCap: {} | BFSQueueCap: {} | BFSVisitedCap: {}] Placement [VisitedCap: {} | QueueCap: {} | BorderCap: {}]",
                    self.attacks.len(), self.attacks.capacity(),
                    self.fleets.len(), self.fleets.capacity(),
                    self.buildings.len(), self.buildings.capacity(),
                    self.state.events.len(), self.state.events.capacity(),
                    self.state.players.len(), self.state.players.capacity(),
                    self.state.map.dirty_tiles.capacity(),
                    self.path_scratch.astar.heap.capacity(),
                    self.path_scratch.astar.came_from.capacity(),
                    self.path_scratch.bfs_queue.capacity(),
                    self.path_scratch.bfs_visited.capacity(),
                    self.placement_scratch.visited_stamp.len(),
                    self.placement_scratch.queue.capacity(),
                    self.placement_scratch.border_scratch.capacity(),
                )
            } else {
                String::new()
            },
        }
    }
}
