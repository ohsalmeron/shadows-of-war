use web_time::{Duration, Instant};
use egui::{Align2, Color32, Context, RichText, Slider, Stroke, pos2, vec2};
use crate::UiAction;
use sow_core::protocol::{AttackSnapshot, FleetSnapshot, PlayerSnapshot};
use sow_lang::Language;

#[derive(Clone, Debug)]
pub struct SelectedTileInfo {
    pub tile_idx: u32,
    pub owner_id: u16,
    pub is_own_territory: bool,
    pub is_friendly: bool,
    pub is_spawning: bool,
    pub is_land: bool,
}

pub struct HudState {
    pub gold: f64,
    pub troops: f64,
    pub troops_display: f64,
    pub max_troops: f64,
    pub max_troops_display: f64,
    pub attack_ratio: f32,
    pub spawn_timer_secs: Option<f32>,
    pub sync_state: Option<sow_core::protocol::ServerSyncStateMessage>,
    pub(crate) last_troops_ui_refresh: Option<Instant>,
    pub my_player_id: u16,
    pub attacks: Vec<AttackSnapshot>,
    pub fleets: Vec<FleetSnapshot>,
    pub players: Vec<PlayerSnapshot>,
    pub safe_area_top: f32,
    pub safe_area_bottom: f32,
    pub selected_tile: Option<SelectedTileInfo>,
    pub show_emoji_panel: bool,
    pub emoji_panel_pos: Option<egui::Pos2>,
    pub emoji_panel_just_opened: bool,
    pub pin_emoji: bool,
    pub show_alliance_inbox: bool,
    pub show_betrayal_warning: Option<(u16, sow_core::protocol::GameplayIntent)>,
    pub show_error: Option<String>,
    pub(crate) last_error_message: Option<String>,
    pub(crate) error_display_timer: Option<Instant>,
    pub selected_building_kind: Option<sow_core::game::BuildingKind>,
    pub building_costs: [f64; 6],
    pub selected_nuke_kind: Option<sow_core::game::NukeKind>,
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

fn draw_buildings_dock(
    ui: &mut egui::Ui,
    state: &mut HudState,
    width: f32,
    compact: bool,
) {
    let bg = crate::ui::theme::panel_bg_transparent();
    let border_stroke = if state.selected_building_kind.is_some() {
        egui::Stroke::new(1.5_f32, crate::ui::theme::accent_solo_cyan())
    } else {
        egui::Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border())
    };

    egui::Frame::NONE
        .fill(bg)
        .stroke(border_stroke)
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = if compact { 4.0 } else { 12.0 };
                
                let col_w = (width - 16.0 - 5.0 * (if compact { 4.0 } else { 12.0 })) / 6.0;
                
