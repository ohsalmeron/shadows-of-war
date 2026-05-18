use web_time::{Duration, Instant};
use egui::{Align, Align2, Color32, Context, Layout, RichText, Slider, Stroke, pos2, vec2};
use crate::UiAction;
use sow_core::protocol::{AttackSnapshot, FleetSnapshot, PlayerSnapshot};

pub struct HudState {
    pub gold: f64,
    pub troops: f64,
    pub troops_display: f64,
    pub max_troops: f64,
    pub max_troops_display: f64,
    pub attack_ratio: f32,
    pub is_mobile: bool,
    pub spawn_timer_secs: Option<f32>,
    pub sync_state: Option<sow_core::protocol::ServerSyncStateMessage>,
    pub(crate) last_troops_ui_refresh: Option<Instant>,
    pub my_player_id: u16,
    pub attacks: Vec<AttackSnapshot>,
    pub fleets: Vec<FleetSnapshot>,
    pub players: Vec<PlayerSnapshot>,
}

impl HudState {
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

pub fn draw(ui: &mut egui::Ui, state: &mut HudState, cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>) -> Option<UiAction> {
    let mut action = None;
    state.refresh_troop_display_if_due();
    let compact = ui.ctx().content_rect().width() < 768.0;

    let panel_w = if compact { ui.ctx().content_rect().width() } else { 500.0 };

    egui::Panel::bottom("hud_bottom_panel")
        .frame(egui::Frame::NONE.inner_margin(if compact { egui::Margin::ZERO } else { egui::Margin::symmetric(0, 8) }))
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            // Center the content horizontally and register the space properly
            ui.vertical_centered(|ui| {
                ui.set_max_width(panel_w);
                
                ui.spacing_mut().item_spacing.y = 4.0;
            
            // 1. Attacks Display (top in vertical stack)
            ui.push_id("attacks_display", |ui| {
                draw_attacks_display(ui, state, panel_w, compact, cancel_intents, &mut action);
            });

            // 2. Control Panel (bottom in vertical stack)
            ui.push_id("control_panel_frame", |ui| {
                let panel_bg = crate::ui::theme::panel_bg_transparent();
                let frame = egui::Frame::NONE
                    .fill(panel_bg)
                    .corner_radius(if compact {
                        egui::CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }
                    } else {
                        egui::CornerRadius::same(8)
                    })
                    .inner_margin(egui::Margin::symmetric(8, 6));
                
                frame.show(ui, |ui| {
                    ui.set_width(panel_w);
                    if let Some(secs) = state.spawn_timer_secs {
                        draw_spawn_panel(ui, secs, compact);
                    } else {
                        draw_control_panel(ui, state, compact, &mut action);
                    }
                });
            });
            });
        });

    // ── Top-right HUD buttons (Keep original logic) ───────────────────────────
    egui::Area::new(egui::Id::new("hud_exit_button"))
        .anchor(Align2::RIGHT_TOP, vec2(-12.0, 12.0))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE
                .fill(Color32::from_black_alpha(150))
                .corner_radius(8)
                .stroke(Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border()))
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("⌖").size(18.0).color(Color32::WHITE)).on_hover_text("Center Camera").clicked() {
                            action = Some(UiAction::CenterCamera);
                        }
                        if ui.button(RichText::new("⚙").size(18.0).color(Color32::WHITE)).on_hover_text("Settings").clicked() {
                            action = Some(UiAction::ToggleSettings);
                        }
                        if ui.button(RichText::new("✖").size(18.0).color(Color32::from_rgb(255, 100, 100))).on_hover_text("Exit").clicked() {
                            action = Some(UiAction::LeaveLobby);
                        }
                    });
                });
        });

    draw_sync_overlay(ui.ctx(), state);

    action
}

fn get_player_display_name(players: &[PlayerSnapshot], id: u16, default: &str) -> String {
    players.iter().find(|p| p.id == id).map(|p| {
        if p.name.is_empty() {
            if p.id >= 200 {
                format!("Tribe {}", p.id - 199)
            } else {
                format!("Nation {}", p.id - 103)
            }
        } else {
            p.name.clone()
        }
    }).unwrap_or_else(|| default.to_string())
}

