use super::plates::*;

use crate::render::world::movers::world_to_tile;
use crate::render::world::utils::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_building_placement_preview(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    _time: &crate::app::TimeState,
    gfx: &mut crate::app::GraphicsState,
    painter: &egui::Painter,
    snap: &sow_core::protocol::SimSnapshot,
    hovered_tile_idx: Option<u32>,
    zoom_scaled: f32,
    final_scale: f32,
    sf: f32,
    config: &sow_core::game_config::GameConfig,
) {
    if !sow_ui_kit::theme::dev_config::DevConfig::get().vfx_placement_preview {
        return;
    }
    if let Some(kind) = ui.app.hud_state.selected_building_kind {
        if let Some(hovered_t) = hovered_tile_idx {
            let h_col = (hovered_t as i32) % sim.map_w as i32;
            let h_row = (hovered_t as i32) / sim.map_w as i32;
            let my_id = sim.my_player_id.unwrap_or(0);
            let owners = gfx
                .map_renderer
                .as_ref()
                .map(|mr| mr.owners.as_slice())
                .unwrap_or(&[]);

            let terrain = gfx
                .map_renderer
                .as_ref()
                .map(|mr| mr.terrain.as_slice())
                .unwrap_or(&[]);

            let target_res = crate::input::resolve_build_target_tile(
                kind,
                h_col,
                h_row,
                sim.map_w,
                sim.map_h,
                owners,
                terrain,
                my_id,
                &snap.buildings,
            );

            let stack_target = crate::input::find_stack_target_tile(
                kind,
                h_col,
                h_row,
                sim.map_w,
                my_id,
                &snap.buildings,
            )
            .and_then(|tile| {
                snap.buildings
                    .iter()
                    .find(|b| b.tile_idx == tile && b.owner_id == my_id && b.kind == kind)
            });

            let preview_tile = target_res.unwrap_or(hovered_t);
            let is_valid = target_res.is_ok();

            let cost = {
                let i = sow_core::game::BuildingKind::ALL
                    .iter()
                    .position(|&k| k == kind)
                    .unwrap_or(0);
                ui.app.hud_state.building_costs[i]
            };

            let has_gold = ui.app.hud_state.gold >= cost;
            let tx = (preview_tile % sim.map_w) as f32;
            let ty = (preview_tile / sim.map_w) as f32;
            let hex_w_cx = tx + 0.5;
            let hex_w_cy = ty + 0.5;
            let center_x = (input.camera_x + hex_w_cx * input.camera_zoom) / sf;
            let center_y = (input.camera_y + hex_w_cy * input.camera_zoom) / sf;
            let preview_center = egui::pos2(center_x, center_y);

            // Draw SNAPPED outline
            let square_half = 0.5 * input.camera_zoom / sf;
            const SQUARE_OFFSETS: [egui::Vec2; 4] = [
                egui::vec2(1.0, -1.0),
                egui::vec2(1.0, 1.0),
                egui::vec2(-1.0, 1.0),
                egui::vec2(-1.0, -1.0),
            ];
            let points = [
                preview_center + SQUARE_OFFSETS[0] * square_half,
                preview_center + SQUARE_OFFSETS[1] * square_half,
                preview_center + SQUARE_OFFSETS[2] * square_half,
                preview_center + SQUARE_OFFSETS[3] * square_half,
            ];

            let is_stack = stack_target.is_some();
            let can_place = is_valid && has_gold;
            let outline_color = if can_place {
                egui::Color32::from_rgb(34, 211, 238) // Cyan — new build and upgrade
            } else {
                egui::Color32::from_rgb(239, 68, 68) // Red — invalid / broke
            };

            painter.add(egui::Shape::convex_polygon(
                points.to_vec(),
                egui::Color32::from_rgba_unmultiplied(
                    outline_color.r(),
                    outline_color.g(),
                    outline_color.b(),
                    25,
                ),
                egui::Stroke::new(3.0_f32, outline_color),
            ));

            let base_size = get_building_icon_size(zoom_scaled) * final_scale;

            let bounce_scale = if let Some(last_time) = ui.last_build_confirm_time {
                let elapsed = last_time.elapsed().as_secs_f32();
                if elapsed < 0.4 {
                    ui.egui_ctx.request_repaint();
                    let t = elapsed / 0.4;
                    let scale_offset = (t * std::f32::consts::PI * 2.5).sin() * (1.0 - t) * 0.35;
                    1.0 + scale_offset
                } else {
                    1.0
                }
            } else {
                1.0
            };

            if is_stack {
                // 1. Upgrade hover highlight: glow only, no duplicate sprite
                let glow_color = if has_gold {
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 45)
                // Soft white highlight glow
                } else {
                    egui::Color32::from_rgba_unmultiplied(239, 68, 68, 30) // Soft red warning glow
                };
                painter.circle_filled(preview_center, base_size * 0.55, glow_color);

                // 2. Upgrade plate
                if let Some(sb) = stack_target {
                    let current_lvl = sb.active_level();
                    let target_lvl = sb.level;

                    let border_color = if has_gold {
                        egui::Color32::from_rgb(250, 204, 21) // Gold
                    } else {
                        egui::Color32::from_rgb(239, 68, 68) // Red
                    };

                    let mut lines = vec![BuildingUpgradePlateLine {
                        text: upgrade_level_label(current_lvl),
                        color: egui::Color32::WHITE, // Clear white text
                        scale: 1.0,
                    }];

                    if target_lvl > current_lvl {
                        lines.push(BuildingUpgradePlateLine {
                            text: format!("(+{} queued)", target_lvl - current_lvl),
                            color: egui::Color32::from_rgb(34, 211, 238), // Soft premium cyan for queue status
                            scale: 0.85,
                        });
                    }

                    let plate = BuildingUpgradePlate {
                        anchor: preview_center,
                        base_size,
                        border_color,
                        lines,
                    };

                    paint_building_upgrade_plate(
                        ui,
                        painter,
                        plate,
                        input.camera_zoom,
                        final_scale * bounce_scale,
                        sf,
                    );
                }
            } else {
                // New placement: ghost emoji and range circle
                if kind == sow_core::game::BuildingKind::Bunker {
                    let current_range = config.bunker_range as f32;
                    let range_color = egui::Color32::from_rgba_unmultiplied(239, 68, 68, 120);
                    let range_fill = egui::Color32::from_rgba_unmultiplied(239, 68, 68, 10);
                    let (preview_col, preview_row) = world_to_tile(
                        (preview_center.x * sf - input.camera_x) / input.camera_zoom,
                        (preview_center.y * sf - input.camera_y) / input.camera_zoom,
                    );
                    paint_bunker_hex_range(
                        painter,
                        preview_col,
                        preview_row,
                        current_range,
                        WorldPaintCamera {
                            camera_x: input.camera_x,
                            camera_y: input.camera_y,
                            camera_zoom: input.camera_zoom,
                            sf,
                        },
                        range_fill,
                        range_color,
                    );
                }

                paint_new_build_ghost(gfx, painter, kind, preview_center, base_size, sf);
            }

            // 3. Gold surplus/deficit indicator below
            let (amount_text, text_color) = if has_gold {
                let leftover = ui.app.hud_state.gold - cost;
                (
                    sow_ui::utils::format_number(leftover),
                    egui::Color32::from_rgb(74, 222, 128), // Green
                )
            } else {
                let deficit = cost - ui.app.hud_state.gold;
                (
                    format!("-{}", sow_ui::utils::format_number(deficit)),
                    egui::Color32::from_rgb(248, 113, 113), // Red
                )
            };

            paint_gold_preview_indicator(
                painter,
                preview_center,
                base_size,
                &amount_text,
                text_color,
                zoom_scaled,
                final_scale * bounce_scale,
            );
        }
    }
}