                for (i, &kind) in sow_core::game::BuildingKind::ALL.iter().enumerate() {
                    let cost = state.building_costs[i];
                    let is_selected = state.selected_building_kind == Some(kind);
                    let can_afford = state.gold >= cost;
                    
                    let tint = if is_selected {
                        crate::ui::theme::accent_solo_cyan()
                    } else if !can_afford {
                        egui::Color32::from_rgb(180, 50, 50)
                    } else {
                        egui::Color32::WHITE
                    };
                    
                    let bg_color = if is_selected {
                        crate::ui::theme::accent_solo_cyan().linear_multiply(0.15)
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    
                    let stroke = if is_selected {
                        egui::Stroke::new(1.5_f32, crate::ui::theme::accent_solo_cyan())
                    } else if !can_afford {
                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(180, 50, 50))
                    } else {
                        egui::Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border().linear_multiply(0.5))
                    };
                    
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(col_w, if compact { 36.0 } else { 44.0 }),
                        egui::Sense::click(),
                    );
                    
                    let is_hovered = resp.hovered();
                    let final_bg = if is_hovered && !is_selected {
                        crate::ui::theme::nickname_field_bg().linear_multiply(0.3)
                    } else {
                        bg_color
                    };
                    
                    ui.painter().rect(rect, 6, final_bg, stroke, egui::StrokeKind::Inside);
                    
                    // Hotkey badge (top-left corner)
                    if !compact {
                        let hotkey_color = if is_selected {
                            crate::ui::theme::accent_solo_cyan()
                        } else {
                            egui::Color32::from_white_alpha(120)
                        };
                        ui.painter().text(
                            egui::pos2(rect.left() + 5.0, rect.top() + 5.0),
                            egui::Align2::LEFT_TOP,
                            format!("{}", i + 1),
                            egui::FontId::proportional(7.0),
                            hotkey_color,
                        );
                    }
                    
                    let icon_size = if compact { 16.0 } else { 22.0 };
                    let icon_rect = egui::Rect::from_center_size(
                        egui::pos2(rect.center().x, rect.top() + (if compact { 10.0 } else { 14.0 })),
                        egui::vec2(icon_size, icon_size),
                    );
                    
                    let uri = match kind {
                        sow_core::game::BuildingKind::City => "bytes://city.svg",
                        sow_core::game::BuildingKind::Factory => "bytes://factory.svg",
                        sow_core::game::BuildingKind::Port => "bytes://port.svg",
                        sow_core::game::BuildingKind::DefensePost => "bytes://defense_post.svg",
                        sow_core::game::BuildingKind::SamLauncher => "bytes://sam_launcher.svg",
                        sow_core::game::BuildingKind::MissileSilo => "bytes://missile_silo.svg",
                    };
                    
                    let image = egui::Image::new(uri).tint(tint);
                    image.paint_at(ui, icon_rect);
                    
                    let cost_text = if cost.is_infinite() {
                        "N/A".to_string()
                    } else {
                        crate::utils::format_number(cost)
                    };
                    
                    let text_color = if !can_afford {
                        egui::Color32::from_rgb(239, 68, 68)
                    } else if is_selected {
                        crate::ui::theme::accent_solo_cyan()
                    } else {
                        egui::Color32::GRAY
                    };
                    
                    let label_text = if compact {
                        cost_text
                    } else {
                        format!("{} - {}", kind.as_str(), cost_text)
                    };
                    
                    let font_size = if compact { 8.0 } else { 9.0 };
                    ui.painter().text(
                        egui::pos2(rect.center().x, rect.bottom() - (if compact { 6.0 } else { 8.0 })),
                        egui::Align2::CENTER_CENTER,
                        label_text,
                        egui::FontId::proportional(font_size),
                        text_color,
                    );
                    
                    if resp.clicked() {
                        if is_selected {
                            state.selected_building_kind = None;
                        } else {
                            state.selected_building_kind = Some(kind);
                        }
                    }
                }
                
                // Nuke launch buttons (separator + icons)
                if sow_core::config::ENABLE_MISSILE_STRUCTURES {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                    
                    let nukes: [(sow_core::game::NukeKind, &str, &str); 3] = [
                        (sow_core::game::NukeKind::AtomBomb, "bytes://atombomb.png", "Atom"),
                        (sow_core::game::NukeKind::HydrogenBomb, "bytes://hydrogenbomb.png", "H-Bomb"),
                        (sow_core::game::NukeKind::MIRV, "bytes://mirv.png", "MIRV"),
                    ];
                    
                    for (ni, (nuke_kind, uri, label)) in nukes.iter().enumerate() {
                        let is_selected = state.selected_nuke_kind == Some(*nuke_kind);
                        let nk_col_w = if compact { 28.0 } else { 36.0 };
                        
                        let tint = if is_selected {
                            egui::Color32::from_rgb(239, 68, 68)
                        } else {
                            egui::Color32::WHITE
                        };
                        
                        let bg_color = if is_selected {
                            egui::Color32::from_rgba_unmultiplied(239, 68, 68, 30)
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        
                        let stroke = if is_selected {
                            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(239, 68, 68))
                        } else {
                            egui::Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border().linear_multiply(0.5))
                        };
                        
                        let (rect, resp) = ui.allocate_exact_size(
                            egui::vec2(nk_col_w, if compact { 36.0 } else { 44.0 }),
                            egui::Sense::click(),
                        );
                        
                        let final_bg = if resp.hovered() && !is_selected {
                            crate::ui::theme::nickname_field_bg().linear_multiply(0.3)
                        } else {
                            bg_color
                        };
                        
                        ui.painter().rect(rect, 6, final_bg, stroke, egui::StrokeKind::Inside);
                        
                        // Hotkey badge (top-left corner)
                        if !compact {
                            let hotkey_color = if is_selected {
                                egui::Color32::from_rgb(239, 68, 68)
                            } else {
                                egui::Color32::from_white_alpha(120)
                            };
                            ui.painter().text(
                                egui::pos2(rect.left() + 4.0, rect.top() + 4.0),
                                egui::Align2::LEFT_TOP,
                                format!("{}", ni + 7),
                                egui::FontId::proportional(7.0),
                                hotkey_color,
                            );
                        }
                        
                        let icon_size = if compact { 16.0 } else { 20.0 };
                        let icon_rect = egui::Rect::from_center_size(
                            egui::pos2(rect.center().x, rect.top() + (if compact { 10.0 } else { 14.0 })),
                            egui::vec2(icon_size, icon_size),
                        );
                        let image = egui::Image::new(*uri).tint(tint);
                        image.paint_at(ui, icon_rect);
                        
                        let font_size = if compact { 7.0 } else { 8.0 };
                        ui.painter().text(
                            egui::pos2(rect.center().x, rect.bottom() - (if compact { 6.0 } else { 8.0 })),
                            egui::Align2::CENTER_CENTER,
                            *label,
                            egui::FontId::proportional(font_size),
                            if is_selected { egui::Color32::from_rgb(239, 68, 68) } else { egui::Color32::GRAY },
                        );
                        
                        if resp.clicked() {
                            if is_selected {
                                state.selected_nuke_kind = None;
                            } else {
                                state.selected_nuke_kind = Some(*nuke_kind);
                                state.selected_building_kind = None;
                            }
                        }
                    }
                }
            });
        });
}

