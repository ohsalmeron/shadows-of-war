use super::*;

use crate::render::world::utils::*;
#[allow(unused_variables)]
pub(crate) fn render(ui: &mut crate::app::UiState, sim: &crate::app::SimState, input: &crate::app::InputState, time: &crate::app::TimeState, gfx: &crate::app::GraphicsState, ctx: &RenderContext) {
    let painter = ctx.painter;
    let sf = ctx.sf;
    let zoom_scaled = ctx.zoom_scaled;
    let player_colors = ctx.player_colors;

    if let Some(snap) = &sim.current_snapshot {
            // S2: Restore zoom LOD gate — at zoom < 0.25, buildings are sub-pixel, skip entirely
            if zoom_scaled < 0.25 {
                return;
            }

            // S6: Pre-build a set of tiles with active upgrade anims (avoids O(N*M) linear scan)
            let upgrade_tiles: std::collections::HashSet<u32> = ui.active_upgrades.iter().map(|a| a.tile_idx).collect();

            struct RenderedBuilding {
                bx: f32,
                by: f32,
                kind: sow_core::game::BuildingKind,
                level: u8,
                under_construction: bool,
                count: usize,
                has_active_upgrade: bool,
            }

            let cell_size = if zoom_scaled < 1.2 {
                32.0
            } else if zoom_scaled < 2.5 {
                16.0
            } else if zoom_scaled < 5.0 {
                8.0
            } else {
                1.0
            };

            let mut rendered_buildings = Vec::new();

            if cell_size > 1.0 {
                #[derive(Hash, PartialEq, Eq)]
                struct ClusterKey {
                    grid_x: i32,
                    grid_y: i32,
                    owner_id: u16,
                    kind: sow_core::game::BuildingKind,
                    level: u8,
                    under_construction: bool,
                }
                let mut clusters: std::collections::HashMap<ClusterKey, (f32, f32, usize, bool)> =
                    std::collections::HashMap::new();

                for b in &snap.buildings {
                    let bx = (b.tile_idx % sim.map_w) as f32;
                    let by = (b.tile_idx / sim.map_w) as f32;
                    let active_upgrade = upgrade_tiles.contains(&b.tile_idx);

                    let grid_x = (bx / cell_size) as i32;
                    let grid_y = (by / cell_size) as i32;
                    let key = ClusterKey {
                        grid_x,
                        grid_y,
                        owner_id: b.owner_id,
                        kind: b.kind,
                        level: b.level,
                        under_construction: b.under_construction,
                    };
                    let entry = clusters.entry(key).or_insert((0.0, 0.0, 0, false));
                    entry.0 += bx;
                    entry.1 += by;
                    entry.2 += 1;
                    if active_upgrade {
                        entry.3 = true;
                    }
                }

                for (key, (sum_bx, sum_by, count, has_active_upgrade)) in clusters {
                    rendered_buildings.push(RenderedBuilding {
                        bx: sum_bx / count as f32,
                        by: sum_by / count as f32,
                        kind: key.kind,
                        level: key.level,
                        under_construction: key.under_construction,
                        count,
                        has_active_upgrade,
                    });
                }
            } else {
                for b in &snap.buildings {
                    let bx = (b.tile_idx % sim.map_w) as f32;
                    let by = (b.tile_idx / sim.map_w) as f32;
                    let active_upgrade = upgrade_tiles.contains(&b.tile_idx);
                    rendered_buildings.push(RenderedBuilding {
                        bx,
                        by,
                        kind: b.kind,
                        level: b.level,
                        under_construction: b.under_construction,
                        count: 1,
                        has_active_upgrade: active_upgrade,
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
                let screen_x = (input.camera_x + (b.bx + 0.5) * input.camera_zoom) / sf;
                let screen_y = (input.camera_y + (b.by + 0.5) * input.camera_zoom) / sf;

                // Frustum cull — reduced margin back from .max(32) to zoom_scaled * 2.0
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

                // Scale up clustered group icons so they stand out and are clearly visible behind the badge
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

                // Level badge
                let is_constructing = b.under_construction;
                let text_val = if is_constructing {
                    "🔨".to_string()
                } else if b.count > 1 {
                    format!("{}x{}", get_level_str(b.level), b.count)
                } else {
                    get_level_str(b.level).to_string()
                };

                let font_size = (zoom_scaled * 0.4).clamp(8.0, 12.0).round();

                // Determine badge geometry: pill-shape for grouped multiplier texts, circle for single badges
                let text_len = text_val.chars().count();
                let is_grouped = b.count > 1;
                let (badge_w, badge_h, bg_radius) = if is_grouped {
                    let w = (text_len as f32 * font_size * 0.55 + 6.0).round();
                    let h = (font_size * 1.3 + 4.0).round();
                    (w, h, h * 0.5)
                } else {
                    let r = (font_size * 0.85).round();
                    (r * 2.0, r * 2.0, r)
                };

                let bg_center = egui::pos2(center.x + base_size * 0.4, center.y - base_size * 0.4);

                let mut pulse_scale = 1.0_f32;

                // S9: Fix float literal suffixes
                let (bg_color, stroke_color, stroke_width) = if b.has_active_upgrade {
                    let anim = ui.active_upgrades.iter().find(|anim| {
                        let abx = (anim.tile_idx % sim.map_w) as f32;
                        let aby = (anim.tile_idx / sim.map_w) as f32;
                        if cell_size > 1.0 {
                            (abx / cell_size) as i32 == (b.bx / cell_size) as i32
                                && (aby / cell_size) as i32 == (b.by / cell_size) as i32
                        } else {
                            anim.tile_idx == (b.bx as u32 + b.by as u32 * sim.map_w)
                        }
                    });

                    if let Some(anim) = anim {
                        let elapsed = anim.start_time.elapsed().as_secs_f32();
                        let p = (elapsed / anim.duration.as_secs_f32()).clamp(0.0, 1.0);

                        let pulse = (p * std::f32::consts::PI * 4.0).sin().abs();
                        pulse_scale = 1.0 + pulse * 0.25;

                        let color = egui::Color32::from_rgb(
                            (251.0 + (255.0 - 251.0) * p) as u8,
                            (191.0 + (255.0 - 191.0) * p) as u8,
                            (36.0 + (255.0 - 36.0) * p) as u8,
                        );
                        (color, egui::Color32::from_rgb(245, 158, 11), 1.5_f32 + (1.0_f32 - p) * 1.5_f32)
                    } else {
                        (egui::Color32::WHITE, egui::Color32::BLACK, 1.0_f32)
                    }
                } else {
                    (egui::Color32::WHITE, egui::Color32::BLACK, 1.0_f32)
                };

                let badge_rect = egui::Rect::from_center_size(bg_center, egui::vec2(badge_w * pulse_scale, badge_h * pulse_scale));
                let rounding = egui::Rounding::same(bg_radius * pulse_scale);
                painter.rect_filled(badge_rect, rounding, bg_color);
                painter.rect_stroke(badge_rect, rounding, egui::Stroke::new(stroke_width, stroke_color));

                painter.text(
                    bg_center,
                    egui::Align2::CENTER_CENTER,
                    text_val,
                    egui::FontId::proportional(font_size),
                    egui::Color32::BLACK,
                );

                // Floating upgrade animation
                if b.has_active_upgrade {
                    let anim = ui.active_upgrades.iter().find(|anim| {
                        let abx = (anim.tile_idx % sim.map_w) as f32;
                        let aby = (anim.tile_idx / sim.map_w) as f32;
                        if cell_size > 1.0 {
                            (abx / cell_size) as i32 == (b.bx / cell_size) as i32
                                && (aby / cell_size) as i32 == (b.by / cell_size) as i32
                        } else {
                            anim.tile_idx == (b.bx as u32 + b.by as u32 * sim.map_w)
                        }
                    });
                    if let Some(anim) = anim {
                        let elapsed = anim.start_time.elapsed().as_secs_f32();
                        let p = (elapsed / anim.duration.as_secs_f32()).clamp(0.0, 1.0);
                        let alpha = 1.0 - p;

                        let float_y = p * base_size * 1.5;
                        let emoji_pos = egui::pos2(center.x, center.y - float_y);

                        painter.text(
                            emoji_pos,
                            egui::Align2::CENTER_CENTER,
                            "🔼",
                            egui::FontId::proportional(base_size.round()),
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8),
                        );

                        let text_pos = egui::pos2(emoji_pos.x, emoji_pos.y + base_size * 0.5);
                        painter.text(
                            text_pos,
                            egui::Align2::CENTER_CENTER,
                            "UPGRADE!",
                            egui::FontId::proportional((base_size * 0.35).clamp(8.0, 14.0).round()),
                            egui::Color32::from_rgba_unmultiplied(74, 222, 128, (255.0 * alpha) as u8),
                        );
                    }
                }
            }
    }
}