fn draw_attacks_display(
    ui: &mut egui::Ui,
    state: &HudState,
    width: f32,
    _compact: bool,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    _action: &mut Option<UiAction>,
) {
    let my_pid = state.my_player_id;
    if my_pid == 0 { return; }

    let attack_bg = crate::ui::theme::panel_bg_transparent();

    enum AttackDisplayItem {
        Incoming { troops: f64, attacker_name: String, retreating: bool },
        Outgoing { troops: f64, target_name: String, retreating: bool, attack_id: u64 },
        Fleet { troops: f64, retreating: bool, fleet_id: u64 },
    }

    let mut items = Vec::new();

    for attack in state.attacks.iter().filter(|a| a.target_owner == my_pid) {
        let attacker_name = get_player_display_name(&state.players, attack.owner_id, "Unknown");
        items.push(AttackDisplayItem::Incoming { troops: attack.troops, attacker_name, retreating: attack.retreating });
    }
    for attack in state.attacks.iter().filter(|a| a.owner_id == my_pid) {
        let target_name = get_player_display_name(&state.players, attack.target_owner, "Wilderness");
        items.push(AttackDisplayItem::Outgoing { troops: attack.troops, target_name, retreating: attack.retreating, attack_id: attack.id });
    }
    for fleet in state.fleets.iter().filter(|f| f.owner_id == my_pid) {
        items.push(AttackDisplayItem::Fleet { troops: fleet.troops, retreating: fleet.retreating, fleet_id: fleet.id });
    }

    if items.is_empty() {
        return;
    }

    // A 2-column grid format with max 8 items visible without scrolling (~4 rows)
    egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
        ui.set_width(width);
        
        let cols = 2;
        let cell_w = (width - (cols as f32 - 1.0) * 4.0) / cols as f32;

        for chunk in items.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for item in chunk {
                    egui::Frame::NONE
                        .fill(attack_bg)
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(4, 2))
                        .show(ui, |ui| {
                            ui.set_width(cell_w);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                // Draw button on the right first to prevent wrapping
                                match item {
                                    AttackDisplayItem::Incoming { retreating, .. } => {
                                        if *retreating {
                                            ui.label(RichText::new("(retreating...)").size(10.0).color(Color32::GRAY));
                                        } else {
                                            let retaliate = egui::Button::new(RichText::new("⚔").size(12.0))
                                                .fill(crate::ui::theme::accent_danger())
                                                .stroke(Stroke::new(1.0_f32, crate::ui::theme::accent_danger_border()))
                                                .corner_radius(6);
                                            let _ = ui.add_sized(vec2(24.0, 24.0), retaliate).on_hover_text("Retaliate");
                                        }
                                    }
                                    AttackDisplayItem::Outgoing { retreating, attack_id, .. } => {
                                        if *retreating {
                                            ui.label(RichText::new("(retreating...)").size(10.0).color(crate::ui::theme::accent_solo_cyan()));
                                        } else {
                                            let cancel_btn = egui::Button::new(RichText::new("❌").size(10.0))
                                                .corner_radius(6);
                                            if ui.add_sized(vec2(24.0, 24.0), cancel_btn).clicked() {
                                                cancel_intents.push(sow_core::protocol::GameplayIntent::CancelAttack { attack_id: *attack_id });
                                            }
                                        }
                                    }
                                    AttackDisplayItem::Fleet { retreating, fleet_id, .. } => {
                                        if *retreating {
                                            ui.label(RichText::new("(retreating...)").size(10.0).color(crate::ui::theme::accent_solo_cyan()));
                                        } else {
                                            let cancel_btn = egui::Button::new(RichText::new("❌").size(10.0))
                                                .corner_radius(6);
                                            if ui.add_sized(vec2(24.0, 24.0), cancel_btn).clicked() {
                                                cancel_intents.push(sow_core::protocol::GameplayIntent::RecallFleet { fleet_id: *fleet_id });
                                            }
                                        }
                                    }
                                }

                                // Then draw the left-aligned text
                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    match item {
                                        AttackDisplayItem::Incoming { troops, attacker_name, .. } => {
                                            let txt = format!("★↓ {} {}", crate::utils::format_number(*troops), attacker_name);
                                            ui.label(RichText::new(txt).size(12.0).color(crate::ui::theme::accent_danger_border()).strong());
                                        }
                                        AttackDisplayItem::Outgoing { troops, target_name, .. } => {
                                            let txt = format!("★↑ {} {}", crate::utils::format_number(*troops), target_name);
                                            ui.label(RichText::new(txt).size(12.0).color(crate::ui::theme::accent_solo_cyan()).strong());
                                        }
                                        AttackDisplayItem::Fleet { troops, .. } => {
                                            let txt = format!("★↑ {} Naval Invasion", crate::utils::format_number(*troops));
                                            ui.label(RichText::new(txt).size(12.0).color(crate::ui::theme::accent_solo_cyan()).strong());
                                        }
                                    }
                                });
                            });
                        });
                }
            });
        }
    });
}