pub fn draw(ui: &mut egui::Ui, state: &mut HudState, cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>, lang: Language) -> Option<UiAction> {
    static REGISTER_ONCE: std::sync::Once = std::sync::Once::new();
    REGISTER_ONCE.call_once(|| {
        ui.ctx().include_bytes("bytes://city.svg", include_bytes!("../../../../sow-client/assets/city.svg").as_slice());
        ui.ctx().include_bytes("bytes://factory.svg", include_bytes!("../../../../sow-client/assets/factory.svg").as_slice());
        ui.ctx().include_bytes("bytes://port.svg", include_bytes!("../../../../sow-client/assets/port.svg").as_slice());
        ui.ctx().include_bytes("bytes://defense_post.svg", include_bytes!("../../../../sow-client/assets/defense_post.svg").as_slice());
        ui.ctx().include_bytes("bytes://sam_launcher.svg", include_bytes!("../../../../sow-client/assets/sam_launcher.svg").as_slice());
        ui.ctx().include_bytes("bytes://missile_silo.svg", include_bytes!("../../../../sow-client/assets/missile_silo.svg").as_slice());
        ui.ctx().include_bytes("bytes://trade_ship.svg", include_bytes!("../../../../sow-client/assets/trade_ship.svg").as_slice());
        ui.ctx().include_bytes("bytes://transport_ship.svg", include_bytes!("../../../../sow-client/assets/transport_ship.svg").as_slice());
        ui.ctx().include_bytes("bytes://battleship.svg", include_bytes!("../../../../sow-client/assets/battleship.svg").as_slice());
        ui.ctx().include_bytes("bytes://atombomb.png", include_bytes!("../../../../sow-client/assets/atombomb.png").as_slice());
        ui.ctx().include_bytes("bytes://hydrogenbomb.png", include_bytes!("../../../../sow-client/assets/hydrogenbomb.png").as_slice());
        ui.ctx().include_bytes("bytes://mirv.png", include_bytes!("../../../../sow-client/assets/mirv.png").as_slice());
        ui.ctx().include_bytes("bytes://sam_missile.png", include_bytes!("../../../../sow-client/assets/sam_missile.png").as_slice());
        ui.ctx().include_bytes("bytes://nuke_explosion.png", include_bytes!("../../../../sow-client/assets/nuke_explosion.png").as_slice());
    });

    let mut action = None;
    state.refresh_troop_display_if_due();
    let compact = ui.ctx().content_rect().width() < 768.0;

    let panel_w = if compact { ui.ctx().content_rect().width() } else { 500.0 };

    let panel_margin = if compact { egui::Margin::ZERO } else { egui::Margin::symmetric(0, 8) };
    
    egui::Panel::bottom("hud_bottom_panel")
        .frame(egui::Frame::NONE.inner_margin(panel_margin))
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.set_max_width(panel_w);
                ui.spacing_mut().item_spacing.y = 4.0;
            
                // 1. Attacks Display (top in vertical stack)
                ui.push_id("attacks_display", |ui| {
                    draw_attacks_display(ui, state, panel_w, compact, cancel_intents, &mut action, lang);
                });

                // 1.5 Building Dock (middle in vertical stack)
                if state.spawn_timer_secs.is_none() {
                    ui.push_id("building_dock", |ui| {
                        draw_buildings_dock(ui, state, panel_w, compact);
                    });
                }

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
                            draw_spawn_panel(ui, secs, compact, lang);
                        } else {
                            draw_control_panel(ui, state, compact, &mut action);
                        }
                        
                        // Mobile selection/actions inside the bottom panel
                        if compact {
                            draw_mobile_selection_bar(ui, state, cancel_intents, lang);
                        }

                        // Add mobile safe area space INSIDE the panel to seamlessly extend the background
                        if state.safe_area_bottom > 0.0 {
                            ui.add_space(state.safe_area_bottom);
                        }
                    });
                });
            });
        });

    // ── Top-right HUD buttons ─────────────────────────────────────────────────
    egui::Area::new(egui::Id::new("hud_exit_button"))
        .anchor(Align2::RIGHT_TOP, vec2(-12.0, 12.0 + state.safe_area_top))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            crate::ui::theme::hud_panel_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.add(crate::widgets::HudButton::new("📩")).on_hover_text(&sow_lang::get(lang).hud.inbox_title).clicked() {
                        state.show_alliance_inbox = !state.show_alliance_inbox;
                    }
                    if ui.add(crate::widgets::HudButton::new("⚙")).on_hover_text(&sow_lang::get(lang).hud.hover_settings).clicked() {
                        action = Some(UiAction::ToggleSettings);
                    }
                    if ui.add(crate::widgets::HudButton::new("✖").color(Color32::from_rgb(255, 100, 100))).on_hover_text(&sow_lang::get(lang).hud.hover_exit).clicked() {
                        action = Some(UiAction::LeaveLobby);
                    }
                });
            });
        });

    // ── Floating Alliance Inbox Panel ─────────────────────────────────────────
    if state.show_alliance_inbox {
        egui::Area::new(egui::Id::new("floating_alliance_inbox"))
            .anchor(Align2::RIGHT_TOP, vec2(-12.0, 56.0 + state.safe_area_top))
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                let panel_bg = crate::ui::theme::panel_bg();
                let glow_color = crate::ui::theme::accent_solo_cyan();
                
                egui::Frame::menu(&ui.ctx().global_style())
                    .fill(panel_bg)
                    .stroke(egui::Stroke::new(1.5_f32, glow_color))
                    .corner_radius(12)
                    .inner_margin(16)
                    .show(ui, |ui| {
                        ui.set_max_width(300.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(&sow_lang::get(lang).hud.inbox_title)
                                    .strong()
                                    .color(crate::ui::theme::accent_solo_cyan())
                                    .size(13.0)
                            );
                            ui.add_space(8.0);
                            
                            let my_snapshot = state.players.iter().find(|p| p.id == state.my_player_id);
                            let requests = my_snapshot.map(|p| p.alliance_requests.clone()).unwrap_or_default();
                            
                            if requests.is_empty() {
                                ui.label(RichText::new(&sow_lang::get(lang).hud.inbox_empty).color(Color32::GRAY));
                            } else {
                                for requester_id in requests {
                                    if let Some(requester) = state.players.iter().find(|p| p.id == requester_id) {
                                        let rgb = if requester.player_type == sow_core::player::PlayerType::Human {
                                            sow_core::player::human_shader_territory_rgb(requester.id)
                                        } else {
                                            requester.color
                                        };
                                        let pc = Color32::from_rgb(
                                            (rgb[0] * 255.0) as u8,
                                            (rgb[1] * 255.0) as u8,
                                            (rgb[2] * 255.0) as u8,
                                        );
                                        
                                        ui.horizontal(|ui| {
                                            let icon = if requester.disconnected { "🔌" } else if requester.id < 200 { "⭐" } else { "🐺" };
                                            let name = if requester.name.is_empty() {
                                                if requester.id >= 200 {
                                                    format!("Tribe {}", requester.id - 199)
                                                } else {
                                                    format!("Nation {}", requester.id - 103)
                                                }
                                            } else {
                                                requester.name.clone()
                                            };
                                            
                                            ui.label(RichText::new(icon).color(pc).size(16.0));
                                            ui.vertical(|ui| {
                                                ui.label(RichText::new(name).strong().color(pc));
                                                ui.label(RichText::new(&sow_lang::get(lang).hud.inbox_wants_ally).size(12.0).color(Color32::LIGHT_GRAY));
                                            });
                                        });
                                        
                                        ui.add_space(4.0);
                                        ui.horizontal(|ui| {
                                            let btn_accept = egui::Button::new(RichText::new(&sow_lang::get(lang).hud.btn_accept).color(Color32::from_rgb(74, 222, 128)))
                                                .fill(crate::ui::theme::menu_secondary_button())
                                                .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(74, 222, 128).linear_multiply(0.3)));
                                            if ui.add(btn_accept).clicked() {
                                                cancel_intents.push(sow_core::protocol::GameplayIntent::AcceptAlliance { target_player: requester.id });
                                                state.show_alliance_inbox = false;
                                            }
                                            
                                            let btn_reject = egui::Button::new(RichText::new(&sow_lang::get(lang).hud.btn_reject).color(Color32::from_rgb(239, 68, 68)))
                                                .fill(crate::ui::theme::menu_secondary_button())
                                                .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(239, 68, 68).linear_multiply(0.3)));
                                            if ui.add(btn_reject).clicked() {
                                                cancel_intents.push(sow_core::protocol::GameplayIntent::RejectAlliance { target_player: requester.id });
                                                // Don't close inbox on reject so they can reject multiple
                                            }
                                        });
                                        ui.add_space(8.0);
                                    }
                                }
                            }
                        });
                    });
            });

        // Click outside the inbox panel closes it
        if ui.ctx().input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ui.ctx().input(|i| i.pointer.press_origin().or(i.pointer.interact_pos())) {
                let screen_size = ui.ctx().content_rect();
                let panel_center = pos2(screen_size.right() - 150.0, 150.0 + state.safe_area_top);
                if pos.distance(panel_center) > 250.0 {
                    state.show_alliance_inbox = false;
                }
            }
        }
    }

    // ── Bottom-right Map Controls ──────────────────────────────────────────────
    egui::Area::new(egui::Id::new("hud_map_controls"))
        .anchor(Align2::RIGHT_BOTTOM, vec2(-12.0, -100.0 - state.safe_area_bottom))
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            crate::ui::theme::hud_panel_frame().show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    if ui.add(crate::widgets::HudButton::new("+")).on_hover_text(&sow_lang::get(lang).hud.hover_zoom_in).clicked() {
                        action = Some(UiAction::ZoomIn);
                    }
                    if ui.add(crate::widgets::HudButton::new("-")).on_hover_text(&sow_lang::get(lang).hud.hover_zoom_out).clicked() {
                        action = Some(UiAction::ZoomOut);
                    }
                    ui.add_space(4.0);
                    if ui.add(crate::widgets::HudButton::new("⌖")).on_hover_text(&sow_lang::get(lang).hud.hover_center_camera).clicked() {
                        action = Some(UiAction::CenterCamera);
                    }
                    ui.add_space(4.0);
                    if ui.add(crate::widgets::HudButton::new("😀")).on_hover_text("Express Emoji").clicked() {
                        state.show_emoji_panel = !state.show_emoji_panel;
                        if state.show_emoji_panel {
                            state.emoji_panel_pos = None;
                            state.emoji_panel_just_opened = true;
                        }
                    }
                });
            });
        });

    if state.show_emoji_panel {
        let emojis = &[
            // Row 1: Happy / Expressive
            "😀", "😎", "😏", "😂", "🤣", "😋", "😉", "😜",
            // Row 2: Wholesome / Love
            "😍", "🥰", "🥳", "🥺", "😇", "🤩", "👍", "❤️",
            // Row 3: Surprised / Confused
            "😮", "🤔", "🧐", "🙄", "🤯", "🤡", "💩", "🤫",
            // Row 4: Anger / Battle-ready
            "😠", "😡", "🤬", "😤", "🥵", "🥶", "🤢", "🤮",
            // Row 5: Action / Combat
            "⚔️", "🛡️", "🏹", "💣", "💥", "💀", "👑", "💪",
            // Row 6: Strategy / Status
            "🔥", "👀", "🏳️", "🤝", "💔", "🔌", "⭐", "🐺"
        ];
        
        let mut area = egui::Area::new(egui::Id::new("floating_emoji_panel"))
            .order(egui::Order::Foreground);
            
        if let Some(pos) = state.emoji_panel_pos {
            area = area.pivot(Align2::CENTER_CENTER).fixed_pos(pos);
        } else {
            area = area.anchor(Align2::RIGHT_BOTTOM, vec2(-64.0, -100.0 - state.safe_area_bottom));
        }

        let response = area.show(ui.ctx(), |ui| {
            crate::ui::theme::standard_panel_frame(compact)
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    let cols = 8;
                    let btn_size = 42.0;
                    let emoji_size = 36.0;
                    let spacing = 2.0;
                    let grid_width = cols as f32 * btn_size + (cols as f32 - 1.0) * spacing;

                    ui.vertical(|ui| {
                        ui.set_width(grid_width);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        
                        for chunk in emojis.chunks(cols) {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = spacing;
                                for &emoji in chunk {
                                    let (rect, resp) = ui.allocate_exact_size(vec2(btn_size, btn_size), egui::Sense::click());
                                    
                                    let is_hovered = resp.hovered();
                                    
                                    if is_hovered {
                                        let bg_color = crate::ui::theme::menu_secondary_button_hover().linear_multiply(0.6);
                                        let stroke_color = crate::ui::theme::accent_solo_cyan().linear_multiply(0.8);
                                        ui.painter().rect(
                                            rect,
                                            btn_size / 2.0,
                                            bg_color,
                                            egui::Stroke::new(1.0_f32, stroke_color),
                                            egui::StrokeKind::Inside,
                                        );
                                    }
                                    
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        emoji,
                                        egui::FontId::proportional(emoji_size),
                                        Color32::WHITE,
                                    );
                                    
                                    if resp.clicked() {
                                        let intent = sow_core::protocol::GameplayIntent::ExpressEmoji {
                                            emoji: emoji.to_owned(),
                                            pinned: state.pin_emoji,
                                        };
                                        cancel_intents.push(intent);
                                        if !state.pin_emoji {
                                            state.show_emoji_panel = false;
                                        }
                                    }
                                }
                            });
                            ui.add_space(spacing);
                        }
                        
                        ui.add_space(8.0);
                        
                        ui.horizontal(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    
                                    let text_color = if state.pin_emoji {
                                        crate::ui::theme::accent_ranked_gold()
                                    } else {
                                        crate::ui::theme::text_secondary()
                                    };
                                    
                                    let label_resp = ui.label(
                                        RichText::new("PIN")
                                            .size(13.0)
                                            .strong()
                                            .color(text_color)
                                    );
                                    
                                    let label_click = ui.interact(
                                        label_resp.rect,
                                        ui.make_persistent_id("pin_label_click"),
                                        egui::Sense::click(),
                                    );
                                    if label_click.clicked() {
                                        state.pin_emoji = !state.pin_emoji;
                                    }
                                    
                                    let box_size = 28.0;
                                    let (rect, resp) = ui.allocate_exact_size(vec2(box_size, box_size), egui::Sense::click());
                                    
                                    let is_hovered = resp.hovered();
                                    if resp.clicked() {
                                        state.pin_emoji = !state.pin_emoji;
                                    }
                                    
                                    let bg_color = if state.pin_emoji {
                                        crate::ui::theme::accent_ranked_gold().linear_multiply(0.2)
                                    } else {
                                        crate::ui::theme::nickname_field_bg()
                                    };
                                    
                                    let stroke_color = if state.pin_emoji {
                                        crate::ui::theme::accent_ranked_gold()
                                    } else if is_hovered {
                                        crate::ui::theme::accent_solo_cyan()
                                    } else {
                                        crate::ui::theme::nickname_field_border()
                                    };
                                    
                                    ui.painter().rect(
                                        rect,
                                        4.0,
                                        bg_color,
                                        egui::Stroke::new(2.0_f32, stroke_color),
                                        egui::StrokeKind::Inside,
                                    );
                                    
                                    if state.pin_emoji {
                                        ui.painter().text(
                                            rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            "✓",
                                            egui::FontId::proportional(22.0),
                                            crate::ui::theme::accent_ranked_gold(),
                                        );
                                    }
                                });
                            });
                        });
                    });
                }).response.rect
            });

        let emoji_panel_rect = response.inner;

        // Robust dynamic Rect-based click-outside dismiss
        if !state.emoji_panel_just_opened && ui.ctx().input(|i| i.pointer.any_pressed()) {
            if let Some(pos) = ui.ctx().input(|i| i.pointer.press_origin().or(i.pointer.interact_pos())) {
                let mut is_outside = true;
                
                if emoji_panel_rect.contains(pos) {
                    is_outside = false;
                }
                
                // Allow clicking the HUD bottom bar controls without dismissing if we are in default mode
                if state.emoji_panel_pos.is_none() {
                    let screen_size = ui.ctx().content_rect();
                    let hud_rect = egui::Rect::from_min_max(
                        pos2(screen_size.right() - 510.0, screen_size.bottom() - 90.0 - state.safe_area_bottom),
                        pos2(screen_size.right(), screen_size.bottom()),
                    );
                    if hud_rect.contains(pos) {
                        is_outside = false;
                    }
                }
                
                if is_outside && !state.pin_emoji {
                    state.show_emoji_panel = false;
                }
            }
        }

        state.emoji_panel_just_opened = false;
    }

    draw_sync_overlay(ui.ctx(), state, lang);
    draw_betrayal_overlay(ui.ctx(), state, cancel_intents, lang);
    draw_error_overlay(ui.ctx(), state);

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
    lang: Language,
) {
    let my_pid = state.my_player_id;
    if my_pid == 0 { return; }

    let attack_bg = crate::ui::theme::panel_bg_transparent();

    // Count items without allocating
    let incoming_count = state.attacks.iter().filter(|a| a.target_owner == my_pid).count();
    let outgoing_count = state.attacks.iter().filter(|a| a.owner_id == my_pid).count();
    let fleet_count = state.fleets.iter().filter(|f| f.owner_id == my_pid).count();
    let total = incoming_count + outgoing_count + fleet_count;

    if total == 0 {
        return;
    }

    // Fixed-height container prevents layout reflow when items change
    let cols = 2;
    let rows = total.div_ceil(cols).min(4);
    let row_h = 30.0;
    let fixed_h = rows as f32 * row_h + (rows as f32 - 1.0) * 4.0;
    let cell_w = (width - 16.0 - (cols as f32 - 1.0) * 4.0) / cols as f32;

    let strings = &sow_lang::get(lang).hud;

    egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 0)).show(ui, |ui| {
        egui::ScrollArea::vertical().max_height(fixed_h).stick_to_bottom(true).show(ui, |ui| {
            ui.set_width(width - 16.0);

            let item_count = total;
            let row_count = item_count.div_ceil(cols);

            // Use standard top-down layout, but reverse the row rendering order!
            // This ensures row 0 (oldest attacks) is drawn LAST (at the physical bottom),
            // and new attacks spawn at the physical top, perfectly pushing the container upwards!
            for r in 0..row_count {
                let data_row = row_count - 1 - r;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for col in 0..cols {
                        let idx = data_row * cols + col;
                        if idx >= item_count { continue; }

                        let glow_color = if idx < incoming_count {
                            crate::ui::theme::accent_danger()
                        } else {
                            crate::ui::theme::accent_solo_cyan()
                        };
                        let glow_alpha = 60; // subtle neon stroke
                        let shadow_alpha = 10;

                        egui::Frame::NONE
                            .fill(attack_bg)
                            .stroke(egui::Stroke::new(1.0_f32, glow_color.linear_multiply(glow_alpha as f32 / 255.0)))
                            .shadow(egui::Shadow {
                                blur: 4, // Reduced blur to prevent visual overlap with troop panel
                                spread: 0,
                                color: glow_color.linear_multiply(shadow_alpha as f32 / 255.0),
                                offset: [0, 0],
                            })
                            .corner_radius(6)
                            .inner_margin(egui::Margin::symmetric(4, 2))
                            .show(ui, |ui| {
                                ui.set_width(cell_w);
                                ui.set_height(row_h - 4.0);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if idx < incoming_count {
                                        let attack = state.attacks.iter().filter(|a| a.target_owner == my_pid).nth(idx).unwrap();
                                        if attack.retreating {
                                            ui.label(RichText::new(&strings.retreating_label).size(10.0).color(Color32::GRAY));
                                        } else {
                                            let retaliate = egui::Button::new(RichText::new("⚔").size(12.0))
                                                .fill(crate::ui::theme::accent_danger())
                                                .stroke(egui::Stroke::new(1.0_f32, crate::ui::theme::accent_danger_border()))
                                                .corner_radius(6);
                                            let _ = ui.add_sized(egui::vec2(24.0, 24.0), retaliate).on_hover_text(&strings.hover_retaliate);
                                        }
                                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                            let attacker_name = get_player_display_name(&state.players, attack.owner_id, &strings.default_player_name);
                                            let txt = format!("★↓ {} {}", crate::utils::format_number(attack.troops), attacker_name);
                                            ui.label(RichText::new(txt).size(12.0).color(crate::ui::theme::accent_danger_border()).strong());
                                        });
                                    } else if idx < incoming_count + outgoing_count {
                                        let attack = state.attacks.iter().filter(|a| a.owner_id == my_pid).nth(idx - incoming_count).unwrap();
                                        if attack.retreating {
                                            ui.label(RichText::new(&strings.retreating_label).size(10.0).color(crate::ui::theme::accent_solo_cyan()));
                                        } else {
                                            let cancel_btn = egui::Button::new(RichText::new("❌").size(10.0))
                                                .corner_radius(6);
                                            if ui.add_sized(egui::vec2(24.0, 24.0), cancel_btn).clicked() {
                                                cancel_intents.push(sow_core::protocol::GameplayIntent::CancelAttack { attack_id: attack.id });
                                             }
                                        }
                                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                            let target_name = get_player_display_name(&state.players, attack.target_owner, &strings.wilderness_player_name);
                                            let txt = format!("★↑ {} {}", crate::utils::format_number(attack.troops), target_name);
                                            ui.label(RichText::new(txt).size(12.0).color(crate::ui::theme::accent_solo_cyan()).strong());
                                        });
                                    } else {
                                        let fleet = state.fleets.iter().filter(|f| f.owner_id == my_pid).nth(idx - incoming_count - outgoing_count).unwrap();
                                        if fleet.retreating {
                                            ui.label(RichText::new(&strings.retreating_label).size(10.0).color(crate::ui::theme::accent_solo_cyan()));
                                        } else {
                                            let cancel_btn = egui::Button::new(RichText::new("❌").size(10.0))
                                                .corner_radius(6);
                                            if ui.add_sized(egui::vec2(24.0, 24.0), cancel_btn).clicked() {
                                                cancel_intents.push(sow_core::protocol::GameplayIntent::RecallFleet { fleet_id: fleet.id });
                                            }
                                        }
                                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                            let txt = format!("★↑ {} {}", crate::utils::format_number(fleet.troops), &strings.naval_invasion_label);
                                            ui.label(RichText::new(txt).size(12.0).color(crate::ui::theme::accent_solo_cyan()).strong());
                                        });
                                    }
                                });
                            });
                     }
                 });
             }
         });
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
    let shadow = Color32::BLACK;
    if compact {
        let troop_text = crate::utils::format_number(troops);
        crate::ui::theme::outlined_text(
            ui.painter(),
            pos2(rect.left() + 4.0, rect.center().y),
            Align2::LEFT_CENTER,
            &troop_text,
            egui::FontId::proportional(12.0),
            Color32::from_rgb(220, 230, 220),
            shadow,
        );
        let max_text = crate::utils::format_number(max_troops);
        crate::ui::theme::outlined_text(
            ui.painter(),
            pos2(rect.right() - 4.0, rect.center().y),
            Align2::RIGHT_CENTER,
            &max_text,
            egui::FontId::proportional(12.0),
            Color32::from_rgb(220, 230, 220),
            shadow,
        );
        let rate_color = if is_increasing { crate::ui::theme::accent_solo_cyan_hover() } else { crate::ui::theme::accent_danger() };
        let rate_text = format!("★ +{}/s", crate::utils::format_number(troop_rate));
        crate::ui::theme::outlined_text(
            ui.painter(),
            rect.center(),
            Align2::CENTER_CENTER,
            &rate_text,
            egui::FontId::proportional(10.0),
            rate_color,
            shadow,
        );
    } else {
        let text = format!("{} / {} ★", crate::utils::format_number(troops), crate::utils::format_number(max_troops));
        crate::ui::theme::outlined_text(
            ui.painter(),
            rect.center(),
            Align2::CENTER_CENTER,
            &text,
            egui::FontId::proportional(14.0),
            Color32::from_rgb(220, 230, 220),
            shadow,
        );
    }
}

