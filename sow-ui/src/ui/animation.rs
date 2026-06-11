/// Damped spring overshoot: approaches 1.0 with a single bounce.
#[inline]
pub fn spring_overshoot(t: f32) -> f32 {
    1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
}

/// Vertical slide distance for modal panel entry/exit.
pub const PANEL_Y_SLIDE: f32 = 80.0;

pub struct PanelInOutAnim {
    pub progress: f32,
    pub scale: f32,
    pub y_offset: f32,
}

/// Shared in/out animation for victory, defeat, and betrayal modals.
pub fn panel_in_out_anim(
    ctx: &egui::Context,
    id: egui::Id,
    visible: bool,
    duration_secs: f32,
) -> PanelInOutAnim {
    let progress = ctx.animate_bool_with_time(id, visible, duration_secs);
    if progress > 0.0 && progress < 1.0 {
        ctx.request_repaint();
    }
    let scale = if visible {
        if progress >= 1.0 {
            1.0
        } else {
            spring_overshoot(progress)
        }
    } else {
        progress
    };
    PanelInOutAnim {
        progress,
        scale,
        y_offset: PANEL_Y_SLIDE * (1.0 - scale),
    }
}

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

/// Cubic ease-out for general UI fades.
#[inline]
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Snappy ease-out quart — fast page turns, no lazy drift.
#[inline]
pub fn ease_out_quart(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(4)
}

/// Book page-turn duration (cached portrait); keep within 250–320 ms.
pub const LEADER_PAGE_TURN_DURATION: f32 = 0.28;

/// Brief minimum on the loading “page” so the spinner is readable.
pub const LEADER_PAGE_LOADING_MIN: f32 = 0.12;

#[inline]
pub fn leader_page_turn_t(elapsed: f32) -> f32 {
    ease_out_quart((elapsed / LEADER_PAGE_TURN_DURATION).clamp(0.0, 1.0))
}

/// Outgoing portrait fades out over the first ~55% of the page turn.
pub const LEADER_PAGE_FADE_OUT_END: f32 = 0.55;

/// Incoming portrait fades in over the last ~55% of the page turn.
pub const LEADER_PAGE_FADE_IN_START: f32 = 0.45;

#[inline]
pub fn leader_page_out_alpha(turn_t: f32) -> f32 {
    let u = (turn_t / LEADER_PAGE_FADE_OUT_END).clamp(0.0, 1.0);
    1.0 - ease_out_quart(u)
}

#[inline]
pub fn leader_page_in_alpha(turn_t: f32) -> f32 {
    let u =
        ((turn_t - LEADER_PAGE_FADE_IN_START) / (1.0 - LEADER_PAGE_FADE_IN_START)).clamp(0.0, 1.0);
    ease_out_quart(u)
}