fn draw_control_panel(ui: &mut egui::Ui, state: &HudState, compact: bool, action: &mut Option<UiAction>) {

    let troop_rate = (state.max_troops * 0.1).max(0.0); // Approximation
    let is_increasing = true; // Simplified

    if compact {
        // Mobile Layout: 1 Row [Gold] [Troop Bar] [Ratio] [Slider]
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;

            // Gold
            egui::Frame::NONE
                .stroke(Stroke::new(1.0_f32, crate::ui::theme::accent_ranked_gold_hover()))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(4, 4))
                .show(ui, |ui| {
                    ui.label(RichText::new(format!("💰 {}", crate::utils::format_number(state.gold))).strong().size(12.0).color(crate::ui::theme::accent_ranked_gold_hover()));
                });

            // Troop Bar (Takes ~40%)
            let bar_w = ui.available_width() * 0.5;
            let (rect, _resp) = ui.allocate_exact_size(vec2(bar_w, 24.0), egui::Sense::hover());
            draw_troop_bar(ui, rect, state.troops_display, state.max_troops_display, troop_rate, true, is_increasing);

            // Attack Ratio + Slider
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("⚔ {:.0}%", state.attack_ratio * 100.0)).strong().size(12.0).color(Color32::from_rgb(220, 230, 220)));
                    let mut ratio = state.attack_ratio;
                    if ui.add_sized(vec2(ui.available_width(), 16.0), Slider::new(&mut ratio, 0.01..=1.0).show_value(false)).changed() {
                        *action = Some(UiAction::SetAttackRatio(ratio));
                    }
                });
            });
        });
    } else {
        // Desktop Layout: 
        // Row 1: [Rate] [Troop Bar] [Gold]
        // Row 2: [Ratio] [Slider]
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                // Rate
                let rate_color = if is_increasing { crate::ui::theme::accent_solo_cyan_hover() } else { crate::ui::theme::accent_danger() };
                egui::Frame::NONE
                    .stroke(Stroke::new(1.0_f32, rate_color))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        ui.label(RichText::new(format!("★ +{}/s", crate::utils::format_number(troop_rate))).strong().size(14.0).color(rate_color));
                    });

                // Troop Bar (Flex-1)
                let bar_w = ui.available_width() - 80.0; // Reserve space for gold
                let (rect, _resp) = ui.allocate_exact_size(vec2(bar_w.max(100.0), 24.0), egui::Sense::hover());
                draw_troop_bar(ui, rect, state.troops_display, state.max_troops_display, troop_rate, false, is_increasing);

                // Gold
                egui::Frame::NONE
                    .stroke(Stroke::new(1.0_f32, crate::ui::theme::accent_ranked_gold_hover()))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        ui.label(RichText::new(format!("💰 {}", crate::utils::format_number(state.gold))).strong().size(14.0).color(crate::ui::theme::accent_ranked_gold_hover()));
                    });
            });

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                
                // Attack Ratio Box
                egui::Frame::NONE
                    .stroke(Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border()))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        let ratio_troops = (state.troops * (state.attack_ratio as f64)).max(0.0);
                        ui.label(RichText::new(format!("⚔ {:.0}% ({})", state.attack_ratio * 100.0, crate::utils::format_number(ratio_troops))).strong().size(14.0).color(Color32::from_rgb(220, 230, 220)));
                    });

                let mut ratio = state.attack_ratio;
                if ui.add_sized(vec2(ui.available_width(), 20.0), Slider::new(&mut ratio, 0.01..=1.0).show_value(false)).changed() {
                    *action = Some(UiAction::SetAttackRatio(ratio));
                }
            });
        });
    }
}