fn draw_spawn_panel(ui: &mut egui::Ui, secs: f32, compact: bool, lang: Language) {
    ui.vertical_centered(|ui| {
        crate::ui::theme::outlined_label(
            ui,
            &sow_lang::get(lang).hud.spawn_choose_location,
            egui::FontId::proportional(if compact { 16.0 } else { 20.0 }),
            crate::ui::theme::accent_ranked_gold_hover()
        );
        ui.label(RichText::new(format!("{:.1}{}", secs, &sow_lang::get(lang).hud.spawn_seconds_remaining)).size(14.0).color(Color32::from_rgb(220, 230, 220)));
    });
}

fn draw_sync_overlay(ctx: &Context, state: &HudState, lang: Language) {
    if let Some(sync) = &state.sync_state {
        let strings = &sow_lang::get(lang).hud;
        let screen_rect = ctx.content_rect();
        ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("sync_overlay")))
            .rect_filled(screen_rect, 0.0, Color32::from_black_alpha(180));

        egui::Window::new(&strings.overlay_waiting_players)
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    if sync.is_starting {
                        crate::ui::theme::outlined_label(
                            ui,
                            &strings.overlay_all_ready,
                            egui::FontId::proportional(24.0),
                            Color32::GREEN
                        );
                        ui.label(RichText::new(&strings.overlay_stabilizing).size(16.0).color(Color32::LIGHT_GRAY));
                    } else {
                        crate::ui::theme::outlined_label(
                            ui,
                            &strings.overlay_waiting_players,
                            egui::FontId::proportional(24.0),
                            Color32::WHITE
                        );
                        ui.label(RichText::new(format!("{}{:.1}{}", &strings.overlay_starting_in, sync.time_remaining, &strings.overlay_seconds_short)).size(18.0).color(Color32::YELLOW));
                    }

                    ui.add_space(20.0);
                    let total = sync.players.len();
                    let ready = sync.players.iter().filter(|p| p.is_ready).count();
                    let ratio = if total == 0 { 0.0 } else { ready as f32 / total as f32 };
                    ui.add(egui::ProgressBar::new(ratio).text(format!("{}/{} {}", ready, total, &strings.overlay_players_ready)));

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

