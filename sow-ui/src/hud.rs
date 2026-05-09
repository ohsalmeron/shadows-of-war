use egui::{Context, Align, Layout, Color32, RichText, Slider};
use crate::UiAction;

pub struct HudState {
    pub gold: f64,
    pub troops: f64,
    pub max_troops: f64,
    pub attack_ratio: f32,
    pub is_mobile: bool,
}

#[allow(deprecated)]
pub fn draw(ctx: &Context, state: &mut HudState) -> Option<UiAction> {
    let mut action = None;

    // Top Panel: Economy
    egui::Panel::top("economy_panel").show(ctx, |ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(format!("Troops: {:.0} / {:.0}", state.troops, state.max_troops)).color(Color32::WHITE).size(16.0));
            ui.add_space(20.0);
            ui.label(RichText::new(format!("Gold: {:.0}", state.gold)).color(Color32::GOLD).size(16.0));
        });
    });

    // Bottom Panel: Attack Controls
    egui::Panel::bottom("attack_panel").show(ctx, |ui| {
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.label("Attack Ratio:");
            let mut ratio = state.attack_ratio;
            if ui.add(Slider::new(&mut ratio, 0.01..=1.0).text("")).changed() {
                action = Some(UiAction::SetAttackRatio(ratio));
            }
            if ui.button("10%").clicked() { action = Some(UiAction::SetAttackRatio(0.1)); }
            if ui.button("50%").clicked() { action = Some(UiAction::SetAttackRatio(0.5)); }
            if ui.button("100%").clicked() { action = Some(UiAction::SetAttackRatio(1.0)); }
        });
    });

    action
}
