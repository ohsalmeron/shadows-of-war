//! Persistent native main-menu shell.
//!
//! The shell owns the viewport chrome. Route implementations only receive the
//! central content rect, which keeps the header, navigation and legal footer
//! stable while the user moves through the menu or receives network updates.

use super::{MainMenuRoute, MainMenuSection, MainMenuState};
use crate::UiAction;
use crate::ui::asset_loader::AssetLoader;
use egui::{Align2, Color32, CornerRadius, FontId, Frame, Margin, Rect, Sense, Stroke, Ui, Vec2};

const HEADER_H_DESKTOP: f32 = 74.0;
const HEADER_H_PHONE: f32 = 66.0;
const LEGAL_FOOTER_H: f32 = 26.0;
const MOBILE_NAV_H: f32 = 70.0;
const RAIL_W: f32 = 96.0;
const NAV_ITEM_H: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NavIcon {
    Battle,
    Heroes,
    Store,
    Profile,
}

pub(crate) fn draw(
    root_ui: &mut Ui,
    state: &mut MainMenuState,
    asset_loader: &mut AssetLoader,
    lang: sow_i18n::Language,
    reduced_motion: bool,
) -> Option<UiAction> {
    let mut action = None;
    let metrics = super::layout::main_menu_metrics(root_ui.ctx());
    let phone = metrics.is_phone();
    let strings = &sow_i18n::get(lang).main_menu;

    // The leader image is the one global surface shared by every native menu
    // section. Content panels provide legibility without replacing the art.
    let screen = root_ui.ctx().content_rect();
    let portrait = screen.width() < screen.height();
    crate::widgets::draw_leader_hero_backdrop(
        root_ui,
        &mut crate::widgets::LeaderHeroBackdropCtx {
            screen_rect: screen,
            selected: state.selected_leader,
            mobile: portrait,
            asset_loader,
            transition: &mut state.leader_backdrop,
            loading_label: &strings.loading_leader_portrait,
            draw_picker_gradient: false,
        },
    );

    if phone {
        egui::Panel::bottom("main_menu_mobile_nav")
            .resizable(false)
            .exact_size(MOBILE_NAV_H + state.safe_area_bottom)
            .frame(
                Frame::NONE
                    .fill(Color32::from_rgba_unmultiplied(7, 9, 13, 242))
                    .inner_margin(Margin::symmetric(6, 3)),
            )
            .show_inside(root_ui, |ui| {
                draw_bottom_nav(ui, state);
                if state.safe_area_bottom > 0.0 {
                    ui.allocate_space(Vec2::new(ui.available_width(), state.safe_area_bottom));
                }
            });
    } else {
        egui::Panel::bottom("main_menu_legal_footer")
            .resizable(false)
            .exact_size(LEGAL_FOOTER_H)
            .frame(
                Frame::NONE
                    .fill(sow_ui_kit::theme::palette::surface())
                    .inner_margin(Margin::symmetric(12, 2)),
            )
            .show_inside(root_ui, |ui| {
                super::draw_terms_privacy_footer(ui, lang, &mut action);
            });
    }

    egui::Panel::top("main_menu_persistent_header")
        .resizable(false)
        .exact_size(if phone {
            HEADER_H_PHONE
        } else {
            HEADER_H_DESKTOP
        })
        .frame(
            Frame::NONE
                .fill(Color32::from_rgba_unmultiplied(7, 9, 13, 120))
                .inner_margin(Margin::symmetric(if phone { 6 } else { 10 }, 7)),
        )
        .show_inside(root_ui, |ui| {
            super::topbar::draw(ui, state, asset_loader, lang, &mut action);
            if state.is_waiting {
                draw_queue_badge(ui, state);
            }
        });

    if !phone {
        egui::Panel::left("main_menu_navigation_rail")
            .resizable(false)
            .exact_size(RAIL_W)
            .frame(
                Frame::NONE
                    .fill(Color32::from_rgba_unmultiplied(7, 9, 13, 186))
                    .stroke(Stroke::new(1.0_f32, Color32::from_white_alpha(24)))
                    .inner_margin(Margin::symmetric(6, 10)),
            )
            .show_inside(root_ui, |ui| {
                draw_rail(ui, state);
            });
    }

    egui::CentralPanel::default()
        .frame(Frame::NONE)
        .show_inside(root_ui, |ui| {
            let content_rect = ui.available_rect_before_wrap();
            ui.set_clip_rect(ui.clip_rect().intersect(content_rect));
            draw_content(
                ui,
                content_rect,
                state,
                asset_loader,
                lang,
                reduced_motion,
                &mut action,
            );
        });

    draw_overlays(root_ui, state, asset_loader, lang, &mut action, phone);
    action
}

