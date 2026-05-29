/// Selection grow: 1.0 at rest, `1.0 + grow` when selected. Uses egui ease (no spring overshoot).
pub fn selection_grow_scale(
    ctx: &egui::Context,
    id: egui::Id,
    selected: bool,
    grow: f32,
    duration_secs: f32,
) -> f32 {
    let t = ctx.animate_bool_with_time(id, selected, duration_secs);
    1.0 + grow * t
}
