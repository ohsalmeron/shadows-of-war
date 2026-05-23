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

            for b in &snap.buildings {
                let bx = (b.tile_idx % sim.map_w) as f32;
                let by = (b.tile_idx / sim.map_w) as f32;
                let screen_x = (input.camera_x + (bx + 0.5) * input.camera_zoom) / sf;
                let screen_y = (input.camera_y + (by + 0.5) * input.camera_zoom) / sf;

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

                let uri = match b.kind {
                    sow_core::game::BuildingKind::City => "bytes://city.svg",
                    sow_core::game::BuildingKind::Factory => "bytes://factory.svg",
                    sow_core::game::BuildingKind::Port => "bytes://port.svg",
                    sow_core::game::BuildingKind::Industry => "bytes://factory.svg",
                    sow_core::game::BuildingKind::Cultural => "bytes://city.svg",
                    sow_core::game::BuildingKind::Science => "bytes://sam_launcher.svg",
                    sow_core::game::BuildingKind::DefensePost => "bytes://defense_post.svg",
                    sow_core::game::BuildingKind::SamLauncher => "bytes://sam_launcher.svg",
                    sow_core::game::BuildingKind::MissileSilo => "bytes://missile_silo.svg",
                };

                let base_size = get_building_icon_size(zoom_scaled);
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
                        egui::Color32::from_black_alpha(128)
                    } else {
                        egui::Color32::BLACK
                    };
                    painter.image(
                        texture.id,
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        tint,
                    );
                }

                // S6: O(1) HashSet lookup instead of linear scan
                let active_upgrade = if upgrade_tiles.contains(&b.tile_idx) {
                    ui.active_upgrades.iter().find(|anim| anim.tile_idx == b.tile_idx)
                } else {
                    None
                };

                // Level badge
                let is_constructing = b.under_construction;
                let text_val = if is_constructing { "🔨" } else { get_level_str(b.level) };
                let font_size = (zoom_scaled * 0.4).clamp(8.0, 12.0).round();
                let mut bg_radius = font_size * 0.8;

                let bg_center = egui::pos2(center.x + base_size * 0.35, center.y - base_size * 0.35);

                // S9: Fix float literal suffixes
                let (bg_color, stroke_color, stroke_width) = if let Some(anim) = active_upgrade {
                    let elapsed = anim.start_time.elapsed().as_secs_f32();
                    let p = (elapsed / anim.duration.as_secs_f32()).clamp(0.0, 1.0);

                    let pulse = (p * std::f32::consts::PI * 4.0).sin().abs();
                    bg_radius *= 1.0 + pulse * 0.25;

                    let color = egui::Color32::from_rgb(
                        (251.0 + (255.0 - 251.0) * p) as u8,
                        (191.0 + (255.0 - 191.0) * p) as u8,
                        (36.0 + (255.0 - 36.0) * p) as u8,
                    );
                    (color, egui::Color32::from_rgb(245, 158, 11), 1.5_f32 + (1.0_f32 - p) * 1.5_f32)
                } else {
                    (egui::Color32::WHITE, egui::Color32::BLACK, 1.0_f32)
                };

                painter.circle_filled(bg_center, bg_radius, bg_color);
                painter.circle_stroke(bg_center, bg_radius, egui::Stroke::new(stroke_width, stroke_color));
                painter.text(
                    bg_center,
                    egui::Align2::CENTER_CENTER,
                    text_val,
                    egui::FontId::proportional(font_size),
                    egui::Color32::BLACK,
                );

                // Floating upgrade animation
                if let Some(anim) = active_upgrade {
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
