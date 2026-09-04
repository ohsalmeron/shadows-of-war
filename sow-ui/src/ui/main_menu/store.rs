use super::MainMenuState;
use crate::UiAction;
use crate::kit::components::{BodyText, Button, Card, Heading, Subtitle};
use crate::ui::asset_loader::AssetLoader;
use egui::{Color32, CornerRadius, Frame, Layout, Margin, RichText, Stroke, Ui, Vec2};
use sow_core::player::Leader;
use sow_data::commerce::{LeaderOffer, SkinOffer};
use sow_ui_kit::theme::palette;

fn leader_from_offer(offer: &LeaderOffer) -> Option<Leader> {
    sow_data::commerce::leader_from_id(&offer.id)
}

fn draw_leader_card(
    ui: &mut Ui,
    offer: &LeaderOffer,
    asset_loader: &AssetLoader,
    action: &mut Option<UiAction>,
    busy: bool,
) {
    let Some(leader) = leader_from_offer(offer) else {
        return;
    };
    let accent = if offer.owned {
        palette::neon_cyan()
    } else if offer.free_rotation {
        palette::neon_gold()
    } else {
        palette::field_border()
    };

    Frame::NONE
        .fill(Color32::from_rgba_unmultiplied(8, 10, 16, 220))
        .stroke(Stroke::new(1.0_f32, accent))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            let art_size = Vec2::new(ui.available_width(), 132.0);
            let (art_rect, _) = ui.allocate_exact_size(art_size, egui::Sense::hover());
            ui.painter().rect_filled(
                art_rect,
                8.0,
                Color32::from_rgb(24, 28, 38),
            );
            if let Some(texture) = asset_loader.leader_portrait_texture(leader, false) {
                ui.put(
                    art_rect,
                    egui::Image::new(texture)
                        .fit_to_exact_size(art_rect.size())
                        .corner_radius(CornerRadius::same(8)),
                );
            } else {
                ui.painter().text(
                    art_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    leader.name(),
                    egui::FontId::proportional(18.0),
                    palette::text_muted(),
                );
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(offer.name.to_uppercase()).strong().size(17.0));
                    ui.label(
                        RichText::new(offer.civilization.to_uppercase())
                            .size(11.0)
                            .color(palette::text_muted()),
                    );
                });
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    let status = if offer.owned {
                        "OWNED"
                    } else if offer.free_rotation {
                        "FREE THIS WEEK"
                    } else {
                        "LOCKED"
                    };
                    ui.label(RichText::new(status).size(10.0).strong().color(accent));
                });
            });
            ui.add_space(6.0);
            ui.label(RichText::new(&offer.perk).size(12.0).color(palette::text_muted()));
            ui.add_space(10.0);
            if !offer.owned && !offer.free_rotation {
                ui.horizontal_wrapped(|ui| {
                    if Button::secondary(&format!("{} LAURELS", offer.cost_laurels))
                        .disabled(busy)
                        .small()
                        .min_size(egui::vec2(0.0, 44.0))
                        .show(ui)
                        .clicked()
                    {
                        *action = Some(UiAction::UnlockLeader {
                            leader_id: offer.id.clone(),
                            currency: "laurels".to_string(),
                        });
                    }
                    if Button::primary(&format!("{} GEMS", offer.cost_gems))
                        .disabled(busy)
                        .small()
                        .min_size(egui::vec2(0.0, 44.0))
                        .show(ui)
                        .clicked()
                    {
                        *action = Some(UiAction::UnlockLeader {
                            leader_id: offer.id.clone(),
                            currency: "gems".to_string(),
                        });
                    }
                });
            }
        });
}

fn skin_color(style: u8) -> Color32 {
    match style {
        1 => Color32::from_rgb(191, 72, 45),
        2 => Color32::from_rgb(45, 125, 191),
        3 => Color32::from_rgb(160, 104, 45),
        _ => palette::surface(),
    }
}

fn draw_skin_card(
    ui: &mut Ui,
    skin: &SkinOffer,
    selected_skin: Option<&str>,
    action: &mut Option<UiAction>,
    busy: bool,
) {
    let equipped = selected_skin == Some(skin.id.as_str());
    let accent = if equipped {
        palette::neon_cyan()
    } else {
        palette::field_border()
    };
    Frame::NONE
        .fill(Color32::from_rgba_unmultiplied(8, 10, 16, 220))
        .stroke(Stroke::new(1.0_f32, accent))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            let preview = ui.allocate_response(Vec2::new(ui.available_width(), 92.0), egui::Sense::hover());
            ui.painter().rect_filled(preview.rect, 8.0, skin_color(skin.style));
            ui.painter().text(
                preview.rect.center(),
                egui::Align2::CENTER_CENTER,
                "✦",
                egui::FontId::proportional(36.0),
                Color32::from_white_alpha(210),
            );
            ui.add_space(8.0);
            ui.label(RichText::new(&skin.name).strong().size(16.0));
            ui.label(RichText::new("ALL LEADERS · COSMETIC").size(10.0).color(palette::text_muted()));
            ui.add_space(8.0);
            if equipped {
                ui.label(RichText::new("EQUIPPED").strong().color(palette::neon_cyan()));
            } else if skin.owned {
                if Button::secondary("EQUIP")
                    .disabled(busy)
                    .small()
                    .min_size(egui::vec2(0.0, 44.0))
                    .show(ui)
                    .clicked()
                {
                    *action = Some(UiAction::EquipSkin(skin.id.clone()));
                }
            } else if Button::primary(&format!("UNLOCK {} GEMS", skin.cost_gems))
                .disabled(busy)
                .small()
                .min_size(egui::vec2(0.0, 44.0))
                .show(ui)
                .clicked()
            {
                *action = Some(UiAction::UnlockSkin(skin.id.clone()));
            }
        });
}

