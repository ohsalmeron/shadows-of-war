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
            // The source portraits are 16:9. Size the art well to that ratio first;
            // never let a shorter component crop or stretch the portrait.
            let art_width = ui.available_width();
            let art_size = Vec2::new(art_width, art_width * 9.0 / 16.0);
            let (art_rect, _) = ui.allocate_exact_size(art_size, egui::Sense::hover());
            ui.painter().rect_filled(
                art_rect,
                8.0,
                Color32::from_rgb(24, 28, 38),
            );
            let texture = asset_loader
                .leader_portrait_texture(leader, false)
                .or_else(|| asset_loader.avatars.get(&leader));
            if let Some(texture) = texture {
                ui.put(
                    art_rect,
                    egui::Image::new(texture)
                        .fit_to_exact_size(art_rect.size())
                        .maintain_aspect_ratio(true)
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
            let preview = ui.allocate_response(
                Vec2::new(ui.available_width(), 92.0),
                egui::Sense::hover(),
            );
            draw_skin_preview(ui, preview.rect, skin.style);
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

fn draw_skin_preview(ui: &mut Ui, rect: egui::Rect, style: u8) {
    let painter = ui.painter().with_clip_rect(rect);
    let base = skin_color(style);
    painter.rect_filled(rect, 8.0, base);
    let light = Color32::from_white_alpha(46);
    let bright = Color32::from_white_alpha(115);
    match style {
        1 => {
            for offset in (-rect.height() as i32..rect.width() as i32).step_by(28) {
                let x = offset as f32;
                painter.line_segment(
                    [
                        egui::pos2(rect.left() + x, rect.bottom()),
                        egui::pos2(rect.left() + x + rect.height(), rect.top()),
                    ],
                    Stroke::new(7.0_f32, light),
                );
            }
            painter.circle_filled(
                egui::pos2(rect.right() - 34.0, rect.top() + 30.0),
                18.0,
                bright,
            );
        }
        2 => {
            let step = 30.0;
            let mut x = rect.left() - rect.height();
            while x < rect.right() {
                painter.line_segment(
                    [egui::pos2(x, rect.top()), egui::pos2(x + rect.height(), rect.bottom())],
                    Stroke::new(3.0_f32, bright),
                );
                painter.line_segment(
                    [
                        egui::pos2(x + rect.height(), rect.top()),
                        egui::pos2(x, rect.bottom()),
                    ],
                    Stroke::new(1.0_f32, light),
                );
                x += step;
            }
        }
        _ => {
            painter.rect_stroke(
                rect.shrink(18.0),
                5.0,
                Stroke::new(8.0_f32, light),
                egui::StrokeKind::Inside,
            );
            painter.circle_stroke(
                rect.center(),
                24.0,
                Stroke::new(5.0_f32, bright),
            );
        }
    }
}

pub fn draw(
    root_ui: &mut Ui,
    state: &mut MainMenuState,
    asset_loader: &mut AssetLoader,
    action: &mut Option<UiAction>,
) {
    asset_loader.ensure_store_leader_portraits_loaded(root_ui.ctx());
    let metrics = super::layout::main_menu_metrics(root_ui.ctx());
    Frame::NONE
        .inner_margin(Margin::symmetric(metrics.outer_pad as i8, 10))
        .show(root_ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("STORE").size(11.0).strong().color(palette::neon_gold()));
                    ui.label(RichText::new("Build your war chest").size(24.0).strong());
                });
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{} LAURELS", state.store_catalog.laurels)).strong().color(palette::neon_gold()));
                    ui.add_space(18.0);
                    ui.label(RichText::new(format!("{} GEMS", state.store_catalog.gems)).strong().color(palette::neon_cyan()));
                });
            });
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .id_salt("store_body_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        Heading::new("LEADERS").cyan().show(ui);
                        ui.add_space(8.0);
                        Subtitle::new("WEEKLY ROTATION · DISTINCT PERKS").muted().show(ui);
                    });
                    ui.add_space(6.0);
                    let leaders = &state.store_catalog.leaders;
                    let columns = super::layout::main_menu_metrics(ui.ctx()).columns();
                    ui.columns(columns, |columns| {
                        let column_count = columns.len();
                        for (idx, offer) in leaders.iter().enumerate() {
                            draw_leader_card(&mut columns[idx % column_count], offer, asset_loader, action, state.store_busy);
                        }
                    });

                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        Heading::new("SKINS").cyan().show(ui);
                        ui.add_space(8.0);
                        Subtitle::new("COSMETICS · ALL LEADERS").muted().show(ui);
                    });
                    ui.add_space(6.0);
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

                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        Heading::new("GEM BUNDLES").cyan().show(ui);
                        ui.add_space(8.0);
                        Subtitle::new("ONE-TIME PURCHASES").muted().show(ui);
                    });
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        for bundle in &state.store_catalog.gem_bundles {
                            Card::surface().show(ui, |ui| {
                                ui.set_min_width(220.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(RichText::new(format!("{} GEMS", bundle.gems)).size(20.0).strong().color(palette::neon_cyan()));
                                    ui.label(RichText::new("RevenueCat checkout").size(11.0).color(palette::text_muted()));
                                    ui.add_space(6.0);
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
                        ui.add_space(10.0);
                        ui.label(RichText::new(message).color(palette::danger()));
                    }
                    ui.add_space(16.0);
                    BodyText::new("Digital items are delivered to your player account after the server confirms the purchase.")
                        .muted()
                        .show(ui);
                });
        });
}
