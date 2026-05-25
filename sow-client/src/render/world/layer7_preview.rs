use super::*;

use crate::render::world::utils::*;
#[allow(unused_variables)]
pub(crate) fn render(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    time: &crate::app::TimeState,
    gfx: &crate::app::GraphicsState,
    ctx: &RenderContext,
) {
    let painter = ctx.painter;
    let sf = ctx.sf;
    let zoom_scaled = ctx.zoom_scaled;
    let player_colors = ctx.player_colors;
    let terrain = ctx.terrain;

    let building_scale = painter.ctx().data(|d| {
        d.get_temp::<f32>(egui::Id::new("dev_building_scale"))
            .unwrap_or(3.0)
    });
    let zoom_factor = ((zoom_scaled - 0.6) / 9.4).clamp(0.0, 1.0);
    let min_lod_scale = 0.5; // Scale when fully zoomed out
    let max_lod_scale = 1.0; // Scale when fully zoomed in
    let lod_scale = min_lod_scale + (max_lod_scale - min_lod_scale) * zoom_factor;
    let mut final_scale = building_scale * lod_scale;
    if zoom_scaled < 0.6 {
        final_scale *= 0.35; // scale down LOD 3 so it doesn't get too big
    }

    if let Some(snap) = &sim.current_snapshot {
        // --- Layer 7: Building Placement Preview (Ghost structures) ---
        if let Some(kind) = ui.app.hud_state.selected_building_kind {
            let mx = input.last_mouse_x as f32;
            let my = input.last_mouse_y as f32;

            let world_x = (mx - input.camera_x) / input.camera_zoom;
            let world_y = (my - input.camera_y) / input.camera_zoom;

            let q_f = world_x - world_y * 0.577_350_26_f32;
            let r_f = world_y * 1.154_700_5_f32;
            let s_f = -q_f - r_f;

            let mut rq = q_f.round();
            let mut rr = r_f.round();
            let rs = s_f.round();

            let q_diff = (rq - q_f).abs();
            let r_diff = (rr - r_f).abs();
            let s_diff = (rs - s_f).abs();

            if q_diff > r_diff && q_diff > s_diff {
                rq = -rr - rs;
            } else if r_diff > s_diff {
                rr = -rq - rs;
            }

            let col = rq as i32 + (rr as i32 - (rr as i32 & 1)) / 2;
            let row = rr as i32;

            if col >= 0 && row >= 0 && col < sim.map_w as i32 && row < sim.map_h as i32 {
                let map_w = sim.map_w;
                let map_h = sim.map_h;
                let owners = gfx
                    .map_renderer
                    .as_ref()
                    .map(|mr| mr.owners.as_slice())
                    .unwrap_or(&[]);
                let my_id = sim.my_player_id.unwrap_or(0);
                let buildings = sim
                    .current_snapshot
                    .as_ref()
                    .map(|s| s.buildings.as_slice())
                    .unwrap_or(&[]);

                // Check if there is a valid upgrade target within Manhattan distance 8 of (col, row)
                let mut upgrade_building = None;
                if kind.upgradable() {
                    let min_dist = 8;
                    let mut best_dist = 999;
                    for b in buildings {
                        if b.owner_id == my_id && b.kind == kind && !b.under_construction {
                            let bx = (b.tile_idx % map_w) as i32;
                            let by = (b.tile_idx / map_w) as i32;
                            let d = (col - bx).abs() + (row - by).abs();
                            if d <= min_dist
                                && (d < best_dist
                                    || (d == best_dist
                                        && upgrade_building.is_none_or(
                                            |old_b: &sow_core::protocol::BuildingSnapshot| {
                                                b.id < old_b.id
                                            },
                                        )))
                            {
                                best_dist = d;
                                upgrade_building = Some(b);
                            }
                        }
                    }
                }

                let snapped_idx = if let Some(b) = upgrade_building {
                    Some(b.tile_idx)
                } else {
                    crate::input::resolve_building_placement_tile(
                        kind, col, row, map_w, map_h, owners, terrain, my_id, buildings,
                    )
                    .ok()
                };

                let can_afford = {
                    let i = sow_core::game::BuildingKind::ALL
                        .iter()
                        .position(|&k| k == kind)
                        .unwrap_or(0);
                    ui.app.hud_state.gold >= ui.app.hud_state.building_costs[i]
                };

                let (draw_col, draw_row, is_valid) = if let Some(idx) = snapped_idx {
                    ((idx % map_w) as i32, (idx / map_w) as i32, can_afford)
                } else {
                    (col, row, false)
                };

                let world_cx = draw_col as f32 + 0.5 + (draw_row % 2) as f32 * 0.5;
                let world_cy = (draw_row as f32 + 0.5) * 0.8660254_f32;
                let tile_screen_x = (input.camera_x + world_cx * input.camera_zoom) / sf;
                let tile_screen_y = (input.camera_y + world_cy * input.camera_zoom) / sf;
                let tile_size = input.camera_zoom / sf;

                // Draw ghost SVG
                {
                    let uri = kind.asset().uri();
                    let base_size = get_building_icon_size(tile_size) * final_scale;
                    let size_hint = egui::load::SizeHint::Size {
                        width: 64,
                        height: 64,
                        maintain_aspect_ratio: true,
                    };
                    if let Ok(egui::load::TexturePoll::Ready { texture }) = painter
                        .ctx()
                        .try_load_texture(uri, egui::TextureOptions::LINEAR, size_hint)
                    {
                        let (r_t, g_t, b_t) = if is_valid {
                            (34, 211, 238)
                        } else {
                            (239, 68, 68)
                        };
                        let tint = egui::Color32::from_rgba_unmultiplied(
                            ((r_t as f32 * 0.20) + (255.0 * 0.80)) as u8,
                            ((g_t as f32 * 0.20) + (255.0 * 0.80)) as u8,
                            ((b_t as f32 * 0.20) + (255.0 * 0.80)) as u8,
                            166, // 0.65 opacity
                        );
                        painter.image(
                            texture.id,
                            egui::Rect::from_center_size(
                                egui::pos2(tile_screen_x, tile_screen_y),
                                egui::vec2(base_size, base_size),
                            ),
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            tint,
                        );
                    }
                }

                // Lack of Gold indicator
                if !can_afford {
                    let i = sow_core::game::BuildingKind::ALL
                        .iter()
                        .position(|&k| k == kind)
                        .unwrap_or(0);
                    let cost = ui.app.hud_state.building_costs[i];
                    let owned = ui.app.hud_state.gold;
                    let deficit = (cost - owned).max(0.0);
                    let text = format!("🪙 -{}", sow_ui::utils::format_number(deficit));
                    let font_size = (12.0_f32 * input.camera_zoom / sf)
                        .clamp(10.0, 16.0)
                        .round();
                    let font_id = egui::FontId::proportional(font_size);
                    let base_size = get_building_icon_size(tile_size) * final_scale;
                    let text_y = if upgrade_building.is_some() {
                        tile_screen_y - tile_size * 0.65 - 18.0
                    } else {
                        tile_screen_y - base_size * 0.5 - 12.0
                    };

                    sow_ui::ui::theme::outlined_text(
                        painter,
                        egui::pos2(tile_screen_x, text_y),
                        egui::Align2::CENTER_CENTER,
                        &text,
                        font_id,
                        egui::Color32::from_rgb(248, 113, 113), // Beautiful bright soft red/coral
                        egui::Color32::BLACK,
                    );
                }

                // Render Upgrade Tooltip if upgrading
                if let Some(b) = upgrade_building {
                    let next_lvl = b.level + 1;
                    let text = format!("Upgrade Lvl {}", next_lvl);
                    let font_size = (8.5_f32 * input.camera_zoom / sf).clamp(8.0, 12.0).round();
                    let font_id = egui::FontId::proportional(font_size);
                    let galley =
                        painter.layout_no_wrap(text.clone(), font_id.clone(), egui::Color32::WHITE);

                    let padding_x = 6.0_f32;
                    let padding_y = 3.0_f32;
                    let rect_w = galley.rect.width() + padding_x * 2.0;
                    let rect_h = galley.rect.height() + padding_y * 2.0;

                    let tooltip_y = tile_screen_y - tile_size * 0.65;
                    let tooltip_rect = egui::Rect::from_center_size(
                        egui::pos2(tile_screen_x, tooltip_y),
                        egui::vec2(rect_w, rect_h),
                    );

                    // Premium slate blue dark glassmorphic container with cyan outline
                    painter.rect(
                        tooltip_rect,
                        4.0_f32,
                        egui::Color32::from_rgba_unmultiplied(15, 23, 42, 220),
                        egui::Stroke::new(
                            1.0_f32,
                            egui::Color32::from_rgba_unmultiplied(34, 211, 238, 180),
                        ),
                        egui::StrokeKind::Inside,
                    );

                    // Glowing cyan text centered
                    painter.text(
                        egui::pos2(tile_screen_x, tooltip_y),
                        egui::Align2::CENTER_CENTER,
                        &text,
                        font_id,
                        egui::Color32::from_rgb(34, 211, 238),
                    );
                }
            }
        }
    }
}