fn draw_content(
    ui: &mut Ui,
    content_rect: Rect,
    state: &mut MainMenuState,
    asset_loader: &mut AssetLoader,
    lang: sow_i18n::Language,
    reduced_motion: bool,
    action: &mut Option<UiAction>,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let route = state.visible_route();
    let metrics = super::layout::main_menu_metrics(ui.ctx());

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| match route {
            MainMenuRoute::Home => {
                // Home is a fixed decision surface. It deliberately does not
                // get a ScrollArea; the map and compact action rows are sized
                // from the central viewport budget.
                super::draw_home_content(ui, state, asset_loader, lang, strings, metrics, action);
            }
            MainMenuRoute::Browser => {
                super::draw_browser(ui, state, asset_loader, lang, action);
            }
            MainMenuRoute::Create => {
                super::custom_game::draw(ui, state, asset_loader, action, lang, reduced_motion);
            }
            MainMenuRoute::Queue => {
                let (_, action_min_h, _, _) = super::layout::menu_layout_chrome(
                    ui.ctx(),
                    content_rect.height(),
                    content_rect.width(),
                    metrics.is_compact(),
                );
                super::queue_overlay::draw_queue_overlay(
                    ui,
                    state,
                    action_min_h,
                    action,
                    asset_loader,
                    lang,
                );
            }
            MainMenuRoute::Store => {
                super::store::draw(ui, state, asset_loader, action);
            }
            MainMenuRoute::Profile => {
                super::profile::draw_native(ui, state, asset_loader, action);
            }
            MainMenuRoute::Heroes => {
                draw_heroes(ui, state, asset_loader);
            }
        },
    );
}

fn draw_rail(ui: &mut Ui, state: &mut MainMenuState) {
    ui.vertical_centered(|ui| {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("SOW")
                .strong()
                .size(16.0)
                .color(sow_ui_kit::theme::palette::neon_gold()),
        );
        ui.add_space(18.0);
        for (section, icon, label) in [
            (MainMenuSection::Battle, NavIcon::Battle, "BATTLE"),
            (MainMenuSection::Heroes, NavIcon::Heroes, "HEROES"),
            (MainMenuSection::Store, NavIcon::Store, "STORE"),
            (MainMenuSection::Profile, NavIcon::Profile, "PROFILE"),
        ] {
            draw_nav_item(
                ui,
                state,
                section,
                icon,
                label,
                Vec2::new(ui.available_width(), NAV_ITEM_H),
            );
            ui.add_space(4.0);
        }
    });
}

fn draw_bottom_nav(ui: &mut Ui, state: &mut MainMenuState) {
    let item_spacing = 4.0;
    let width = ((ui.available_width() - item_spacing * 3.0) / 4.0).max(1.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(item_spacing, 0.0);
        for (section, icon, label) in [
            (MainMenuSection::Battle, NavIcon::Battle, "BATTLE"),
            (MainMenuSection::Heroes, NavIcon::Heroes, "HEROES"),
            (MainMenuSection::Store, NavIcon::Store, "STORE"),
            (MainMenuSection::Profile, NavIcon::Profile, "PROFILE"),
        ] {
            draw_nav_item(ui, state, section, icon, label, Vec2::new(width, 62.0));
        }
    });
}

fn draw_nav_item(
    ui: &mut Ui,
    state: &mut MainMenuState,
    section: MainMenuSection,
    icon: NavIcon,
    label: &str,
    size: Vec2,
) {
    let selected = state.active_section() == section;
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if selected {
        Color32::from_rgba_unmultiplied(243, 177, 43, 42)
    } else if response.hovered() {
        Color32::from_white_alpha(18)
    } else {
        Color32::TRANSPARENT
    };
    let stroke = if selected {
        Stroke::new(1.0_f32, palette::neon_gold())
    } else {
        Stroke::new(1.0_f32, Color32::TRANSPARENT)
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(8),
        fill,
        stroke,
        egui::StrokeKind::Inside,
    );

    let icon_rect = Rect::from_center_size(rect.center() - Vec2::new(0.0, 10.0), Vec2::splat(24.0));
    paint_nav_icon(
        ui,
        icon_rect,
        icon,
        if selected {
            palette::neon_gold()
        } else {
            palette::text_muted()
        },
    );
    ui.painter().text(
        rect.center() + Vec2::new(0.0, 16.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(9.0),
        if selected {
            Color32::WHITE
        } else {
            palette::text_muted()
        },
    );

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        state.open_section(section);
    }
}

