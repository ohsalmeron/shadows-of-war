/// Allow very wide map views (scroll / pinch clamp to this minimum).
pub const CAMERA_MIN_ZOOM: f32 = 0.001;
/// Hard ceiling so zoom stays finite and GPU paths stay well-behaved.
pub const CAMERA_MAX_ZOOM_CAP: f32 = 8192.0;

/// Pixels-per-world-unit zoom max scales with window size so you can fill ~one map tile
/// across the long screen axis.
pub fn camera_zoom_upper_bound(screen_w: f32, screen_h: f32) -> f32 {
    let longest = screen_w.max(screen_h).max(1.0);
    (longest * 3.0).clamp(768.0, CAMERA_MAX_ZOOM_CAP)
}
