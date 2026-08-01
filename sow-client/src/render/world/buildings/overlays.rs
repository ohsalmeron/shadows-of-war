use super::cluster::RenderedBuilding;
use super::plates::*;
use crate::render::world::utils::get_level_str;

pub(super) struct BuildingOverlayOpts<'a> {
    pub painter: &'a egui::Painter,
    pub snap: &'a sow_core::protocol::SimSnapshot,
    pub config: &'a sow_core::game_config::GameConfig,
    pub b: &'a RenderedBuilding,
    pub center: egui::Pos2,
    pub base_size: f32,
    pub zoom_scaled: f32,
    pub final_scale: f32,
    pub sf: f32,
    pub hovered_tile_idx: Option<u32>,
    pub player_colors: &'a [egui::Color32],
}

pub(super) fn paint_building_overlays(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    gfx: &mut crate::app::GraphicsState,
    opts: &BuildingOverlayOpts,
) {
    let painter = opts.painter;
    let snap = opts.snap;
    let config = opts.config;
    let b = opts.b;
    let center = opts.center;
    let base_size = opts.base_size;
    let zoom_scaled = opts.zoom_scaled;
    let final_scale = opts.final_scale;
    let sf = opts.sf;
    let hovered_tile_idx = opts.hovered_tile_idx;
    let player_colors = opts.player_colors;
    let dev = sow_ui_kit::theme::dev_config::DevConfig::get();
    let is_my_building = sim.my_player_id == Some(b.owner_id);

    if is_my_building
        && b.under_construction
        && b.ticks_until_complete > 0
        && zoom_scaled >= 1.5
        && dev.vfx_upgrade_plate
    {
        let active_l = b.active_level;
        let target_l = b.target_level;

        let progress = if active_l > 0 {
            let mut queued_above_ticks = 0;
            for lvl in (active_l + 2)..=target_l {
                queued_above_ticks += sow_core::building::core::upgrade_duration_ticks(b.kind, lvl);
            }
            let ticks_current = b.ticks_until_complete.saturating_sub(queued_above_ticks);
            let dur_current =
                sow_core::building::core::upgrade_duration_ticks(b.kind, active_l + 1);
            1.0 - (ticks_current as f32 / dur_current as f32).clamp(0.0, 1.0)
        } else {
            let total_ticks = b.kind.construction_duration_ticks();
            if total_ticks > 0 {
                1.0 - (b.ticks_until_complete as f32 / total_ticks as f32).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };

        let radius = base_size * 0.28;

        // Black transparent circle panel behind the circle loader
        painter.circle_filled(
            center,
            radius + 2.0_f32,
            egui::Color32::from_black_alpha(150),
        );

        // Track outline
        painter.circle_stroke(
            center,
            radius,
            egui::Stroke::new(
                2.5_f32,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 35),
            ),
        );

        if progress > 0.0 {
            // Sharp glowing outer progress arc
            let num_points = (32.0 * progress).ceil().max(2.0) as usize;
            let mut arc_points = Vec::with_capacity(num_points);
            for i in 0..num_points {
                let t = i as f32 / (num_points - 1) as f32;
                let angle = -std::f32::consts::FRAC_PI_2 + t * progress * std::f32::consts::TAU;
                arc_points.push(egui::pos2(
                    center.x + radius * angle.cos(),
                    center.y + radius * angle.sin(),
                ));
            }

            // Golden glowing arc for upgrade, cyan for initial construction
            let arc_color = if active_l > 0 {
                egui::Color32::from_rgb(250, 204, 21) // Amber / Gold
            } else {
                egui::Color32::from_rgb(0, 220, 255) // Cyan
            };

            painter.add(egui::Shape::line(
                arc_points,
                egui::Stroke::new(2.5_f32, arc_color),
            ));
        }
    }

    // Level badge (no white plate background, no frame, larger text in black)
    if b.active_level != 1
        && b.active_level != 0
        && zoom_scaled >= super::super::BUILDINGS_HIDE_FLOOR
    {
        let text_val = get_level_str(b.active_level);
        let font_size = {
            let raw = zoom_scaled * 0.65 * final_scale;
            if dev.clamp_text_zoom {
                raw.clamp(8.0, 18.0)
            } else {
                raw
            }
        }
        .round();
        let bg_center = egui::pos2(center.x + base_size * 0.45, center.y - base_size * 0.45);

        let mut gpu_text_rendered = false;
        if let Some(ref mut tr) = gfx.text_renderer {
            gpu_text_rendered = true;
            let color_arr = [1.0f32, 1.0, 1.0, 1.0];
            let outline_color_arr = [0.0f32, 0.0, 0.0, 1.0];
            let baseline_y = (bg_center.y + font_size * 0.25) * sf;

            let face_dilate = dev.font_face_dilate * sf;
            let outline_thickness = dev.font_outline_thickness * sf;
            let shadow_y = dev.font_shadow_y * sf;
            let underlay_softness = dev.font_underlay_softness * sf;
            let char_spacing = dev.font_char_spacing;
            let font_size_scale = dev.font_size_scale;
            let emoji_scale = dev.emoji_size_scale;

            let settings = crate::render::gpu::TmpFontSettings {
                face_dilate,
                outline_thickness,
                underlay_offset_y: shadow_y,
                underlay_softness,
            };

            tr.push_string(
                text_val,
                [bg_center.x * sf, baseline_y],
                font_size * font_size_scale * sf,
                (color_arr, outline_color_arr),
                settings,
                (0.5, char_spacing, emoji_scale),
            );
        }

        if !gpu_text_rendered {
            let font_id = egui::FontId::proportional(font_size);
            let key = (text_val.to_string(), font_size as u32);
            let galley = ui
                .cached_galleys
                .entry(key)
                .or_insert_with(|| {
                    painter.layout_no_wrap(text_val.to_owned(), font_id, egui::Color32::WHITE)
                })
                .clone();
            let pos = bg_center - galley.rect.size() / 2.0;

            crate::hud::nameplate::paint_glow_nameplate_galley(
                painter,
                pos,
                galley,
                egui::Color32::WHITE,
            );
        }
    }

    // Render premium golden glassmorphic floating egui badge above upgrading building
    if is_my_building
        && b.under_construction
        && b.ticks_until_complete > 0
        && b.active_level > 0
        && b.count == 1
        && zoom_scaled >= 1.5
    {
        let active_l = b.active_level;
        let target_l = b.target_level;
        let queued_count = (target_l as i32 - active_l as i32).max(0) as u32;

        let main_text = upgrade_level_label(active_l);
        let text_color = egui::Color32::from_rgb(254, 240, 138); // Very soft warm golden text

        let mut lines = vec![BuildingUpgradePlateLine {
            text: main_text,
            color: text_color,
            scale: 1.0,
        }];

        if queued_count > 1 {
            lines.push(BuildingUpgradePlateLine {
                text: format!("(+{} queued)", queued_count - 1),
                color: text_color,
                scale: 0.85,
            });
        }

        let border_color = egui::Color32::from_rgb(250, 204, 21); // Amber / Gold

        let plate = BuildingUpgradePlate {
            anchor: center,
            base_size,
            border_color,
            lines,
        };

        paint_building_upgrade_plate(ui, painter, plate, input.camera_zoom, final_scale, sf);
    }

    // Render floating stats tooltip on hover
    if b.active_level > 0 && b.count == 1 && zoom_scaled >= 1.5 {
        let is_hovered =
            if let Some(snap_b) = snap.buildings.iter().find(|sb| sb.id == b.id.unwrap_or(0)) {
                hovered_tile_idx == Some(snap_b.tile_idx)
            } else {
                false
            };

        if is_hovered {
            let b_id = b.id.unwrap_or(0);
            if ui.cached_hovered_building_id != Some(b_id)
                || ui.cached_hovered_building_level != b.active_level
            {
                ui.cached_hovered_building_id = Some(b_id);
                ui.cached_hovered_building_level = b.active_level;
                let lang = ui.app.settings_state.language;
                let s = &sow_i18n::get(lang).hud;
                ui.cached_hovered_building_tooltip = match b.kind {
                    sow_core::game::BuildingKind::Bunker => {
                        let stat1 = s
                            .build_bunker_coverage
                            .replace("{}", &config.bunker_range.round().to_string());
                        format!("{}\n{}", s.build_bunker_title, stat1)
                    }
                    sow_core::game::BuildingKind::Factory => {
                        let stat1 = s
                            .build_factory_gold
                            .replace("{}", &format!("{:.1}", config.factory_gold_income));
                        format!("{}\n{}", s.build_factory_title, stat1)
                    }
                    sow_core::game::BuildingKind::Port => {
                        let title = s
                            .build_port_title
                            .replace("{}", &b.active_level.to_string());
                        let stat2 = s
                            .build_port_troops
                            .replace("{}", &format!("{:.1}", b.active_level as f64 * 25.0));
                        let stat3 = s
                            .build_port_gold
                            .replace("{}", &format!("{:.1}", b.active_level as f64 * 50.0));
                        format!("{}\n{}\n{}\n{}", title, s.build_port_fleet, stat2, stat3)
                    }
                    _ => String::new(),
                };
            }

            let tooltip_text = &ui.cached_hovered_building_tooltip;

            if !tooltip_text.is_empty() {
                let font_size = (9.0_f32 * input.camera_zoom / sf).clamp(9.0, 12.0).round();
                let font_id = egui::FontId::proportional(font_size);
                let key = (tooltip_text.clone(), font_size as u32);
                let galley = ui
                    .cached_galleys
                    .entry(key)
                    .or_insert_with(|| {
                        painter.layout_no_wrap(
                            tooltip_text.clone(),
                            font_id.clone(),
                            egui::Color32::WHITE,
                        )
                    })
                    .clone();

                let padding_x = 8.0_f32;
                let padding_y = 5.0_f32;
                let rect_w = galley.rect.width() + padding_x * 2.0;
                let rect_h = galley.rect.height() + padding_y * 2.0;

                let tooltip_y = center.y - base_size * 0.8;
                let tooltip_rect = egui::Rect::from_center_size(
                    egui::pos2(center.x, tooltip_y),
                    egui::vec2(rect_w, rect_h),
                );

                // Premium slate blue dark glassmorphic container with owner's tint outline
                let player_color = if b.owner_id != 0 {
                    player_colors
                        .get(b.owner_id as usize)
                        .copied()
                        .unwrap_or(egui::Color32::from_rgb(34, 211, 238))
                } else {
                    egui::Color32::from_rgb(34, 211, 238)
                };
                painter.rect(
                    tooltip_rect,
                    4.0_f32,
                    egui::Color32::from_rgba_unmultiplied(15, 23, 42, 220),
                    egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgba_unmultiplied(
                            player_color.r(),
                            player_color.g(),
                            player_color.b(),
                            180,
                        ),
                    ),
                    egui::StrokeKind::Inside,
                );

                painter.galley(
                    egui::pos2(center.x, tooltip_y) - galley.rect.size() / 2.0,
                    galley,
                    egui::Color32::WHITE,
                );
            }
        }
    }
}
