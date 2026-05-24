use super::*;

use crate::render::world::utils::*;

#[allow(unused_variables)]
pub(crate) fn render(ui: &mut crate::app::UiState, sim: &crate::app::SimState, input: &crate::app::InputState, time: &crate::app::TimeState, gfx: &crate::app::GraphicsState, ctx: &RenderContext) {
    let painter = ctx.painter.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("world_buildings"),
    ));
    let sf = ctx.sf;
    let zoom_scaled = ctx.zoom_scaled;
    let player_colors = ctx.player_colors;
    let wall_secs = ctx.wall_secs;
    let building_scale = ctx.painter.ctx().data(|d| {
        d.get_temp::<f32>(egui::Id::new("dev_building_scale")).unwrap_or(1.0)
    });

    if let Some(snap) = &sim.current_snapshot {
        // S2: Restore zoom LOD gate — at zoom < 0.25, buildings are sub-pixel, skip entirely
        if zoom_scaled < 0.25 {
            return;
        }

        // Collect silo tiles that have an active projectile in-flight
        let mut launching_silo_tiles: Vec<u32> = Vec::new();
        for proj in &snap.projectiles {
            if matches!(proj.kind, sow_core::game::ProjectileKind::Nuke { .. }) {
                launching_silo_tiles.push(proj.src_tile);
            }
        }

        struct RenderedBuilding {
            bx: f32,
            by: f32,
            kind: sow_core::game::BuildingKind,
            level: u32,
            under_construction: bool,
            ticks_until_complete: u32,
            count: usize,
            owner_id: u16,
            tile_idx: u32,
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
                    ticks_until_complete: 0,
                    count,
                    owner_id: key.owner_id,
                    tile_idx: 0,
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
                    ticks_until_complete: b.ticks_until_complete,
                    count: 1,
                    owner_id: b.owner_id,
                    tile_idx: b.tile_idx,
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
            } * building_scale;
            let rect = egui::Rect::from_center_size(center, egui::vec2(base_size, base_size));

            // --- Cybernetic Base Plate & Glow effects (drawn behind the sprite) ---
            if b.owner_id != 0 {
                let pc = player_colors.get(b.owner_id as usize).copied().unwrap_or(egui::Color32::GRAY);
                let is_launching_silo = launching_silo_tiles.contains(&b.tile_idx);

                // 1. Sleek drop shadow (dark semi-transparent offset circle to give depth)
                let shadow_offset = base_size * 0.08;
                let shadow_center = center + egui::vec2(shadow_offset, shadow_offset);
                painter.circle_filled(shadow_center, base_size * 0.46, egui::Color32::from_black_alpha(150));

                // 2. High-contrast Dark cyber-plate foundation disc (blocks out underlying terrain color)
                painter.circle_filled(center, base_size * 0.46, egui::Color32::from_rgba_unmultiplied(15, 15, 20, 240));

                // 3. Sharp, high-contrast Player colored neon ring outline
                if is_launching_silo {
                    // Double ring / pulsing launching silo effect
                    let ring_pulse = (wall_secs * 15.0).sin() as f32 * 0.5 + 0.5;
                    let ext_r = base_size * (0.46 + ring_pulse * 0.15);
                    let ext_a = (180.0 * (1.0 - ring_pulse)) as u8;
                    // Fading expanding ring
                    painter.circle_stroke(center, ext_r, egui::Stroke::new(2.0_f32, egui::Color32::from_rgba_unmultiplied(pc.r(), pc.g(), pc.b(), ext_a)));
                    // Solid inner neon ring
                    painter.circle_stroke(center, base_size * 0.46, egui::Stroke::new(3.0_f32, pc));
                } else if b.under_construction {
                    // Dotted/pulsing construction ring
                    let pulse = (wall_secs * 5.0).sin() as f32 * 0.5 + 0.5;
                    let stroke_w = 1.5_f32 + pulse * 2.0_f32;
                    let construction_color = egui::Color32::from_rgba_unmultiplied(pc.r(), pc.g(), pc.b(), (100.0 + pulse * 155.0) as u8);
                    painter.circle_stroke(center, base_size * 0.46, egui::Stroke::new(stroke_w, construction_color));
                } else {
                    // High-contrast, sharp, beautiful neon outline
                    painter.circle_stroke(center, base_size * 0.46, egui::Stroke::new(2.5_f32, pc));
                }
            }


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
                    } else if b.owner_id != 0 {
                        let player_color = player_colors.get(b.owner_id as usize).copied().unwrap_or(egui::Color32::WHITE);
                        let r = ((player_color.r() as f32 * 0.30) + (255.0 * 0.70)) as u8;
                        let g = ((player_color.g() as f32 * 0.30) + (255.0 * 0.70)) as u8;
                        let b_val = ((player_color.b() as f32 * 0.30) + (255.0 * 0.70)) as u8;
                        egui::Color32::from_rgba_unmultiplied(r, g, b_val, 128)
                    } else {
                        egui::Color32::from_white_alpha(128)
                    }
                } else if b.kind.asset().is_svg() {
                    egui::Color32::BLACK
                } else if b.owner_id != 0 {
                    let player_color = player_colors.get(b.owner_id as usize).copied().unwrap_or(egui::Color32::WHITE);
                    let r = ((player_color.r() as f32 * 0.30) + (255.0 * 0.70)) as u8;
                    let g = ((player_color.g() as f32 * 0.30) + (255.0 * 0.70)) as u8;
                    let b_val = ((player_color.b() as f32 * 0.30) + (255.0 * 0.70)) as u8;
                    egui::Color32::from_rgba_unmultiplied(r, g, b_val, 255)
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
            
            if b.under_construction && b.ticks_until_complete > 0 {
                let total_ticks = b.kind.construction_duration_ticks();
                if total_ticks > 0 {
                    let progress = 1.0 - (b.ticks_until_complete as f32 / total_ticks as f32).clamp(0.0, 1.0);
                    
                    // Design: Sleek glassmorphic bar just below the building
                    let bar_w = base_size * 0.95;
                    let bar_h = (4.0 * zoom_scaled).clamp(3.0, 5.0);
                    let bar_y = center.y + base_size * 0.55;
                    let bar_rect = egui::Rect::from_center_size(
                        egui::pos2(center.x, bar_y),
                        egui::vec2(bar_w, bar_h)
                    );
                    
                    // Glass background pill with border
                    let bg_color = egui::Color32::from_black_alpha(160);
                    let border_color = egui::Color32::from_white_alpha(40);
                    painter.rect(
                        bar_rect,
                        2.0,
                        bg_color,
                        egui::Stroke::new(1.0_f32, border_color),
                        egui::StrokeKind::Inside,
                    );
                    
                    // Glowing cyan progress fill
                    if progress > 0.0 {
                        let fill_w = bar_w * progress;
                        let fill_rect = egui::Rect::from_min_max(
                            egui::pos2(bar_rect.min.x, bar_rect.min.y),
                            egui::pos2(bar_rect.min.x + fill_w, bar_rect.max.y)
                        );
                        let fill_color = egui::Color32::from_rgb(0, 220, 255);
                        painter.rect(
                            fill_rect,
                            2.0,
                            fill_color,
                            egui::Stroke::NONE,
                            egui::StrokeKind::Inside,
                        );
                    }
                    
                    // Small, premium, highly legible micro-font tick label below the bar
                    let font_size = (8.5 * zoom_scaled).clamp(8.0, 12.0).round();
                    let text_y = bar_y + bar_h * 0.5 + font_size * 0.5 + 1.0;
                    let label = format!("{} / {} t", total_ticks.saturating_sub(b.ticks_until_complete), total_ticks);
                    
                    // Subtle drop shadow for readability
                    painter.text(
                        egui::pos2(center.x + 1.0, text_y + 1.0),
                        egui::Align2::CENTER_CENTER,
                        &label,
                        egui::FontId::proportional(font_size),
                        egui::Color32::from_black_alpha(180),
                    );
                    
                    // Main crisp text
                    painter.text(
                        egui::pos2(center.x, text_y),
                        egui::Align2::CENTER_CENTER,
                        &label,
                        egui::FontId::proportional(font_size),
                        egui::Color32::WHITE,
                    );
                }
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
