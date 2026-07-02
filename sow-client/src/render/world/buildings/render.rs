use super::super::*;
use super::bunker::paint_bunker_effects;
use super::cluster;
use super::overlays::paint_building_overlays;
use super::plates::*;
use super::preview::paint_building_placement_preview;

use crate::config::ClientVisualConfig;
use crate::render::world::movers::world_to_tile;
use crate::render::world::utils::*;

#[allow(unused_variables)]
pub(crate) fn render(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    time: &crate::app::TimeState,
    gfx: &mut crate::app::GraphicsState,
    ctx: &RenderContext,
) {
    if !sow_ui_kit::theme::dev_config::DevConfig::get().vfx_world_buildings {
        return;
    }

    let default_config;
    let config = if let Some(e) = sim.engine.as_ref() {
        &e.state.config
    } else {
        default_config = sow_core::game_config::GameConfig::default();
        &default_config
    };
    if ctx.zoom_scaled < super::super::BUILDINGS_HIDE_FLOOR {
        return;
    }

    let edge_cache_stale = sim
        .current_snapshot
        .as_ref()
        .is_some_and(|s| !s.dirty_tiles.is_empty());
    let painter = ctx.painter.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("world_buildings"),
    ));
    let sf = ctx.sf;
    let zoom_scaled = ctx.zoom_scaled;
    let player_colors = ctx.player_colors;
    let building_scale = sow_ui_kit::theme::dev_config::DevConfig::get().building_scale;
    let far_zoom_threshold = ClientVisualConfig::default().far_zoom_lod_threshold;
    let zoom_factor = ((zoom_scaled - super::super::BUILDINGS_HIDE_FLOOR) / 9.0).clamp(0.0, 1.0);
    let min_lod_scale = 0.5; // Scale when fully zoomed out
    let max_lod_scale = 1.0; // Scale when fully zoomed in
    let lod_scale = min_lod_scale + (max_lod_scale - min_lod_scale) * zoom_factor;
    let final_scale = building_scale * lod_scale;

    if let Some(snap) = &sim.current_snapshot {
        let mx = input.last_mouse_x as f32;
        let my = input.last_mouse_y as f32;
        let world_x = (mx - input.camera_x) / input.camera_zoom;
        let world_y = (my - input.camera_y) / input.camera_zoom;
        let (h_col, h_row) = world_to_tile(world_x, world_y);
        let hovered_tile_idx =
            if h_col >= 0 && h_row >= 0 && h_col < sim.map_w as i32 && h_row < sim.map_h as i32 {
                Some((h_row * sim.map_w as i32 + h_col) as u32)
            } else {
                None
            };

        let mut rendered_buildings =
            cluster::collect_rendered_buildings(snap, sim.map_w, zoom_scaled, far_zoom_threshold);
        rendered_buildings.sort_by(|a, b| {
            a.by.partial_cmp(&b.by)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.bx.partial_cmp(&b.bx).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.count.cmp(&b.count))
        });

        let mut gpu_rendered = false;
        if let Some(ref mut tr) = gfx.text_renderer {
            gpu_rendered = true;
            // SDF outline — font dev settings control all emoji outlines globally.
            let dev = sow_ui_kit::theme::dev_config::DevConfig::get();
            let outline_px = dev.font_outline_thickness * sf;
            let shadow_px = dev.font_shadow_y * sf;
            for b in &rendered_buildings {
                // Render the building as a full emoji + alpha outline, sized to `base_size`
                // (same as the egui path), positioned in screen space like all other text.
                let base_size = if b.count > 1 {
                    28.0_f32.max(get_building_icon_size(zoom_scaled) * 1.2)
                } else {
                    get_building_icon_size(zoom_scaled)
                } * final_scale;
                let screen_x = input.camera_x + b.bx * input.camera_zoom;
                let screen_y = input.camera_y + b.by * input.camera_zoom;
                let a = if b.under_construction { 0.5f32 } else { 1.0f32 };
                tr.push_emoji(
                    building_kind_emoji(b.kind),
                    [screen_x, screen_y],
                    base_size * sf / 2.0,
                    [1.0, 1.0, 1.0, a],
                    [0.0, 0.0, 0.0, a],
                    outline_px,
                    shadow_px,
                );
            }
        }

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

            let base_size = if b.count > 1 {
                28.0_f32.max(get_building_icon_size(zoom_scaled) * 1.2)
            } else {
                get_building_icon_size(zoom_scaled)
            } * final_scale;
            let rect = egui::Rect::from_center_size(center, egui::vec2(base_size, base_size));

            // Icon sprite rendering

            if !gpu_rendered {
                let player_color = if b.owner_id != 0 {
                    player_colors
                        .get(b.owner_id as usize)
                        .copied()
                        .unwrap_or(egui::Color32::WHITE)
                } else {
                    egui::Color32::WHITE
                };

                let tint = if b.owner_id != 0 {
                    let mut color = player_color;
                    if b.under_construction {
                        color = color.gamma_multiply(0.5);
                    }
                    color
                } else {
                    if b.under_construction {
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 128)
                    } else {
                        egui::Color32::WHITE
                    }
                };

                let emoji = building_kind_emoji(b.kind);

                if !sow_ui_kit::widgets::try_paint_emoji(&painter, emoji, rect, tint) {
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        emoji,
                        egui::FontId::proportional(base_size * 0.7),
                        tint,
                    );
                }

                // Render Automated City Districts (Port, Silo, Foundry)
                if b.kind == sow_core::game::BuildingKind::City
                    && b.count == 1
                    && zoom_scaled >= 1.5
                {
                    if let (Some(b_id), Some(mods)) = (b.id, b.modules) {
                        let district_size = base_size * 0.75;
                        let neighbors_offsets = [
                            (0.85_f32, 0.0_f32),
                            (0.425_f32, 0.736_f32),
                            (-0.425_f32, 0.736_f32),
                            (-0.85_f32, 0.0_f32),
                            (-0.425_f32, -0.736_f32),
                            (0.425_f32, -0.736_f32),
                        ];

                        let draw_district = |emoji: &str, dir_idx: usize| {
                            let (dx, dy) = neighbors_offsets[dir_idx % 6];
                            let dist_cx = screen_x + dx * input.camera_zoom / sf;
                            let dist_cy = screen_y + dy * input.camera_zoom / sf;
                            let dist_center = egui::pos2(dist_cx, dist_cy);
                            let dist_rect = egui::Rect::from_center_size(
                                dist_center,
                                egui::vec2(district_size, district_size),
                            );

                            let player_color = if b.owner_id != 0 {
                                player_colors
                                    .get(b.owner_id as usize)
                                    .copied()
                                    .unwrap_or(egui::Color32::WHITE)
                            } else {
                                egui::Color32::WHITE
                            };

                            // Draw connector line
                            painter.line_segment(
                                [center, dist_center],
                                egui::Stroke::new(
                                    1.2_f32,
                                    egui::Color32::from_rgba_unmultiplied(
                                        player_color.r(),
                                        player_color.g(),
                                        player_color.b(),
                                        60,
                                    ),
                                ),
                            );

                            if !sow_ui_kit::widgets::try_paint_emoji(
                                &painter,
                                emoji,
                                dist_rect,
                                player_color,
                            ) {
                                painter.text(
                                    dist_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    emoji,
                                    egui::FontId::proportional(district_size * 0.7),
                                    player_color,
                                );
                            }
                        };

                        if mods.arsenal > 0 {
                            draw_district("🚀", (b_id % 6) as usize);
                        }
                        if mods.port > 0 {
                            draw_district("⚓", ((b_id + 2) % 6) as usize);
                        }
                        if mods.foundry > 0 {
                            draw_district("🏭", ((b_id + 4) % 6) as usize);
                        }
                    }
                }

                paint_bunker_effects(
                    ui,
                    sim,
                    input,
                    time,
                    gfx,
                    ctx,
                    &painter,
                    snap,
                    config,
                    &b,
                    center,
                    base_size,
                    zoom_scaled,
                    sf,
                    edge_cache_stale,
                    player_colors,
                );
            }

            paint_building_overlays(
                ui,
                sim,
                input,
                time,
                gfx,
                &painter,
                snap,
                config,
                &b,
                center,
                base_size,
                zoom_scaled,
                final_scale,
                sf,
                hovered_tile_idx,
                player_colors,
            );
        }

        paint_building_placement_preview(
            ui,
            sim,
            input,
            time,
            gfx,
            &painter,
            snap,
            hovered_tile_idx,
            zoom_scaled,
            final_scale,
            sf,
            config,
        );
    }
}
