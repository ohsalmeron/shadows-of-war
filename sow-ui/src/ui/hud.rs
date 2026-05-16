use web_time::{Duration, Instant};

use egui::{Align2, Color32, Context, RichText, Slider};
use crate::UiAction;

pub struct HudState {
    pub gold: f64,
    /// Sim truth (updated on sim ticks); attack intents must use this.
    pub troops: f64,
    /// Throttled for the HUD label only (~2/s), OpenFront-style.
    pub troops_display: f64,
    pub max_troops: f64,
    pub max_troops_display: f64,
    pub attack_ratio: f32,
    pub is_mobile: bool,
    pub spawn_timer_secs: Option<f32>,
    pub sync_state: Option<sow_core::protocol::ServerSyncStateMessage>,
    pub connection_lost: bool,
    pub(crate) last_troops_ui_refresh: Option<Instant>,
}

impl HudState {
    /// Call each frame. Wall clock (not egui time) caps label updates at ~2/s.
    pub fn refresh_troop_display_if_due(&mut self) {
        const MIN_INTERVAL: Duration = Duration::from_millis(50);
        let now = Instant::now();
        let refresh = match self.last_troops_ui_refresh {
            None => true,
            Some(t) if now.duration_since(t) >= MIN_INTERVAL => true,
            _ => false,
        };
        if refresh {
            self.troops_display = self.troops;
            self.max_troops_display = self.max_troops;
            self.last_troops_ui_refresh = Some(now);
        }
    }
}

#[allow(deprecated)]
pub fn draw(ctx: &Context, state: &mut HudState) -> Option<UiAction> {
    let mut action = None;

    state.refresh_troop_display_if_due();

    if state.connection_lost {
        // Draw a dark full-screen overlay
        let screen_rect = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("connection_lost_overlay")))
            .rect_filled(screen_rect, 0.0, Color32::from_black_alpha(200));

        egui::Window::new("CONNECTION LOST")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("CONNECTION LOST").size(32.0).strong().color(Color32::RED));
                    ui.add_space(10.0);
                    ui.label(RichText::new("You have been disconnected from the server.").size(18.0).color(Color32::LIGHT_GRAY));
                    ui.add_space(30.0);
                    
                    if ui.add_sized([200.0, 40.0], egui::Button::new(RichText::new("Main Menu").size(20.0))).clicked() {
                        action = Some(UiAction::LeaveLobby);
                    }
                });
            });

        return action; // Do not draw the rest of the HUD
    }

    // Top panel removed as requested.

    if let Some(secs) = state.spawn_timer_secs {
        egui::Window::new("deployment_phase")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(Align2::CENTER_TOP, [0.0, 50.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading(RichText::new("DEPLOYMENT PHASE").color(Color32::GOLD).size(32.0));
                    ui.allocate_ui_with_layout(egui::vec2(250.0, 30.0), egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{:.1}s remaining", secs)).size(24.0));
                    });
                    ui.add_space(10.0);
                    ui.label("Click anywhere on the map to place your capital!");
                });
            });
    }

    // Bottom Panel: Economy & Attack Controls (Modern Floating Layout)
    egui::Area::new(egui::Id::new("hud_bottom_panel"))
        .anchor(Align2::CENTER_BOTTOM, egui::vec2(0.0, -20.0))
        .show(ctx, |ui| {
            let frame = egui::Frame::window(&ctx.style())
                .rounding(16.0)
                .fill(Color32::from_black_alpha(220))
                .inner_margin(16.0)
                .stroke(egui::Stroke::new(1.0_f32, Color32::from_white_alpha(40)));
            
            frame.show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    // Troops display with progress bar
                    ui.vertical(|ui| {
                        let fraction = if state.max_troops_display > 0.0 {
                            (state.troops_display / state.max_troops_display) as f32
                        } else {
                            0.0
                        };
                        
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Troops").strong().size(16.0).color(Color32::WHITE));
                            ui.label(RichText::new(format!("{:.0} / {:.0}", state.troops_display, state.max_troops_display)).color(Color32::LIGHT_GRAY));
                        });
                        
                        ui.add(
                            egui::ProgressBar::new(fraction)
                                .desired_width(180.0)
                                .desired_height(14.0)
                                .fill(Color32::from_rgb(40, 150, 255))
                        );
                    });
                    
                    ui.add_space(20.0);
                    
                    // Gold display
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Gold").strong().size(16.0).color(Color32::GOLD));
                        ui.label(RichText::new(format!("{:.0}", state.gold)).size(20.0).strong().color(Color32::GOLD));
                    });
                    
                    ui.add_space(20.0);
                    
                    // Attack Controls
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Attack Strength").strong().size(16.0).color(Color32::WHITE));
                        ui.horizontal(|ui| {
                            let mut ratio = state.attack_ratio;
                            if ui
                                .add(Slider::new(&mut ratio, 0.01..=0.5).show_value(false).text(format!("{:.0}%", state.attack_ratio * 100.0)))
                                .changed()
                            {
                                action = Some(UiAction::SetAttackRatio(ratio));
                            }
                            if ui.button("1%").clicked() {
                                action = Some(UiAction::SetAttackRatio(0.01));
                            }
                            if ui.button("Max").clicked() {
                                action = Some(UiAction::SetAttackRatio(0.5));
                            }
                        });
                    });
                });
            });
        });

    egui::Area::new(egui::Id::new("hud_exit_button"))
        .anchor(Align2::RIGHT_TOP, egui::vec2(-12.0, 12.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Color32::from_black_alpha(200))
                .corner_radius(10.0)
                .stroke(egui::Stroke::new(1.0_f32, Color32::from_white_alpha(30)))
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let gear = egui::Button::new(RichText::new("⚙").size(18.0).color(Color32::from_gray(200)))
                            .fill(Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE);
                        if ui.add(gear).on_hover_text("Settings").clicked() {
                            action = Some(UiAction::ToggleSettings);
                        }

                        ui.add_space(2.0);

                        let exit = egui::Button::new(RichText::new("✖").size(18.0).color(Color32::from_rgb(255, 100, 100)))
                            .fill(Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE);
                        if ui.add(exit).on_hover_text("Exit Game").clicked() {
                            action = Some(UiAction::LeaveLobby);
                        }
                    });
                });
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
        ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("sync_overlay")))
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
                    let ready = sync.players.iter().filter(|p| p.is_ready).count();
                    let ratio = if total == 0 { 0.0 } else { ready as f32 / total as f32 };
                    ui.add(egui::ProgressBar::new(ratio)
                        .text(format!("{}/{} Players Ready", ready, total)));
                        
                    ui.add_space(15.0);
                    
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        for p in &sync.players {
                            ui.horizontal(|ui| {
                                if p.is_ready { 
                                    ui.label(RichText::new("✔").color(Color32::GREEN));
                                } else { 
                                    ui.add(egui::Spinner::new().size(14.0).color(Color32::LIGHT_GRAY));
                                }
                                ui.label(RichText::new(&p.name).color(Color32::WHITE));
                            });
                        }
                    });
                });
            });
    }

    action
}
