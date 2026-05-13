use egui::{Align2, Color32, Context, RichText, Slider};
use crate::UiAction;

pub struct HudState {
    pub gold: f64,
    pub troops: f64,
    pub max_troops: f64,
    pub attack_ratio: f32,
    pub is_mobile: bool,
    pub spawn_timer_secs: Option<f32>,
    pub sync_state: Option<sow_core::protocol::ServerSyncStateMessage>,
}

#[allow(deprecated)]
pub fn draw(ctx: &Context, state: &mut HudState) -> Option<UiAction> {
    let mut action = None;

    egui::Panel::top("economy_panel").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Troops: {:.0} / {:.0}", state.troops, state.max_troops));
            ui.add_space(20.0);
            ui.label(RichText::new(format!("Gold: {:.0}", state.gold)).color(Color32::GOLD));
        });
    });

    if let Some(secs) = state.spawn_timer_secs {
        egui::Window::new("deployment_phase")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(Align2::CENTER_TOP, [0.0, 50.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading(RichText::new("DEPLOYMENT PHASE").color(Color32::GOLD).size(32.0));
                    ui.label(RichText::new(format!("{:.1}s remaining", secs)).size(24.0));
                    ui.add_space(10.0);
                    ui.label("Click anywhere on the map to place your capital!");
                });
            });
    }

    // Bottom Panel: Attack Controls
    egui::Panel::bottom("attack_panel").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
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

    // Keep these controls pinned to screen corners so mobile wrapping never pushes them left.
    egui::Area::new(egui::Id::new("hud_exit_button"))
        .anchor(Align2::RIGHT_TOP, egui::vec2(-8.0, 8.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            if ui
                .button(RichText::new("Exit").size(14.0).color(Color32::RED))
                .on_hover_text("Exit Game")
                .clicked()
            {
                action = Some(UiAction::LeaveLobby);
            }
        });

    egui::Area::new(egui::Id::new("hud_center_camera_button"))
        .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-8.0, -8.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            if ui
                .button(RichText::new("⌖").size(18.0))
                .on_hover_text("Center Camera")
                .clicked()
            {
                action = Some(UiAction::CenterCamera);
            }
        });

    if let Some(sync) = &state.sync_state {
        // Draw a dark full-screen overlay to block input visually and practically
        let screen_rect = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("sync_overlay")))
            .rect_filled(screen_rect, 0.0, Color32::from_black_alpha(180));

        egui::Window::new("WAITING FOR PLAYERS")
            .collapsible(false)
            .resizable(false)
            .title_bar(false) // Custom polished look
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    if sync.is_starting {
                        ui.label(RichText::new("All Players Ready!").size(24.0).strong().color(Color32::GREEN));
                        ui.label(RichText::new("Stabilizing connection...").size(16.0).color(Color32::LIGHT_GRAY));
                    } else {
                        ui.label(RichText::new("WAITING FOR PLAYERS").size(24.0).strong().color(Color32::WHITE));
                        ui.label(RichText::new(format!("Starting in: {:.1}s", sync.time_remaining)).size(18.0).color(Color32::YELLOW));
                    }
                    
                    ui.add_space(20.0);
                    
                    let total = sync.players.len();
                    let ready = sync.ready_players.len();
                    ui.add(egui::ProgressBar::new(ready as f32 / total as f32)
                        .text(format!("{}/{} Players Ready", ready, total)));
                        
                    ui.add_space(15.0);
                    
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for p in &sync.players {
                            ui.horizontal(|ui| {
                                let is_ready = sync.ready_players.contains(p);
                                if is_ready { 
                                    ui.label(RichText::new("✅").color(Color32::GREEN));
                                } else { 
                                    ui.add(egui::Spinner::new().size(14.0).color(Color32::LIGHT_GRAY));
                                }
                                ui.label(RichText::new(p).color(Color32::WHITE));
                            });
                        }
                    });
                });
            });
    }

    action
}
