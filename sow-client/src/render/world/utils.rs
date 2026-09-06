pub(crate) fn get_building_icon_size(zoom_scaled: f32) -> f32 {
    let size = if zoom_scaled < 10.0 {
        zoom_scaled * 2.0
    } else {
        zoom_scaled * 1.6
    };
    size.clamp(11.0, 96.0)
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BunkerLaserVfxOpts {
    pub target_seeking: bool,
    pub plasma_arc: bool,
    pub volley_scatter: bool,
}

pub(crate) struct WorldPaintCamera {
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub sf: f32,
}

pub(crate) struct BunkerLaserPaint {
    pub center: egui::Pos2,
    pub atk_center: egui::Pos2,
    pub elapsed: f32,
    pub glow_color: egui::Color32,
    pub core_color: egui::Color32,
    pub low_detail: bool,
    pub opts: BunkerLaserVfxOpts,
    pub scatter_seed: u64,
    pub scatter_slot: u32,
}

pub(crate) fn bunker_laser_vfx_opts() -> BunkerLaserVfxOpts {
    let dev = sow_ui_kit::theme::dev_config::DevConfig::get();
    BunkerLaserVfxOpts {
        target_seeking: dev.bunker_laser_target,
        plasma_arc: dev.bunker_laser_arc,
        volley_scatter: dev.bunker_laser_scatter,
    }
}

#[inline]
fn quad_bezier(p0: egui::Pos2, p1: egui::Pos2, p2: egui::Pos2, t: f32) -> egui::Pos2 {
    let u = 1.0 - t;
    egui::pos2(
        u * u * p0.x + 2.0 * u * t * p1.x + t * t * p2.x,
        u * u * p0.y + 2.0 * u * t * p1.y + t * t * p2.y,
    )
}

pub(crate) fn deterministic_scatter_jitter(seed: u64, slot: u32, spread: f32) -> (f32, f32) {
    let h = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(slot as u64);
    let jx = ((h & 0xFFFF) as f32 / 65535.0 - 0.5) * 2.0 * spread;
    let jy = (((h >> 16) & 0xFFFF) as f32 / 65535.0 - 0.5) * 2.0 * spread;
    (jx, jy)
}

pub(crate) fn paint_bunker_hex_range(
    painter: &egui::Painter,
    center_col: i32,
    center_row: i32,
    range: f32,
    camera: WorldPaintCamera,
    fill_color: egui::Color32,
    stroke_color: egui::Color32,
) {
    let WorldPaintCamera {
        camera_x,
        camera_y,
        camera_zoom,
        sf,
    } = camera;
    let range_i = range.ceil() as i32;
    let half = camera_zoom / sf * 0.5;
    let cell = half * 2.0;

    for r_offset in -range_i..=range_i {
        for c_offset in -range_i..=range_i {
            let c = center_col + c_offset;
            let r = center_row + r_offset;
            let d = sow_core::building::hex_distance(c, r, center_col, center_row);
            if d as f32 > range {
                continue;
            }
            let wx = c as f32 + 0.5;
            let wy = r as f32 + 0.5;
            let sx = (camera_x + wx * camera_zoom) / sf;
            let sy = (camera_y + wy * camera_zoom) / sf;
            let rect = egui::Rect::from_center_size(egui::pos2(sx, sy), egui::vec2(cell, cell));
            painter.rect_filled(rect, 0.0, fill_color);

            let on_edge = [
                sow_core::building::hex_distance(c + 1, r, center_col, center_row),
                sow_core::building::hex_distance(c - 1, r, center_col, center_row),
                sow_core::building::hex_distance(c, r + 1, center_col, center_row),
                sow_core::building::hex_distance(c, r - 1, center_col, center_row),
            ]
            .into_iter()
            .any(|nd| nd as f32 > range);
            if on_edge {
                painter.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.5_f32, stroke_color),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
}

pub(crate) fn paint_bunker_laser(painter: &egui::Painter, paint: BunkerLaserPaint) {
    let BunkerLaserPaint {
        center,
        atk_center,
        elapsed,
        glow_color,
        core_color,
        low_detail,
        opts,
        scatter_seed,
        scatter_slot,
    } = paint;
    let mut end = atk_center;
    if opts.volley_scatter {
        let (jx, jy) = deterministic_scatter_jitter(scatter_seed, scatter_slot, 18.0);
        end += egui::vec2(jx, jy);
    }

    if low_detail {
        painter.line_segment([center, end], egui::Stroke::new(5.0_f32, glow_color));
        painter.line_segment(
            [center, end],
            egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
        );
        painter.circle_filled(end, 6.0, egui::Color32::WHITE);
        painter.circle_filled(end, 9.0, glow_color);
        return;
    }

    let dir = end - center;
    let length = dir.length();
    if length <= 1.0 {
        return;
    }
    let perp = egui::vec2(-dir.y, dir.x) / length;

    let arc_ctrl = if opts.plasma_arc {
        let arc_offset = (elapsed * 2.8 + scatter_slot as f32 * 0.7).sin() * length * 0.18;
        Some(center + dir * 0.5 + perp * arc_offset)
    } else {
        None
    };

    let steps = 8;
    let mut prev_pt = center;
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let mut pt = if let Some(ctrl) = arc_ctrl {
            quad_bezier(center, ctrl, end, t)
        } else {
            center + dir * t
        };
        if !opts.plasma_arc && step < steps {
            let offset_mag = (elapsed * 45.0 + step as f32 * 1.6).sin() * 5.0
                + (elapsed * 95.0 - step as f32 * 2.3).cos() * 2.5;
            pt += perp * offset_mag;
        }

        painter.line_segment(
            [prev_pt, pt],
            egui::Stroke::new(8.0_f32, glow_color.linear_multiply(0.55)),
        );
        painter.line_segment([prev_pt, pt], egui::Stroke::new(3.5_f32, core_color));
        painter.line_segment(
            [prev_pt, pt],
            egui::Stroke::new(1.2_f32, egui::Color32::WHITE),
        );
        prev_pt = pt;
    }

    let angle = (end.y - center.y).atan2(end.x - center.x);
    let trail_len = 20.0_f32;
    for p_idx in 0..3 {
        let t = (elapsed * 3.0 + p_idx as f32 * 0.33 + scatter_slot as f32 * 0.11) % 1.0;
        let proj_pos = if let Some(ctrl) = arc_ctrl {
            quad_bezier(center, ctrl, end, t)
        } else {
            egui::pos2(
                center.x + (end.x - center.x) * t,
                center.y + (end.y - center.y) * t,
            )
        };
        let trail_start = egui::pos2(
            proj_pos.x - angle.cos() * trail_len,
            proj_pos.y - angle.sin() * trail_len,
        );
        painter.line_segment(
            [trail_start, proj_pos],
            egui::Stroke::new(4.5_f32, glow_color),
        );
        painter.circle_filled(proj_pos, 5.0, egui::Color32::WHITE);
        painter.circle_filled(proj_pos, 7.5, glow_color);
    }

    let ring_t = (elapsed * 4.0 + scatter_slot as f32 * 0.2) % 1.0;
    painter.circle(
        end,
        ring_t * 26.0,
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(
            2.5_f32,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, ((1.0 - ring_t) * 230.0) as u8),
        ),
    );

    let spark_t = (elapsed * 6.0) % 1.0;
    for i in 0..8 {
        let spark_angle =
            (i as f32 * 45.0 + elapsed * 280.0 + scatter_slot as f32 * 17.0).to_radians();
        let spark_len = spark_t * 20.0;
        let spark_start =
            end + egui::vec2(spark_angle.cos(), spark_angle.sin()) * (spark_len * 0.25);
        let spark_end = end + egui::vec2(spark_angle.cos(), spark_angle.sin()) * spark_len;
        painter.line_segment(
            [spark_start, spark_end],
            egui::Stroke::new(
                2.2_f32,
                egui::Color32::from_rgba_unmultiplied(
                    255,
                    235,
                    130,
                    ((1.0 - spark_t) * 255.0) as u8,
                ),
            ),
        );
    }
}

pub(crate) fn get_level_str(level: u8) -> &'static str {
    match level {
        0 => "0",
        1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        5 => "5",
        6 => "6",
        7 => "7",
        8 => "8",
        9 => "9",
        10 => "10",
        11 => "11",
        12 => "12",
        13 => "13",
        14 => "14",
        15 => "15",
        16 => "16",
        17 => "17",
        18 => "18",
        19 => "19",
        20 => "20",
        21 => "21",
        22 => "22",
        23 => "23",
        24 => "24",
        25 => "25",
        26 => "26",
        27 => "27",
        28 => "28",
        29 => "29",
        30 => "30",
        _ => "99+",
    }
}
