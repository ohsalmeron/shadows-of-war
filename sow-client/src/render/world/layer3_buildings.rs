use super::*;

use crate::render::world::utils::*;

#[allow(unused_variables)]
pub(crate) fn render(ui: &mut crate::app::UiState, sim: &crate::app::SimState, input: &crate::app::InputState, time: &crate::app::TimeState, gfx: &crate::app::GraphicsState, ctx: &RenderContext) {
    let painter = ctx.painter;
    let sf = ctx.sf;
    let zoom_scaled = ctx.zoom_scaled;

    if let Some(snap) = &sim.current_snapshot {
        // S2: Restore zoom LOD gate — at zoom < 0.25, buildings are sub-pixel, skip entirely
        if zoom_scaled < 0.25 {
            return;
        }

        struct RenderedBuilding {
            bx: f32,
            by: f32,
            kind: sow_core::game::BuildingKind,
            level: u32,
            under_construction: bool,
            count: usize,
        }

        let cell_size = if zoom_scaled < 0.6 {
            128.0 // LOD 3: Major sector-level grouping
        } else if zoom_scaled < 1.2 {
            64.0  // LOD 2: Intermediate grid grouping
        } else if zoom_scaled < 2.5 {
            24.0  // LOD 1: Close clustering
        } else {
            1.0   // No clustering
        };

        let mut rendered_buildings = Vec::new();

        if cell_size > 1.0 {
            #[derive(Hash, PartialEq, Eq)]
            struct ClusterKey {
                grid_x: i32,
                grid_y: i32,
                owner_id: u16,
                kind: Option<sow_core::game::BuildingKind>,
                level: Option<u8>,
            }
            let mut clusters: std::collections::HashMap<ClusterKey, (f32, f32, usize, u32, Option<sow_core::game::BuildingKind>)> =
                std::collections::HashMap::new();

            for b in &snap.buildings {
                let tile_x = (b.tile_idx % sim.map_w) as f32;
                let tile_y = (b.tile_idx / sim.map_w) as f32;
                let bx = tile_x + 0.5 + (tile_y as i32 % 2) as f32 * 0.5;
                let by = (tile_y + 0.5) * 0.8660254_f32;

                let grid_x = (tile_x / cell_size) as i32;
                let grid_y = (tile_y / cell_size) as i32;

                let (kind_key, level_key) = if zoom_scaled < 0.6 {
                    (None, None)
                } else if zoom_scaled < 1.2 {
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

                let b_level = if b.under_construction { 1 } else { b.level as u32 };

                let entry = clusters.entry(key).or_insert((0.0, 0.0, 0, 0, Some(b.kind)));
                entry.0 += bx;
                entry.1 += by;
                entry.2 += 1;
                entry.3 += b_level;
            }

            for (key, (sum_bx, sum_by, count, sum_level, cluster_kind)) in clusters {
                let final_kind = key.kind.or(cluster_kind).unwrap_or(sow_core::game::BuildingKind::City);
                rendered_buildings.push(RenderedBuilding {
                    bx: sum_bx / count as f32,
                    by: sum_by / count as f32,
                    kind: final_kind,
                    level: sum_level,
                    under_construction: false,
                    count,
                });
            }
        } else {
            for b in &snap.buildings {
                let tile_x = (b.tile_idx % sim.map_w) as f32;
                let tile_y = (b.tile_idx / sim.map_w) as f32;
                let bx = tile_x + 0.5 + (tile_y as i32 % 2) as f32 * 0.5;
                let by = (tile_y + 0.5) * 0.8660254_f32;
                rendered_buildings.push(RenderedBuilding {
                    bx,
                    by,
                    kind: b.kind,
                    level: b.level as u32,
                    under_construction: b.under_construction,
                    count: 1,
                });
            }
        }

        // Depth sort bottom-to-top (and left-to-right) to make overlaps completely stable and prevent flickering
        rendered_buildings.sort_by(|a, b| {
            a.by.partial_cmp(&b.by)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.bx.partial_cmp(&b.bx)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.count.cmp(&b.count))
        });

        for b in rendered_buildings {
            let screen_x = (input.camera_x + b.bx * input.camera_zoom) / sf;
            let screen_y = (input.camera_y + b.by * input.camera_zoom) / sf;

            // Frustum cull
            let margin = zoom_scaled * 2.0;
            if screen_x < -margin
                || screen_x > input.screen_w / sf + margin
                || screen_y < -margin
                || screen_y > input.screen_h / sf + margin
            {
                continue;
            }

            let center = egui::pos2(screen_x, screen_y);

            let uri = b.kind.asset().uri();

            let base_size = if b.count > 1 {
                28.0_f32.max(get_building_icon_size(zoom_scaled) * 1.2)
            } else {
                get_building_icon_size(zoom_scaled)
            };
            let rect = egui::Rect::from_center_size(center, egui::vec2(base_size, base_size));

            let size_hint = egui::load::SizeHint::Size {
                width: 64,
                height: 64,
                maintain_aspect_ratio: true,
            };

            let load_res = painter.ctx().try_load_texture(
                uri,
                egui::TextureOptions::LINEAR,
                size_hint,
            );

            if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                let tint = if b.under_construction {
                    if b.kind.asset().is_svg() {
                        egui::Color32::from_black_alpha(128)
                    } else {
                        egui::Color32::from_white_alpha(128)
                    }
                } else if b.kind.asset().is_svg() {
                    egui::Color32::BLACK
                } else {
                    egui::Color32::WHITE
                };
                painter.image(
                    texture.id,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    tint,
                );
            }

            // Level badge (no white plate background, no frame, larger text in black)
            let text_val = if b.count > 1 {
                b.level.to_string()
            } else {
                get_level_str(b.level as u8).to_string()
            };

            if text_val != "1" {
                let font_size = (zoom_scaled * 0.65).clamp(11.0, 18.0).round();
                let bg_center = egui::pos2(center.x + base_size * 0.45, center.y - base_size * 0.45);

                painter.text(
                    bg_center,
                    egui::Align2::CENTER_CENTER,
                    text_val,
                    egui::FontId::proportional(font_size),
                    egui::Color32::BLACK,
                );
            }
        }
    }
}
