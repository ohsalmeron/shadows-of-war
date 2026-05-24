use crate::engine::SowEngine;
use crate::game::{BuildingKind, GameEvent, GamePhase};
/// Advance construction timers; emits [`GameEvent::StructureReady`].
/// Split out for unit tests (`advance_building_construction_tick`).
impl SowEngine {
    pub fn execute_construction(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        // buildings are maintained sorted by id on insertion.

        for b in &mut self.buildings {
            if !b.under_construction {
                continue;
            }
            if b.ticks_until_complete > 0 {
                b.ticks_until_complete -= 1;
            }
            if b.ticks_until_complete == 0 {
                b.under_construction = false;
                self.building_aggregates_dirty = true;
                if b.kind == BuildingKind::City {
                    self.railroads_dirty = true;
                    self.sea_lanes_dirty = true;
                }
                if b.kind == BuildingKind::Bunker {
                    self.defense_grid_dirty = true;
                }
                self.state.events.push(GameEvent::StructureReady {
                    id: b.id,
                    tile_idx: b.tile_idx,
                    kind: b.kind,
                });
            }
        }
    }

    pub fn execute_ship_production(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        let mut new_fleets = Vec::new();

        for (port_id, queue) in self.port_queues.iter_mut() {
            if let Some(prod) = queue.front_mut() {
                if prod.ticks_until_complete > 0 {
                    prod.ticks_until_complete -= 1;
                }
                if prod.ticks_until_complete == 0 {
                    let prod = queue.pop_front().unwrap();
                    if let Some(port) = self.buildings.iter().find(|b| b.id == *port_id) {
                        let owner_id = port.owner_id;
                        let src_tile = port.tile_idx;
                        let map = &self.state.map;
                        let x = src_tile % map.width;
                        let y = src_tile / map.width;
                        let mut spawn_tile = src_tile;
                        let neighbors = [
                            (x.wrapping_sub(1), y),
                            (x + 1, y),
                            (x, y.wrapping_sub(1)),
                            (x, y + 1),
                        ];
                        for (nx, ny) in neighbors {
                            if map.is_valid_coord(nx as i32, ny as i32) {
                                let idx = ny * map.width + nx;
                                if map.terrain[idx as usize].is_water() {
                                    spawn_tile = idx;
                                    break;
                                }
                            }
                        }

                        let fid = self.state.next_fleet_id;
                        self.state.next_fleet_id = self.state.next_fleet_id.wrapping_add(1).max(1);
                        new_fleets.push(crate::warp_fleet::WarpFleet::new(
                            fid,
                            owner_id,
                            0,
                            prod.kind,
                            prod.kind.max_health(), // Treat troops as health for ships
                            spawn_tile,
                            spawn_tile,
                            vec![],
                        ));
                    }
                }
            }
        }

        for f in new_fleets {
            self.add_fleet(f);
        }
    }
}