fn draw_betrayal_overlay(ctx: &Context, state: &mut HudState, cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>, _lang: Language) {
    if let Some((ally_id, intent)) = state.show_betrayal_warning.clone() {
        let screen_rect = ctx.content_rect();
        ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("betrayal_overlay_bg")))
            .rect_filled(screen_rect, 0.0, Color32::from_black_alpha(180));

        egui::Window::new("betrayal_warning_modal")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, -20.0))
            .frame(egui::Frame::window(&ctx.global_style())
                .fill(crate::ui::theme::panel_bg())
                .stroke(egui::Stroke::new(2.0f32, crate::ui::theme::accent_danger()))
                .inner_margin(24.0)
                .corner_radius(12)
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    let ally_name = get_player_display_name(&state.players, ally_id, "Ally");
                    
                    crate::ui::theme::outlined_label(
                        ui,
                        "BETRAYAL WARNING",
                        egui::FontId::proportional(28.0),
                        crate::ui::theme::accent_danger()
                    );
                    
                    ui.add_space(12.0);
                    
                    ui.label(RichText::new(format!("If you attack {}, other allies could attack you.", ally_name))
                        .size(16.0)
                        .color(Color32::WHITE)
                    );
                    
                    ui.label(RichText::new("Are you sure?")
                        .size(18.0)
                        .strong()
                        .color(crate::ui::theme::accent_ranked_gold())
                    );

                    ui.add_space(24.0);

                    ui.horizontal(|ui| {
                        // NO button (safe)
                        let no_btn = egui::Button::new(RichText::new("NO, KEEP ALLIANCE").size(16.0))
                            .fill(crate::ui::theme::menu_secondary_button())
                            .corner_radius(8);
                        
                        if ui.add_sized(vec2(160.0, 44.0), no_btn).clicked() {
                            state.show_betrayal_warning = None;
                        }

                        ui.add_space(16.0);

                        // YES button (danger)
                        let yes_btn = egui::Button::new(RichText::new("YES, BETRAY").size(16.0).strong())
                            .fill(crate::ui::theme::accent_danger().linear_multiply(0.3))
                            .stroke(egui::Stroke::new(1.5f32, crate::ui::theme::accent_danger()))
                            .corner_radius(8);
                        
                        if ui.add_sized(vec2(140.0, 44.0), yes_btn).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            // 1. Send BreakAlliance intent
                            cancel_intents.push(sow_core::protocol::GameplayIntent::BreakAlliance { target_player: ally_id });
                            
                            // 2. Send the original Attack intent right after
                            cancel_intents.push(intent);
                            
                            state.show_betrayal_warning = None;
                        }
                    });
                });
            });
    }
}

