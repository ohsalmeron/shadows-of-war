use wyrand::WyRand;

use crate::rng::NextIntExt;
use sow_data::PREMIUM_COLORS;

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> [f32; 3] {
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    let delta = max - min;
    let v = max;
    let s = if max == 0.0 { 0.0 } else { delta / max };
    let mut h = 0.0;
    if delta > 0.00001 {
        if r == max {
            h = (g - b) / delta;
        } else if g == max {
            h = 2.0 + (b - r) / delta;
        } else {
            h = 4.0 + (r - g) / delta;
        }
        h /= 6.0;
        if h < 0.0 {
            h += 1.0;
        }
    }
    [h, s, v]
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h.clamp(0.0, 1.0).fract();
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let i = (h * 6.0).floor() as i32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [r, g, b]
}

pub fn premium_color(index: usize) -> [f32; 3] {
    PREMIUM_COLORS[index % 300].rgb
}

/// Planar team territory color (Red/Blue). Every teammate shares the exact
/// same color so the whole team reads as one bloc on the map and in the HUD.
/// This is the single canonical palette for team colors — leaderboard,
/// map shader, nameplates and endgame all derive from here.
#[inline]
pub fn team_territory_rgb(team: crate::protocol::Team) -> [f32; 3] {
    match team {
        crate::protocol::Team::Red => [1.0, 0.2, 0.2],
        crate::protocol::Team::Blue => [0.2, 0.5, 1.0],
    }
}

/// RGB used for human-owned territory in the sow-render map shader (`map.wgsl`).
/// Matches WGSL `owner_id <= 16` branch so UI (nameplates) matches the map tint.
#[inline]
pub fn human_shader_territory_rgb(player_id: u16) -> [f32; 3] {
    let base_color = &PREMIUM_COLORS[(player_id as usize).saturating_sub(1) % 300];
    if (1..=300).contains(&player_id) {
        base_color.rgb
    } else {
        let [r, g, b] = base_color.rgb;
        let [h, s, v] = rgb_to_hsv(r, g, b);

        let seed = (player_id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut rng = WyRand::new(seed);

        let hj = h + (rng.next_int(0, 1000) as f32 / 1000.0 - 0.5) * 0.03;
        let sj = (s + (rng.next_int(0, 1000) as f32 / 1000.0 - 0.5) * 0.08).clamp(0.4, 1.0);
        let mut vj = (v + (rng.next_int(0, 1000) as f32 / 1000.0 - 0.5) * 0.08).clamp(0.45, 1.0);

        if hj.fract().abs() >= 0.03 && hj.fract().abs() <= 0.15 && vj < 0.60 {
            vj = 0.65 + (vj * 0.3);
        }

        hsv_to_rgb(hj, sj, vj)
    }
}

pub fn bot_territory_color(game_seed: u64, bot_id: u16) -> [f32; 3] {
    let base_color = &PREMIUM_COLORS[(bot_id as usize).saturating_sub(1) % 300];
    if (1..=300).contains(&bot_id) {
        base_color.rgb
    } else {
        let [r, g, b] = base_color.rgb;
        let [h, s, v] = rgb_to_hsv(r, g, b);

        let mix = game_seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (bot_id as u64).wrapping_shl(32)
            ^ (bot_id as u64);
        let mut rng = WyRand::new(mix);

        let hj = h + (rng.next_int(0, 1000) as f32 / 1000.0 - 0.5) * 0.03;
        let sj = (s + (rng.next_int(0, 1000) as f32 / 1000.0 - 0.5) * 0.08).clamp(0.4, 1.0);
        let mut vj = (v + (rng.next_int(0, 1000) as f32 / 1000.0 - 0.5) * 0.08).clamp(0.45, 1.0);

        if hj.fract().abs() >= 0.03 && hj.fract().abs() <= 0.15 && vj < 0.60 {
            vj = 0.65 + (vj * 0.3);
        }

        let [r_res, g_res, b_res] = hsv_to_rgb(hj, sj, vj);
        [
            r_res.clamp(0.05, 0.95),
            g_res.clamp(0.05, 0.95),
            b_res.clamp(0.05, 0.95),
        ]
    }
}
