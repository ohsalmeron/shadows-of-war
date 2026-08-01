pub const GAP: f32 = 0.0;
pub const ACCENT_BAR_H: f32 = 2.0;
pub const BASELINE_H: f32 = 1.0;

#[inline]
pub fn height(compact: bool) -> f32 {
    if compact { 28.0 } else { 30.0 }
}