fn draw_mobile_selection_bar(
    ui: &mut egui::Ui,
    state: &HudState,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
) {
    if let Some(tile_info) = &state.selected_tile {
        use crate::ui::theme::palette;
        use egui::RichText;
        let strings = &sow_lang::get(lang).hud;

        if tile_info.is_spawning {
            return;
        }

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let text_color = if tile_info.is_own_territory {
                    palette::neon_gold()
                } else if tile_info.is_friendly {
                    palette::neon_cyan()
                } else {
                    palette::danger()
                };
                let status_text = if tile_info.is_own_territory {
                    &strings.status_own
                } else if tile_info.is_friendly {
                    &strings.status_ally
                } else if tile_info.owner_id != 0 {
                    &strings.status_enemy
                } else {
                    &strings.status_neutral
                };
                ui.label(
                    RichText::new(format!("{}{}-{}", &strings.status_tile_prefix, tile_info.tile_idx, status_text))
                        .strong()
                        .size(11.0)
                        .color(text_color)
                );
            });

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                
                let btn_w = (ui.available_width() - 12.0) / 4.0;
                
                // 1. Info Button
                let info_btn = egui::Button::new(RichText::new(&strings.btn_info).strong().size(12.0))
                    .fill(palette::button_inactive())
                    .stroke(egui::Stroke::new(1.0_f32, palette::text_muted()))
                    .corner_radius(6);
                let _ = ui.add_sized(egui::vec2(btn_w, 32.0), info_btn);

                // 2. Fleet / Delete Button
                let right_fill = if tile_info.is_own_territory { palette::danger() } else { palette::neon_cyan() };
                let right_glow = if tile_info.is_own_territory { palette::danger_border() } else { palette::neon_cyan_hover() };
                let right_label = if tile_info.is_own_territory { &strings.btn_delete } else { &strings.btn_fleft };

                let fleet_btn = egui::Button::new(RichText::new(right_label).strong().size(12.0))
                    .fill(right_fill.linear_multiply(0.3))
                    .stroke(egui::Stroke::new(1.2_f32, right_glow))
                    .corner_radius(6);

                if ui.add_sized(egui::vec2(btn_w, 32.0), fleet_btn).clicked() {
                    let troops = Some(state.troops * (state.attack_ratio as f64));
                    cancel_intents.push(sow_core::protocol::GameplayIntent::LaunchFleet {
                        target_tile: tile_info.tile_idx,
                        troops,
                    });
                }

                // 3. Ally Button
                let ally_btn = egui::Button::new(RichText::new(&strings.btn_ally).strong().size(12.0))
                    .fill(palette::button_inactive())
                    .stroke(egui::Stroke::new(1.0_f32, palette::neon_cyan()))
                    .corner_radius(6);
                let _ = ui.add_sized(egui::vec2(btn_w, 32.0), ally_btn);

                // 4. Build / Attack Button
                let left_fill = if tile_info.is_own_territory { palette::neon_gold() } else { palette::danger() };
                let left_glow = if tile_info.is_own_territory { palette::neon_gold_hover() } else { palette::danger_border() };
                let left_label = if tile_info.is_own_territory { &strings.btn_build } else { &strings.btn_attack };

                let action_btn = egui::Button::new(RichText::new(left_label).strong().size(12.0))
                    .fill(left_fill.linear_multiply(0.3))
                    .stroke(egui::Stroke::new(1.2_f32, left_glow))
                    .corner_radius(6);

                if ui.add_sized(egui::vec2(btn_w, 32.0), action_btn).clicked() {
                    if !tile_info.is_own_territory {
                        let troops = state.troops * (state.attack_ratio as f64);
                        if troops > 0.0 {
                            let attack = sow_core::protocol::AttackIntent {
                                target_owner: tile_info.owner_id,
                                troops: Some(troops),
                            };
                            cancel_intents.push(sow_core::protocol::GameplayIntent::Attack(attack));
                        }
                    }
                }
            });
            ui.add_space(4.0);
        });
    }
}

