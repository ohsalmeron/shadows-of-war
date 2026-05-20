use crate::app::SowApp;

impl SowApp {
    pub(crate) fn pump_hold_attack(&mut self, ctx: &egui::Context) {
        if let Some((target_owner, press_start, sx, sy, has_fired_initial)) = self.input.hold_attack_target {
            let held_ms = press_start.elapsed().as_millis();
            // Only start streaming after 300ms grace period (to distinguish from quick-click)
            if held_ms > 300 {
                // Check cursor hasn't drifted too far from press origin
                let dx = self.input.last_mouse_x - sx;
                let dy = self.input.last_mouse_y - sy;
                if dx * dx + dy * dy <= 2500.0 {
                    if !has_fired_initial {
                        // Mobile hold threshold reached -> fire initial burst
                        self.input.hold_attack_target = Some((target_owner, press_start, sx, sy, true));
                        self.input.hold_attack_accum = 0.0;
                        
                        let troops = self.ui.app.hud_state.troops * (self.ui.app.hud_state.attack_ratio as f64);
                        if troops > 0.0 {
                            let attack = sow_core::protocol::AttackIntent {
                                target_owner,
                                troops: Some(troops),
                            };
                            let intent = sow_core::protocol::GameplayIntent::Attack(attack);
                            if let Some(c) = self.net.client.as_ref() {
                                if let Ok(json) = bincode::serialize(&sow_core::protocol::ClientMessage::Gameplay { intent: intent.clone() }) {
                                    c.send(json);
                                }
                            } else {
                                self.sim.offline_intents.push(intent);
                            }
                        }
                    } else {
                        // Accumulate real time since last pump
                        let dt = ctx.input(|i| i.predicted_dt);
                        self.input.hold_attack_accum += dt;

                        // Send one attack every 250ms
                        while self.input.hold_attack_accum >= 0.25 {
                            self.input.hold_attack_accum -= 0.25;
                            // 25% of the bar settings (bar is attack_ratio)
                            let ratio_per_tick = (self.ui.app.hud_state.attack_ratio as f64) * 0.25;
                            let troops = self.ui.app.hud_state.troops * ratio_per_tick;
                            if troops > 0.0 {
                                let attack = sow_core::protocol::AttackIntent {
                                    target_owner,
                                    troops: Some(troops),
                                };
                                let intent = sow_core::protocol::GameplayIntent::Attack(attack);
                                if let Some(c) = self.net.client.as_ref() {
                                    if let Ok(json) = bincode::serialize(&sow_core::protocol::ClientMessage::Gameplay { intent: intent.clone() }) {
                                        c.send(json);
                                    }
                                } else {
                                    self.sim.offline_intents.push(intent);
                                }
                            }
                        }
                    }
                } else {
                    // Drifted too far, cancel hold
                    self.input.hold_attack_target = None;
                    self.input.hold_attack_accum = 0.0;
                }
            }
        }
    }
}
