//! The screen-space attention pointer ("here") that guides the player to a target.

/// Animated attention pointer drawn at a screen position: a steady ring, two expanding
/// "sonar ping" rings, and a bobbing chevron above. egui painter only (GPU via blade_egui)
/// — no separate particle/shader pipeline needed for a 2D screen-space hint.
pub(super) fn draw_tutorial_pointer(ctx: &egui::Context, target: egui::Pos2) {
    let t = ctx.input(|i| i.time);
    // Middle layer: above the map/nameplates, but BELOW the Foreground dialog so the
    // dialog is never covered by the effect.
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("tutorial_pointer"),
    ));
    let gold = egui::Color32::from_rgb(255, 200, 90);
    // Frames the 20px-radius tutorial avatar with a small gap.
    let base_r = 26.0_f32;

    // Two expanding, fading rings (sonar ping).
    for k in 0..2 {
        let phase = ((t * 1.1 + k as f64 * 0.5) % 1.0) as f32;
        let r = base_r + phase * base_r * 2.2;
        painter.circle_stroke(
            target,
            r,
            egui::Stroke::new(2.0_f32, gold.gamma_multiply(1.0 - phase)),
        );
    }

    // Steady inner ring.
    painter.circle_stroke(target, base_r, egui::Stroke::new(2.5_f32, gold));

    // Bobbing downward chevron above the ring.
    let bob = (t * 3.0).sin() as f32 * 5.0;
    let tip = egui::pos2(target.x, target.y - base_r - 10.0 + bob);
    let (w, h) = (9.0_f32, 11.0_f32);
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(tip.x - w, tip.y - h),
            egui::pos2(tip.x + w, tip.y - h),
            tip,
        ],
        gold,
        egui::Stroke::NONE,
    ));

    // Keep the animation running even without other input.
    ctx.request_repaint();
}