fn draw_troop_bar(ui: &mut egui::Ui, rect: egui::Rect, troops: f64, max_troops: f64, troop_rate: f64, compact: bool, is_increasing: bool) {
    let base = max_troops.max(1.0);
    let green_pct = (troops / base).clamp(0.0, 1.0) as f32;
    
    // Animate the backfiller so it smoothly catches up to the current troop level
    let catchup_pct = ui.ctx().animate_value_with_time(
        ui.id().with("troop_bar_catchup"),
        green_pct,
        2.0, // Two seconds to drain
    );

    let green_pct_f32 = green_pct;
    // The orange bar (dark green visually) is the gap between the actual troops and the animated catchup
    let orange_pct_f32 = (catchup_pct - green_pct).max(0.0);

    let bg_color = crate::ui::theme::nickname_field_bg();
    let green_color = crate::ui::theme::accent_solo_cyan_hover();
    let orange_color = crate::ui::theme::accent_solo_cyan();

    // Draw background
    ui.painter().rect(
        rect,
        6,
        bg_color,
        Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border()),
        egui::StrokeKind::Inside,
    );

    // Draw green fill
    if green_pct_f32 > 0.0 {
        let green_rect = egui::Rect::from_min_size(rect.min, vec2(rect.width() * green_pct_f32, rect.height()));
        let green_radius = if orange_pct_f32 > 0.0 {
            egui::CornerRadius { nw: 6, ne: 0, sw: 6, se: 0 }
        } else {
            egui::CornerRadius::same(6)
        };
        ui.painter().rect_filled(green_rect, green_radius, green_color);
    }
    
    // Draw orange fill (backfiller)
    if orange_pct_f32 > 0.0 {
        let orange_start = rect.min.x + rect.width() * green_pct_f32;
        let orange_rect = egui::Rect::from_min_size(pos2(orange_start, rect.min.y), vec2(rect.width() * orange_pct_f32, rect.height()));
        ui.painter().rect_filled(
            orange_rect, 
            egui::CornerRadius { nw: 0, ne: 6, sw: 0, se: 6 }, 
            orange_color
        );
    }

    // Overlay text
    if compact {
        // Text overlays for mobile
        ui.painter().text(
            pos2(rect.left() + 4.0, rect.center().y),
            Align2::LEFT_CENTER,
            crate::utils::format_number(troops),
            egui::FontId::proportional(12.0),
            Color32::from_rgb(220, 230, 220),
        );
        ui.painter().text(
            pos2(rect.right() - 4.0, rect.center().y),
            Align2::RIGHT_CENTER,
            crate::utils::format_number(max_troops),
            egui::FontId::proportional(12.0),
            Color32::from_rgb(220, 230, 220),
        );
        let rate_color = if is_increasing { crate::ui::theme::accent_solo_cyan_hover() } else { crate::ui::theme::accent_danger() };
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            format!("★ +{}/s", crate::utils::format_number(troop_rate)),
            egui::FontId::proportional(10.0),
            rate_color,
        );
    } else {
        // Desktop overlay: "troops / max_troops ★" centered
        let text = format!("{} / {} ★", crate::utils::format_number(troops), crate::utils::format_number(max_troops));
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(14.0),
            Color32::from_rgb(220, 230, 220),
        );
    }
}

fn draw_spawn_panel(ui: &mut egui::Ui, secs: f32, compact: bool) {
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("CHOOSE A STARTING LOCATION").strong().size(if compact { 16.0 } else { 20.0 }).color(crate::ui::theme::accent_ranked_gold_hover()));
        ui.label(RichText::new(format!("{:.1}s remaining", secs)).size(14.0).color(Color32::from_rgb(220, 230, 220)));
    });
}

fn draw_sync_overlay(ctx: &Context, state: &HudState) {
    if let Some(sync) = &state.sync_state {
        let screen_rect = ctx.content_rect();
        ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("sync_overlay")))
            .rect_filled(screen_rect, 0.0, Color32::from_black_alpha(180));

        egui::Window::new("WAITING FOR PLAYERS")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
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
                    ui.add(egui::ProgressBar::new(ratio).text(format!("{}/{} Players Ready", ready, total)));

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
}
