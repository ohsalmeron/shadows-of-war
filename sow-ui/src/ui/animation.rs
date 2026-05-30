/// Selection grow: 1.0 at rest, `1.0 + grow` when selected. Uses egui ease (no spring overshoot).
///
/// `id` must be stable across frames (e.g. `Id::new(("leader_picker_select", leader))`), not
/// `ui.id()`, or sibling widgets above the picker can reset the animation when layout changes.
pub fn selection_grow_scale(
    ctx: &egui::Context,
    id: egui::Id,
    selected: bool,
    grow: f32,
    duration_secs: f32,
) -> f32 {
    let t = ctx.animate_bool_with_time(id, selected, duration_secs);
    if t > 0.0 && t < 1.0 {
        ctx.request_repaint();
    }
    1.0 + grow * t
}