fn draw_error_overlay(ctx: &Context, state: &mut HudState) {
    if let Some(err_msg) = state.show_error.clone() {
        let now = Instant::now();
        let display_duration = Duration::from_millis(2500);

        let reset = match &state.last_error_message {
            Some(last) if last == &err_msg => false,
            _ => true,
        };

        if reset {
            state.last_error_message = Some(err_msg.clone());
            state.error_display_timer = Some(now);
        }

        let start_time = state.error_display_timer.unwrap_or(now);
        let elapsed = now.duration_since(start_time);

        if elapsed >= display_duration {
            state.show_error = None;
            state.last_error_message = None;
            state.error_display_timer = None;
            return;
        }

        let remaining = display_duration.as_secs_f32() - elapsed.as_secs_f32();
        let alpha = if remaining < 0.5 {
            (remaining / 0.5).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let bg_color = Color32::from_rgba_unmultiplied(15, 23, 42, (180.0 * alpha) as u8);
        let border_color = crate::ui::theme::accent_danger().linear_multiply(alpha);
        let text_color = Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8);

        egui::Area::new(egui::Id::new("error_toast_area"))
            .anchor(egui::Align2::CENTER_TOP, vec2(0.0, 80.0 + state.safe_area_top))
            .order(egui::Order::Tooltip)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(1.0_f32, border_color))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(16, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("⚠️")
                                    .color(border_color)
                                    .size(12.0)
                            );
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(err_msg)
                                    .color(text_color)
                                    .size(12.0)
                                    .strong()
                            );
                        });
                    });
            });

        // Request repaint so the fade-out animation and auto-dismiss run smoothly
        ctx.request_repaint();
    }
}