fn paint_nav_icon(ui: &Ui, rect: Rect, icon: NavIcon, color: Color32) {
    let painter = ui.painter();
    let stroke = Stroke::new(1.8_f32, color);
    let c = rect.center();
    match icon {
        NavIcon::Battle => {
            painter.line_segment(
                [
                    rect.left_top() + Vec2::new(4.0, 4.0),
                    rect.right_bottom() - Vec2::new(4.0, 4.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    rect.right_top() + Vec2::new(-4.0, 4.0),
                    rect.left_bottom() + Vec2::new(4.0, -4.0),
                ],
                stroke,
            );
            painter.circle_filled(c, 2.5, color);
        }
        NavIcon::Heroes => {
            let shield = Rect::from_center_size(c, Vec2::new(16.0, 19.0));
            painter.rect(
                shield,
                CornerRadius::same(4),
                Color32::TRANSPARENT,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.line_segment([c - Vec2::new(0.0, 5.0), c + Vec2::new(0.0, 6.0)], stroke);
            painter.line_segment([c - Vec2::new(5.0, 0.0), c + Vec2::new(5.0, 0.0)], stroke);
        }
        NavIcon::Store => {
            let bag = Rect::from_center_size(c + Vec2::new(0.0, 2.0), Vec2::new(16.0, 14.0));
            painter.rect(
                bag,
                CornerRadius::same(3),
                Color32::TRANSPARENT,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.circle_stroke(c + Vec2::new(0.0, -4.0), 4.0, stroke);
        }
        NavIcon::Profile => {
            painter.circle_stroke(c - Vec2::new(0.0, 4.0), 4.0, stroke);
            painter.line_segment([c + Vec2::new(-8.0, 8.0), c + Vec2::new(-4.0, 4.0)], stroke);
            painter.line_segment([c + Vec2::new(4.0, 4.0), c + Vec2::new(8.0, 8.0)], stroke);
            painter.line_segment([c + Vec2::new(-8.0, 8.0), c + Vec2::new(8.0, 8.0)], stroke);
        }
    }
}

fn draw_queue_badge(ui: &mut Ui, state: &MainMenuState) {
    let text = if state.wait_timer_secs > 0.0 {
        format!("QUEUE  {}s", state.wait_timer_secs.ceil() as u32)
    } else {
        "QUEUE".to_string()
    };
    let rect = Rect::from_min_size(
        egui::pos2(ui.max_rect().right() - 118.0, ui.max_rect().bottom() - 24.0),
        Vec2::new(108.0, 20.0),
    );
    ui.painter()
        .rect_filled(rect, 6.0, Color32::from_black_alpha(180));
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(9.0),
        sow_ui_kit::theme::palette::neon_cyan(),
    );
}

fn draw_heroes(ui: &mut Ui, state: &mut MainMenuState, asset_loader: &mut AssetLoader) {
    asset_loader.ensure_avatars_loaded(ui.ctx());
    let phone = super::layout::main_menu_metrics(ui.ctx()).is_phone();
    let outer = ui.available_rect_before_wrap();
    let pad = if phone { 10.0 } else { 18.0 };
    let inner = outer.shrink(pad);

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("HEROES")
                        .size(if phone { 20.0 } else { 28.0 })
                        .strong()
                        .color(sow_ui_kit::theme::palette::neon_cyan()),
                );
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("SELECT YOUR COMMANDER")
                        .size(11.0)
                        .strong()
                        .color(sow_ui_kit::theme::palette::text_muted()),
                );
            });
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .id_salt("native_heroes_content")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let columns = if phone { 2 } else { 4 };
                    let gap = if phone { 6.0 } else { 10.0 };
                    ui.spacing_mut().item_spacing = Vec2::splat(gap);
                    ui.columns(columns, |cols| {
                        for (idx, &leader) in sow_core::player::Leader::ALL.iter().enumerate() {
                            draw_hero_card(
                                &mut cols[idx % columns],
                                state,
                                asset_loader,
                                leader,
                                phone,
                            );
                        }
                    });
                });
        },
    );
}

