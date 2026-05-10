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

    egui::Panel::top("economy_panel").show(ctx, |ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button(RichText::new("Exit").size(14.0).color(Color32::RED)).on_hover_text("Exit Game").clicked() {
                action = Some(UiAction::LeaveLobby);
            }
            if ui.button(RichText::new("⌖").size(18.0)).on_hover_text("Center Camera").clicked() {
                action = Some(UiAction::CenterCamera);
            }
            ui.add_space(20.0);
            ui.label(format!("Troops: {:.0} / {:.0}", state.troops, state.max_troops));
            ui.add_space(20.0);
            ui.label(RichText::new(format!("Gold: {:.0}", state.gold)).color(Color32::GOLD));
        });
    });

    // Bottom Panel: Attack Controls
    egui::Panel::bottom("attack_panel").show(ctx, |ui| {
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.label("Attack Ratio:");
            let mut ratio = state.attack_ratio;
            if ui
                .add(Slider::new(&mut ratio, 0.01..=1.0).show_value(false).text(""))
                .changed()
            {
                action = Some(UiAction::SetAttackRatio(ratio));
            }
            if ui.button("1%").clicked() {
                action = Some(UiAction::SetAttackRatio(0.01));
            }
            if ui.button("10%").clicked() {
                action = Some(UiAction::SetAttackRatio(0.1));
            }
            if ui.button("25%").clicked() {
                action = Some(UiAction::SetAttackRatio(0.25));
            }
            if ui.button("50%").clicked() {
                action = Some(UiAction::SetAttackRatio(0.5));
            }
            if ui.button("100%").clicked() {
                action = Some(UiAction::SetAttackRatio(1.0));
            }
        });
    });

    action
}
