use crate::render::world::movers::tile_to_world;
use sow_core::game::BuildingKind;

pub(super) struct RenderedBuilding {
    pub bx: f32,
    pub by: f32,
    pub kind: BuildingKind,
    pub active_level: u8,
    pub target_level: u8,
    pub under_construction: bool,
    pub ticks_until_complete: u32,
    pub count: usize,
    pub owner_id: u16,
    pub id: Option<u64>,
    pub modules: Option<sow_core::building::CityModules>,
    pub tile_idx: Option<u32>,
}

pub(super) fn collect_rendered_buildings(
    snap: &sow_core::protocol::SimSnapshot,
    map_w: u32,
    zoom_scaled: f32,
    far_zoom_threshold: f32,
) -> Vec<RenderedBuilding> {
    let cell_size = if zoom_scaled < 1.5 {
        128.0 // LOD 3: Aggressive far-zoom grouping
    } else if zoom_scaled < 2.5 {
        64.0 // LOD 2: Intermediate grid grouping
    } else if zoom_scaled < far_zoom_threshold {
        24.0 // LOD 1: Close clustering
    } else {
        1.0 // No clustering
    };

    let building_count = snap.buildings.len();
    let mut rendered_buildings = Vec::with_capacity(building_count);

    if cell_size > 1.0 {
        #[derive(Hash, PartialEq, Eq)]
        struct ClusterKey {
            grid_x: i32,
            grid_y: i32,
            owner_id: u16,
            kind: Option<sow_core::game::BuildingKind>,
            level: Option<u8>,
        }
        let mut clusters: std::collections::HashMap<
            ClusterKey,
            (f32, f32, usize, u32, Option<sow_core::game::BuildingKind>),
        > = std::collections::HashMap::with_capacity(building_count / 4);

        for b in &snap.buildings {
            let (bx, by) = tile_to_world(b.tile_idx, map_w);
            let tile_x = (b.tile_idx % map_w) as f32;
            let tile_y = (b.tile_idx / map_w) as f32;

            let grid_x = (tile_x / cell_size) as i32;
            let grid_y = (tile_y / cell_size) as i32;

            let (kind_key, level_key) = if zoom_scaled < 2.5 {
                (Some(b.kind), None)
            } else {
                (Some(b.kind), Some(b.level))
            };

            let key = ClusterKey {
                grid_x,
                grid_y,
                owner_id: b.owner_id,
                kind: kind_key,
                level: level_key,
            };

            let b_level = if b.under_construction {
                b.active_level() as u32
            } else {
                b.level as u32
            };

            let entry = clusters
                .entry(key)
                .or_insert((0.0, 0.0, 0, 0, Some(b.kind)));
            entry.0 += bx;
            entry.1 += by;
            entry.2 += 1;
            entry.3 += b_level;
        }

        for (key, (sum_bx, sum_by, count, sum_level, cluster_kind)) in clusters {
            let final_kind = key
                .kind
                .or(cluster_kind)
                .unwrap_or(sow_core::game::BuildingKind::City);
            let avg_level = (sum_level / count as u32) as u8;
            rendered_buildings.push(RenderedBuilding {
                bx: sum_bx / count as f32,
                by: sum_by / count as f32,
                kind: final_kind,
                active_level: avg_level,
                target_level: avg_level,
                under_construction: false,
                ticks_until_complete: 0,
                count,
                owner_id: key.owner_id,
                id: None,
                modules: None,
                tile_idx: None,
            });
        }
    } else {
        for b in &snap.buildings {
            let (bx, by) = tile_to_world(b.tile_idx, map_w);
            rendered_buildings.push(RenderedBuilding {
                bx,
                by,
                kind: b.kind,
                active_level: b.active_level(),
                target_level: b.level,
                under_construction: b.under_construction,
                ticks_until_complete: b.ticks_until_complete,
                count: 1,
                owner_id: b.owner_id,
                id: Some(b.id),
                modules: Some(b.modules),
                tile_idx: Some(b.tile_idx),
            });
        }
    }

    // Depth sort bottom-to-top (and left-to-right) to make overlaps completely stable and prevent flickering

    rendered_buildings
}