fn draw_hero_card(
    ui: &mut Ui,
    state: &mut MainMenuState,
    asset_loader: &AssetLoader,
    leader: sow_core::player::Leader,
    phone: bool,
) {
    let selected = state.selected_leader == leader;
    let width = ui.available_width();
    let image_h = (width * 9.0 / 16.0).clamp(78.0, if phone { 132.0 } else { 170.0 });
    let frame = Frame::NONE
        .fill(if selected {
            Color32::from_rgba_unmultiplied(17, 33, 42, 232)
        } else {
            Color32::from_rgba_unmultiplied(8, 12, 18, 210)
        })
        .stroke(Stroke::new(
            if selected { 1.5_f32 } else { 1.0_f32 },
            if selected {
                sow_ui_kit::theme::palette::neon_gold()
            } else {
                sow_ui_kit::theme::palette::field_border()
            },
        ))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(if phone { 7 } else { 9 }));
    let response = frame.show(ui, |ui| {
        let image_rect = ui
            .allocate_space(Vec2::new(ui.available_width(), image_h))
            .1;
        let rgb = leader.filler_rgb();
        let fill = Color32::from_rgb(
            (rgb[0] * 255.0).round() as u8,
            (rgb[1] * 255.0).round() as u8,
            (rgb[2] * 255.0).round() as u8,
        );
        ui.painter().rect_filled(image_rect, 6.0, fill);
        if let Some(texture) = asset_loader
            .leader_portrait_texture(leader, phone)
            .or_else(|| asset_loader.avatars.get(&leader))
        {
            draw_aspect_fit(ui, image_rect, texture, 6);
        }
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(leader.name().to_uppercase())
                .strong()
                .size(13.0),
        );
        ui.label(
            egui::RichText::new(crate::widgets::avatar_picker::leader_civilization(leader).name())
                .size(9.0)
                .color(sow_ui_kit::theme::palette::text_muted()),
        );
    });
    let response = ui.interact(
        response.response.rect,
        ui.make_persistent_id(("native_hero_card", leader)),
        Sense::click(),
    );
    if response.clicked() {
        state.selected_leader = leader;
        state.selected_civilization = crate::widgets::avatar_picker::leader_civilization(leader);
    }
}

fn draw_aspect_fit(ui: &mut Ui, rect: Rect, texture: &egui::TextureHandle, radius: u8) {
    let source = texture.size_vec2();
    if source.x <= 0.0 || source.y <= 0.0 {
        return;
    }
    let scale = (rect.width() / source.x).min(rect.height() / source.y);
    let image_rect = Rect::from_center_size(rect.center(), source * scale);
    ui.put(
        image_rect,
        egui::Image::new(texture)
            .fit_to_exact_size(image_rect.size())
            .corner_radius(CornerRadius::same(radius)),
    );
}

fn draw_overlays(
    root_ui: &mut Ui,
    state: &mut MainMenuState,
    asset_loader: &mut AssetLoader,
    lang: sow_i18n::Language,
    action: &mut Option<UiAction>,
    compact: bool,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    modals::draw_connecting_indicator(root_ui.ctx(), state, lang, compact);
    modals::draw_map_download_indicator(root_ui.ctx(), state, lang, compact);

    if let Some(target_id) = state.join_password_for_lobby {
        super::join_browser::draw_password_modal(
            root_ui, state, target_id, action, strings, compact,
        );
    }

    if state.show_leader_picker
        && crate::widgets::draw_leader_picker_modal(
            root_ui.ctx(),
            &mut state.selected_leader,
            &mut state.selected_civilization,
            asset_loader,
            &mut state.leader_backdrop,
            lang,
        )
    {
        state.show_leader_picker = false;
    }

    if let Some(notice) = state.notice {
        let now = root_ui.input(|i| i.time);
        let shown_at = *state.notice_at.get_or_insert(now);
        let dismissed = modals::draw_lobby_notice(root_ui, notice, strings, compact);
        root_ui.ctx().request_repaint();
        if dismissed || now - shown_at >= 3.0 {
            state.notice = None;
            state.notice_at = None;
        }
    }
}

mod modals {
    pub(super) use super::super::modals::*;
}

mod palette {
    pub(super) use sow_ui_kit::theme::palette::*;
}
