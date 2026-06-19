use crate::app::SowApp;
use egui::Color32;

impl SowApp {
    pub(super) fn draw_build_popover(
        &mut self, _ui: &mut egui::Ui, ctx: &egui::Context, tile_idx: u32,
        center: egui::Pos2, scale: f32, compact: bool, screen: egui::Rect,
        outer_r: f32, col: u32, row: u32, is_own_territory: bool, radial_build_active: bool,
        build_active_id: egui::Id,
    ) {
        // Render Build sub-popover
        if radial_build_active && is_own_territory {
            let mut area = egui::Area::new(egui::Id::new("radial_build_popover"))
                .order(egui::Order::Tooltip);

            if compact {
                area = area.fixed_pos(screen.center()).pivot(egui::Align2::CENTER_CENTER);
            } else {
                area = area.fixed_pos(center - egui::vec2(outer_r + 240.0, 150.0));
            }

            let theme_color = Color32::from_rgb(34, 211, 238); // Cyan

            area.show(ctx, |ui| {
                let response_rect = ui.min_rect();
                ctx.data_mut(|d| d.insert_temp(egui::Id::new("build_popover_rect"), response_rect));

                egui::Frame::window(&ctx.global_style())
                    .fill(sow_ui::ui::theme::panel_bg())
                    .stroke(egui::Stroke::new(1.8_f32, theme_color))
                    .corner_radius(16)
                    .inner_margin(if compact { 16 } else { 12 })
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            // Header
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new("CONSTRUCT")
                                        .strong()
                                        .color(theme_color)
                                        .size(13.0)
                                );
                            });
                            ui.add_space(8.0);

                            let card_w = if compact { 280.0 } else { 220.0 };
                            let card_h = 50.0;
                            let city_opt = self.sim.current_snapshot.as_ref().and_then(|s| {
                                s.buildings.iter().find(|b| b.tile_idx == tile_idx && b.kind == sow_core::game::BuildingKind::City)
                                    .map(|b| (b.id, b.modules, b.under_construction))
                            });

                            if let Some((city_id, city_modules, city_under_construction)) = city_opt {
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        egui::RichText::new("CITY DISTRICTS")
                                            .strong()
                                            .color(theme_color)
                                            .size(13.0)
                                    );
                                });
                                ui.add_space(8.0);

                                let card_w = if compact { 280.0 } else { 220.0 };
                                let card_h = 50.0;

                                // Draw City Module upgrades (Silo/Arsenal, Port, Foundry)
                                let modules_list = [
                                    (sow_core::building::ModuleKind::Arsenal, "Silo District", "🚀"),
                                    (sow_core::building::ModuleKind::Port, "Port District", "⚓"),
                                    (sow_core::building::ModuleKind::Foundry, "Foundry District", "🏭"),
                                ];

                                for &(mod_kind, mod_name, icon) in &modules_list {
                                    let current_lvl = city_modules.get_level(mod_kind);
                                    let cost = sow_core::building::cost::module_upgrade_cost_gold(mod_kind, current_lvl + 1);
                                    let is_disabled = self.ui.app.hud_state.gold < cost;

                                    let label = if current_lvl == 0 {
                                        format!("Build {}", mod_name)
                                    } else {
                                        format!("Upgrade {}", mod_name)
                                    };

                                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                    let is_hovered = resp.hovered() && !is_disabled;
                                    let hover_id = ui.make_persistent_id(("popover_hover_mod", mod_name));
                                    let hover_t = ui.ctx().animate_bool_with_time(hover_id, is_hovered, 0.15);

                                    let border_glow = theme_color.linear_multiply(0.3 + 0.7 * hover_t);
                                    let bg_fill = if is_disabled {
                                        Color32::from_rgba_unmultiplied(20, 20, 20, 180)
                                    } else if is_hovered {
                                        theme_color.linear_multiply(0.12)
                                    } else {
                                        Color32::from_rgba_unmultiplied(10, 15, 30, 220)
                                    };

                                    ui.painter().rect(
                                        rect,
                                        8.0,
                                        bg_fill,
                                        egui::Stroke::new(1.0_f32 + hover_t * 1.0_f32, border_glow),
                                        egui::StrokeKind::Inside,
                                    );

                                    // Icon
                                    ui.painter().text(
                                        rect.min + egui::vec2(20.0, card_h / 2.0),
                                        egui::Align2::CENTER_CENTER,
                                        icon,
                                        egui::FontId::proportional((22.0 + 4.0 * hover_t) * scale),
                                        if is_disabled { Color32::GRAY } else { Color32::WHITE }
                                    );

                                    // Label
                                    ui.painter().text(
                                        rect.min + egui::vec2(44.0, card_h / 2.0 - 8.0),
                                        egui::Align2::LEFT_CENTER,
                                        label,
                                        egui::FontId::proportional(13.0),
                                        if is_disabled { Color32::GRAY } else { Color32::WHITE }
                                    );

                                    // Cost & Level info
                                    ui.painter().text(
                                        rect.min + egui::vec2(44.0, card_h / 2.0 + 8.0),
                                        egui::Align2::LEFT_CENTER,
                                        format!("Lvl {} -> {} | {}g", current_lvl, current_lvl + 1, cost as u32),
                                        egui::FontId::proportional(10.5),
                                        if is_disabled { Color32::from_rgb(180, 100, 100) } else { Color32::from_rgb(251, 191, 36) }
                                    );

                                    if !is_disabled && resp.clicked() {
                                        self.send_intent(sow_core::protocol::GameplayIntent::UpgradeCityModule {
                                            building_id: city_id,
                                            module: mod_kind,
                                        });
                                        ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                        self.input.map_context_menu = None;
                                    }
                                    ui.add_space(4.0);
                                }

                                // If Port module is completed, also show Shipyard options!
                                if city_modules.port > 0 && !city_under_construction {
                                    ui.add_space(6.0);
                                    ui.separator();
                                    ui.add_space(6.0);

                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            egui::RichText::new("SHIPYARD")
                                                .strong()
                                                .color(theme_color)
                                                .size(13.0)
                                        );
                                    });
                                    ui.add_space(8.0);

                                    let ships = [
                                        (sow_core::game::UnitType::Warship, "Warship", 100_000.0, "🚢"),
                                        (sow_core::game::UnitType::TradeShip, "Trade Ship", 10_000.0, "⛴️"),
                                    ];

                                    for &(kind, label, cost, icon) in &ships {
                                        let is_disabled = self.ui.app.hud_state.gold < cost;

                                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                        let is_hovered = resp.hovered() && !is_disabled;
                                        let hover_id = ui.make_persistent_id(("popover_hover", label));
                                        let hover_t = ui.ctx().animate_bool_with_time(hover_id, is_hovered, 0.15);

                                        let border_glow = theme_color.linear_multiply(0.3 + 0.7 * hover_t);
                                        let bg_fill = if is_disabled {
                                            Color32::from_rgba_unmultiplied(20, 20, 20, 180)
                                        } else if is_hovered {
                                            theme_color.linear_multiply(0.12)
                                        } else {
                                            Color32::from_rgba_unmultiplied(10, 15, 30, 220)
                                        };

                                        ui.painter().rect(
                                            rect,
                                            8.0,
                                            bg_fill,
                                            egui::Stroke::new(1.0_f32 + hover_t * 1.0_f32, border_glow),
                                            egui::StrokeKind::Inside,
                                        );

                                        // Icon
                                        ui.painter().text(
                                            rect.min + egui::vec2(20.0, card_h / 2.0),
                                            egui::Align2::CENTER_CENTER,
                                            icon,
                                            egui::FontId::proportional((22.0 + 4.0 * hover_t) * scale),
                                            if is_disabled { Color32::GRAY } else { Color32::WHITE }
                                        );

                                        // Label
                                        ui.painter().text(
                                            rect.min + egui::vec2(44.0, card_h / 2.0 - 8.0),
                                            egui::Align2::LEFT_CENTER,
                                            label,
                                            egui::FontId::proportional(13.0),
                                            if is_disabled { Color32::GRAY } else { Color32::WHITE }
                                        );

                                        // Cost
                                        ui.painter().text(
                                            rect.min + egui::vec2(44.0, card_h / 2.0 + 8.0),
                                            egui::Align2::LEFT_CENTER,
                                            format!("{}g", cost as u32),
                                            egui::FontId::proportional(10.5),
                                            if is_disabled { Color32::from_rgb(180, 100, 100) } else { Color32::from_rgb(251, 191, 36) }
                                        );

                                        if !is_disabled && resp.clicked() {
                                            self.send_intent(sow_core::protocol::GameplayIntent::BuildShip {
                                                port_tile: tile_idx,
                                                kind,
                                            });
                                            ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                            self.input.map_context_menu = None;
                                        }
                                        ui.add_space(4.0);
                                    }
                                }
                            } else {
                                let current_level = if (tile_idx as usize) < self.sim.tile_upgrades.len() {
                                    self.sim.tile_upgrades[tile_idx as usize]
                                } else {
                                    0
                                };

                                let tile_byte = self.gfx.map_renderer.as_ref()
                                    .and_then(|mr| mr.terrain.get(tile_idx as usize).copied())
                                    .unwrap_or(0b10000000);
                                let map_tile = sow_core::map::MapTile::from_byte(tile_byte);

                                // Procedural resource extraction identical to map.rs
                                let magnitude = map_tile.magnitude();
                                let seed = (col as u64).wrapping_mul(374761393)
                                    .wrapping_add((row as u64).wrapping_mul(668265263))
                                    .wrapping_add(magnitude as u64);
                                let hash = (seed ^ (seed >> 13)).wrapping_mul(1274126177) % 100;

                                let resource = if !map_tile.is_land() {
                                    sow_core::map::TileResource::None
                                } else if magnitude >= 20 {
                                    match hash % 5 {
                                        0 => sow_core::map::TileResource::Copper,
                                        1 => sow_core::map::TileResource::Stone,
                                        2 => sow_core::map::TileResource::Iron,
                                        3 => sow_core::map::TileResource::Diamonds,
                                        _ => sow_core::map::TileResource::None,
                                    }
                                } else if magnitude >= 10 {
                                    match hash % 8 {
                                        0 => sow_core::map::TileResource::Wheat,
                                        1 => sow_core::map::TileResource::Stone,
                                        2 => sow_core::map::TileResource::Copper,
                                        3 => sow_core::map::TileResource::Iron,
                                        4 => sow_core::map::TileResource::Jade,
                                        _ => sow_core::map::TileResource::None,
                                    }
                                } else {
                                    match hash % 10 {
                                        0 => sow_core::map::TileResource::Corn,
                                        1 => sow_core::map::TileResource::Rice,
                                        2 => sow_core::map::TileResource::Wheat,
                                        3 => sow_core::map::TileResource::Jade,
                                        4 => sow_core::map::TileResource::Salt,
                                        _ => sow_core::map::TileResource::None,
                                    }
                                };

                                let (upgrade_label, upgrade_icon) = match resource {
                                    sow_core::map::TileResource::Corn => ("Upgrade Farm (Corn)", "🌽"),
                                    sow_core::map::TileResource::Rice => ("Upgrade Farm (Rice)", "🌾"),
                                    sow_core::map::TileResource::Wheat => ("Upgrade Farm (Wheat)", "🍞"),
                                    sow_core::map::TileResource::Copper => ("Upgrade Mine (Copper)", "🪙"),
                                    sow_core::map::TileResource::Stone => ("Upgrade Quarry (Stone)", "🪨"),
                                    sow_core::map::TileResource::Iron => ("Upgrade Mine (Iron)", "⛓️"),
                                    sow_core::map::TileResource::Jade => ("Upgrade Jade Opp.", "🟢"),
                                    sow_core::map::TileResource::Diamonds => ("Upgrade Diamond Opp.", "💎"),
                                    sow_core::map::TileResource::Salt => ("Upgrade Salt Opp.", "🧂"),
                                    sow_core::map::TileResource::None => {
                                        match map_tile.terrain_type() {
                                            sow_core::map::TerrainType::Land => ("Upgrade Flatland Farm", "🌾"),
                                            sow_core::map::TerrainType::Highland | sow_core::map::TerrainType::Mountain => ("Upgrade Highland Mine", "🪨"),
                                            sow_core::map::TerrainType::Water | sow_core::map::TerrainType::Lake => ("Upgrade Water Opp.", "🐠"),
                                        }
                                    }
                                };

                                let s = sow_core::config::GOLD_SCALE.max(1.0);
                                let upgrade_cost = (1000.0 * 1.5f64.powi(current_level as i32)) / s;

                                // Render Upgrade Card
                                let is_upgrade_disabled = self.ui.app.hud_state.gold < upgrade_cost;
                                let (upgrade_rect, upgrade_resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                let is_upgrade_hovered = upgrade_resp.hovered() && !is_upgrade_disabled;
                                let upgrade_hover_id = ui.make_persistent_id(("popover_hover", "upgrade_tile"));
                                let upgrade_hover_t = ui.ctx().animate_bool_with_time(upgrade_hover_id, is_upgrade_hovered, 0.15);

                                let upgrade_border_glow = theme_color.linear_multiply(0.3 + 0.7 * upgrade_hover_t);
                                let upgrade_bg_fill = if is_upgrade_disabled {
                                    Color32::from_rgba_unmultiplied(20, 20, 20, 180)
                                } else if is_upgrade_hovered {
                                    theme_color.linear_multiply(0.12)
                                } else {
                                    Color32::from_rgba_unmultiplied(10, 15, 30, 220)
                                };

                                ui.painter().rect(
                                    upgrade_rect,
                                    8.0,
                                    upgrade_bg_fill,
                                    egui::Stroke::new(1.0_f32 + upgrade_hover_t * 1.0_f32, upgrade_border_glow),
                                    egui::StrokeKind::Inside,
                                );

                                ui.painter().text(
                                    upgrade_rect.min + egui::vec2(20.0, card_h / 2.0),
                                    egui::Align2::CENTER_CENTER,
                                    upgrade_icon,
                                    egui::FontId::proportional((22.0 + 4.0 * upgrade_hover_t) * scale),
                                    if is_upgrade_disabled { Color32::GRAY } else { Color32::WHITE }
                                );

                                ui.painter().text(
                                    upgrade_rect.min + egui::vec2(44.0, card_h / 2.0 - 8.0),
                                    egui::Align2::LEFT_CENTER,
                                    upgrade_label,
                                    egui::FontId::proportional(13.0),
                                    if is_upgrade_disabled { Color32::GRAY } else { Color32::WHITE }
                                );

                                ui.painter().text(
                                    upgrade_rect.min + egui::vec2(44.0, card_h / 2.0 + 8.0),
                                    egui::Align2::LEFT_CENTER,
                                    format!("Lvl {} -> {} | {}g", current_level, current_level + 1, upgrade_cost as u32),
                                    egui::FontId::proportional(10.5),
                                    if is_upgrade_disabled { Color32::from_rgb(180, 100, 100) } else { Color32::from_rgb(251, 191, 36) }
                                );

                                if !is_upgrade_disabled && upgrade_resp.clicked() {
                                    self.send_intent(sow_core::protocol::GameplayIntent::UpgradeTile { tile_idx });
                                    ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                    self.input.map_context_menu = None;
                                }

                                ui.add_space(6.0);
                                ui.separator();
                                ui.add_space(4.0);

                                let buildings_list = [
                                    (
                                        sow_core::game::BuildingKind::City,
                                        "City Center",
                                        "Core of your empire. Increases troop generation, gold generation, and max troops. Can be upgraded with 6 powerful modules (Port, Foundry, Armory, Intel, Arsenal, Shield)!",
                                    ),
                                    (
                                        sow_core::game::BuildingKind::Factory,
                                        "Industrial Factory",
                                        "Economic Engine: A specialized pure gold generator. Upgradable up to Level 5 to progressively multiply gold income. Must be spaced from other structures.",
                                    ),
                                    (
                                        sow_core::game::BuildingKind::Port,
                                        "Maritime Port",
                                        "Maritime Port: Specialized coastal harbor. Generates gold and troop income and enables launching naval fleets. Must be built near the shore.",
                                    ),
                                    (
                                        sow_core::game::BuildingKind::Bunker,
                                        "Defense Tower",
                                        "Frontline Anchor: Fortifies borders, slowing enemy land grabs. Naturally strong on mountains (3x) and highlands (2x), upgradable with gold!",
                                    ),
                                ];

                                for &(kind, label, desc) in &buildings_list {
                                    let my_player_id = self.sim.my_player_id.unwrap_or(1);
                                    let count = self.sim.current_snapshot.as_ref()
                                        .map(|s| s.buildings.iter().filter(|b| b.owner_id == my_player_id && b.kind == kind).count() as u32)
                                        .unwrap_or(0);
                                    let cost = sow_core::building::structure_build_cost_gold(kind, count, &self.sim.config);
                                    let is_disabled = self.ui.app.hud_state.gold < cost;

                                    let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());
                                    let is_hovered = resp.hovered() && !is_disabled;
                                    let hover_id = ui.make_persistent_id(("popover_hover", label));
                                    let hover_t = ui.ctx().animate_bool_with_time(hover_id, is_hovered, 0.15);

                                    let border_glow = theme_color.linear_multiply(0.3 + 0.7 * hover_t);
                                    let bg_fill = if is_disabled {
                                        Color32::from_rgba_unmultiplied(20, 20, 20, 180)
                                    } else if is_hovered {
                                        theme_color.linear_multiply(0.12)
                                    } else {
                                        Color32::from_rgba_unmultiplied(10, 15, 30, 220)
                                    };

                                    ui.painter().rect(
                                        rect,
                                        8.0,
                                        bg_fill,
                                        egui::Stroke::new(1.0_f32 + hover_t * 1.0_f32, border_glow),
                                        egui::StrokeKind::Inside,
                                    );

                                    // Icon (Premium building image asset)
                                    let icon_size = 24.0 * scale;
                                    let icon_rect = egui::Rect::from_center_size(
                                        rect.min + egui::vec2(20.0, card_h / 2.0),
                                        egui::vec2(icon_size, icon_size),
                                    );
                                    let tint = if is_disabled { Color32::GRAY } else { Color32::WHITE };
                                    let emoji = match kind {
                                        sow_core::game::BuildingKind::City => "🏛️",
                                        sow_core::game::BuildingKind::Factory => "🏭",
                                        sow_core::game::BuildingKind::Port => "⚓",
                                        sow_core::game::BuildingKind::Bunker => "🛡️",
                                    };
                                    if !sow_ui::widgets::try_paint_emoji(ui.painter(), emoji, icon_rect, tint) {
                                        ui.painter().text(
                                            icon_rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            emoji,
                                            egui::FontId::proportional(icon_size * 0.7),
                                            tint,
                                        );
                                    }

                                    // Label
                                    ui.painter().text(
                                        rect.min + egui::vec2(44.0, card_h / 2.0 - 8.0),
                                        egui::Align2::LEFT_CENTER,
                                        label,
                                        egui::FontId::proportional(13.0),
                                        if is_disabled { Color32::GRAY } else { Color32::WHITE }
                                    );

                                    // Cost
                                    let cost_text = if cost.is_infinite() { "N/A".to_string() } else { format!("{}", cost as u32) };
                                    let cost_color = if is_disabled {
                                        Color32::from_rgb(180, 100, 100)
                                    } else {
                                        Color32::from_rgb(251, 191, 36)
                                    };
                                    let cost_label = if cost_text == "N/A" {
                                        cost_text.clone()
                                    } else {
                                        format!("🪙 {cost_text}")
                                    };
                                    sow_ui::widgets::paint_emoji_text_at(
                                        ui.painter(),
                                        rect.min + egui::vec2(44.0, card_h / 2.0 + 8.0),
                                        egui::Align2::LEFT_CENTER,
                                        &cost_label,
                                        egui::FontId::proportional(10.5),
                                        cost_color,
                                        false,
                                    );

                                    resp = resp.on_hover_ui(|ui| {
                                        ui.label(egui::RichText::new(label).strong().size(14.0).color(theme_color));
                                        ui.add_space(4.0);
                                        ui.label(egui::RichText::new(desc).size(12.0).color(egui::Color32::LIGHT_GRAY));
                                        ui.add_space(6.0);
                                        let cost_color = if !is_disabled { egui::Color32::from_rgb(74, 222, 128) } else { egui::Color32::from_rgb(239, 68, 68) };
                                        sow_ui::widgets::emoji_label(
                                            ui,
                                            &format!("Cost: 🪙 {cost_text} Gold"),
                                            egui::FontId::proportional(13.0),
                                            cost_color,
                                        );
                                    });

                                    if !is_disabled && resp.clicked() {
                                        self.send_intent(sow_core::protocol::GameplayIntent::BuildStructure {
                                            kind,
                                            target_tile: tile_idx,
                                        });
                                        ctx.data_mut(|d| d.insert_temp(build_active_id, false));
                                        self.input.map_context_menu = None;
                                    }
                                    ui.add_space(4.0);
                                }
                            }
                        });
                    });
            });
        }

    }
}