pub fn draw(
    root_ui: &mut Ui,
    state: &mut MainMenuState,
    asset_loader: &AssetLoader,
    action: &mut Option<UiAction>,
) {
    let screen = root_ui.ctx().content_rect();
    root_ui.painter().rect_filled(
        screen,
        0.0,
        Color32::from_rgba_unmultiplied(5, 7, 12, 230),
    );

    let metrics = super::layout::main_menu_metrics(root_ui.ctx());
    Frame::NONE
        .inner_margin(Margin::symmetric(metrics.outer_pad as i8, 12))
        .show(root_ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if Button::ghost("← BACK")
                    .small()
                    .min_size(egui::vec2(96.0, 44.0))
                    .show(ui)
                    .clicked()
                {
                    state.go_home();
                }
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("STORE").size(11.0).strong().color(palette::neon_gold()));
                    ui.label(RichText::new("Build your war chest").size(24.0).strong());
                });
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("◈ {} LAURELS", state.store_catalog.laurels)).strong().color(palette::neon_gold()));
                    ui.add_space(18.0);
                    ui.label(RichText::new(format!("✦ {} GEMS", state.store_catalog.gems)).strong().color(palette::neon_cyan()));
                });
            });
        });

    Frame::NONE
        .inner_margin(Margin::symmetric(metrics.outer_pad as i8, 8))
        .show(root_ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("store_body_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width().min(1180.0));
                    ui.vertical_centered(|ui| {
                        ui.horizontal(|ui| {
                            Heading::new("LEADERS").cyan().show(ui);
                            ui.add_space(8.0);
                            Subtitle::new("WEEKLY ROTATION · DISTINCT PERKS").muted().show(ui);
                        });
                        ui.add_space(8.0);
                        let leaders = &state.store_catalog.leaders;
                        let columns = super::layout::main_menu_metrics(ui.ctx()).columns();
                        ui.columns(columns, |columns| {
                            let column_count = columns.len();
                            for (idx, offer) in leaders.iter().enumerate() {
                                draw_leader_card(&mut columns[idx % column_count], offer, asset_loader, action, state.store_busy);
                            }
                        });

                        ui.add_space(18.0);
                        ui.horizontal(|ui| {
                            Heading::new("SKINS").cyan().show(ui);
                            ui.add_space(8.0);
                            Subtitle::new("COSMETICS · ALL LEADERS").muted().show(ui);
                        });
                        ui.add_space(8.0);
                        let skins = &state.store_catalog.skins;
                        let columns = super::layout::main_menu_metrics(ui.ctx()).columns();
                        ui.columns(columns, |columns| {
                            let column_count = columns.len();
                            for (idx, skin) in skins.iter().enumerate() {
                                draw_skin_card(
                                    &mut columns[idx % column_count],
                                    skin,
                                    state.selected_skin.as_deref(),
                                    action,
                                    state.store_busy,
                                );
                            }
                        });

                        ui.add_space(18.0);
                        ui.horizontal(|ui| {
                            Heading::new("GEM BUNDLES").cyan().show(ui);
                            ui.add_space(8.0);
                            Subtitle::new("ONE-TIME PURCHASES").muted().show(ui);
                        });
                        ui.add_space(8.0);
                        ui.horizontal_wrapped(|ui| {
                            for bundle in &state.store_catalog.gem_bundles {
                                Card::surface().show(ui, |ui| {
                                    ui.set_min_width(220.0);
                                    ui.vertical_centered(|ui| {
                                        ui.label(RichText::new(format!("✦ {} GEMS", bundle.gems)).size(20.0).strong().color(palette::neon_cyan()));
                                        ui.label(RichText::new("RevenueCat checkout").size(11.0).color(palette::text_muted()));
                                        ui.add_space(8.0);
                                        if Button::primary("BUY ONLINE")
                                            .disabled(state.store_busy)
                                            .small()
                                            .min_size(egui::vec2(0.0, 44.0))
                                            .show(ui)
                                            .clicked()
                                        {
                                            *action = Some(UiAction::BuyGems(bundle.product_id.clone()));
                                        }
                                    });
                                });
                            }
                        });
                        if let Some(message) = &state.error_message {
                            ui.add_space(12.0);
                            ui.label(RichText::new(message).color(palette::danger()));
                        }
                        ui.add_space(24.0);
                        BodyText::new("Digital items are delivered to your player account after the server confirms the purchase.")
                            .muted()
                            .show(ui);
                    });
                });
        });
}
