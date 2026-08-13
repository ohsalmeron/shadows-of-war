use egui::{Color32, Context, RichText, Stroke, vec2};
use sow_core::protocol::PlayerSnapshot;
use sow_i18n::Language;

use super::super::state::{HudState, get_player_display_name};

pub(in crate::ui::hud) fn paint_betrayal_ally_portrait(
    ui: &mut egui::Ui,
    ally: &PlayerSnapshot,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    alpha: f32,
    size: f32,
) {
    use sow_core::player::PlayerType;

    let (rect, _) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    let center = rect.center();
    let radius = size * 0.5;
    let inner_r = radius - 2.0;

    let rgb = ally
        .team
        .map_or(ally.color, sow_core::player::team_territory_rgb);
    let vibrant = Color32::from_rgb(
        (rgb[0] * 255.0).round() as u8,
        (rgb[1] * 255.0).round() as u8,
        (rgb[2] * 255.0).round() as u8,
    )
    .linear_multiply(alpha);

    match ally.player_type {
        PlayerType::Human => {
            let leader_rgb = ally.leader.filler_rgb();
            let leader_fill = Color32::from_rgb(
                (leader_rgb[0] * 255.0).round() as u8,
                (leader_rgb[1] * 255.0).round() as u8,
                (leader_rgb[2] * 255.0).round() as u8,
            )
            .linear_multiply(alpha);
            ui.painter().circle_filled(center, inner_r, leader_fill);
            if let Some(tex) = asset_loader
                .avatars
                .get(&ally.leader)
                .or(asset_loader.avatar_fallback.as_ref())
            {
                let image = egui::Image::new(tex)
                    .fit_to_exact_size(vec2(inner_r * 2.0, inner_r * 2.0))
                    .corner_radius(egui::CornerRadius::same((inner_r as u8).max(1)));
                let image_rect =
                    egui::Rect::from_center_size(center, vec2(inner_r * 2.0, inner_r * 2.0));
                ui.put(image_rect, image);
            }
            ui.painter()
                .circle_stroke(center, radius, Stroke::new(2.0_f32, leader_fill));
            if ally.team.is_some() {
                // Team games: the frame carries the team color (the portrait
                // texture stays the leader's identity).
                ui.painter()
                    .circle_stroke(center, radius, Stroke::new(2.0_f32, vibrant));
            }
        }
        PlayerType::Bot => {
            ui.painter().circle_filled(center, inner_r, vibrant);
            let animal = sow_core::player::tribe_animal(ally.id, &ally.name);
            let emoji_rect =
                egui::Rect::from_center_size(center, vec2(inner_r * 1.4, inner_r * 1.4));
            if !crate::widgets::try_paint_emoji(ui.painter(), animal, emoji_rect, Color32::WHITE) {
                let galley = ui.painter().layout_no_wrap(
                    animal.to_owned(),
                    egui::FontId::proportional(inner_r * 1.2),
                    Color32::WHITE.linear_multiply(alpha),
                );
                ui.painter().galley(
                    egui::pos2(
                        center.x - galley.size().x / 2.0,
                        center.y - galley.size().y / 2.0,
                    ),
                    galley,
                    Color32::WHITE.linear_multiply(alpha),
                );
            }
            ui.painter()
                .circle_stroke(center, radius, Stroke::new(2.0_f32, vibrant));
        }
        PlayerType::Nation => {
            ui.painter().circle_filled(center, inner_r, vibrant);
            let emoji = ally.leader.menu_emoji();
            let emoji_rect =
                egui::Rect::from_center_size(center, vec2(inner_r * 1.4, inner_r * 1.4));
            if !crate::widgets::try_paint_emoji(ui.painter(), emoji, emoji_rect, Color32::WHITE) {
                let galley = ui.painter().layout_no_wrap(
                    emoji.to_owned(),
                    egui::FontId::proportional(inner_r * 1.2),
                    Color32::WHITE.linear_multiply(alpha),
                );
                ui.painter().galley(
                    egui::pos2(
                        center.x - galley.size().x / 2.0,
                        center.y - galley.size().y / 2.0,
                    ),
                    galley,
                    Color32::WHITE.linear_multiply(alpha),
                );
            }
            ui.painter()
                .circle_stroke(center, radius, Stroke::new(2.0_f32, vibrant));
        }
    }

    ui.painter().circle_stroke(
        center,
        radius + 1.5,
        Stroke::new(1.0_f32, Color32::from_black_alpha((120.0 * alpha) as u8)),
    );
}

pub(in crate::ui::hud) fn draw_betrayal_ally_card(
    ui: &mut egui::Ui,
    ally: &PlayerSnapshot,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    alpha: f32,
    compact: bool,
) {
    let ally_name = sow_core::player::display_name(ally.id, &ally.name, ally.player_type);
    let subtitle = match ally.player_type {
        sow_core::player::PlayerType::Bot => "Tribe".to_string(),
        _ => ally.civilization.name().to_string(),
    };
    let avatar_size = if compact { 52.0 } else { 64.0 };

    egui::Frame::new()
        .fill(sow_ui_kit::theme::palette::field_bg().linear_multiply(alpha))
        .stroke(Stroke::new(
            1.0_f32,
            sow_ui_kit::theme::palette::field_border().linear_multiply(alpha),
        ))
        .corner_radius(8)
        .inner_margin(if compact {
            egui::Margin::symmetric(12, 10)
        } else {
            egui::Margin::symmetric(16, 12)
        })
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width());
            ui.vertical_centered(|ui| {
                paint_betrayal_ally_portrait(ui, ally, asset_loader, alpha, avatar_size);
                ui.add_space(if compact { 8.0 } else { 10.0 });
                crate::widgets::emoji_label(
                    ui,
                    &ally_name,
                    egui::FontId::proportional(if compact { 16.0 } else { 18.0 }),
                    Color32::WHITE.linear_multiply(alpha),
                );
                ui.label(
                    RichText::new(subtitle)
                        .size(if compact { 12.0 } else { 13.0 })
                        .color(sow_ui_kit::theme::palette::neon_gold().linear_multiply(alpha)),
                );
            });
        });
}
pub(in crate::ui::hud) fn draw_betrayal_overlay(
    ctx: &Context,
    state: &mut HudState,
    cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>,
    lang: Language,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
) {
    let strings = &sow_i18n::get(lang).hud;
    if let Some(warning) = state.show_betrayal_warning.clone() {
        state.betrayal_warning_cached = Some(warning);
    }

    let is_active = state.show_betrayal_warning.is_some();
    let anim_dur = sow_ui_kit::theme::anim_duration_from_ctx(ctx);
    let anim = crate::ui::animation::panel_in_out_anim(
        ctx,
        egui::Id::new("betrayal_panel_animation"),
        is_active,
        anim_dur,
        crate::ui::animation::PANEL_Y_SLIDE,
        crate::ui::animation::SlideDir::Down,
    );

    if anim.progress <= 0.01 {
        return;
    }

    let Some((ally_id, intent)) = state.betrayal_warning_cached.clone() else {
        return;
    };

    let alpha = anim.progress;
    let y_offset = anim.offset;
    let screen_rect = ctx.content_rect();
    let compact = screen_rect.width() < 768.0 || screen_rect.width() < screen_rect.height() * 1.25;

    sow_ui_kit::theme::paint_scrim(ctx, "betrayal_overlay_bg", alpha);

    let window = egui::Window::new("betrayal_warning_modal")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .order(egui::Order::Foreground);

    let panel_w = if compact {
        (screen_rect.width() - 32.0).min(500.0)
    } else {
        520.0
    };

    let window = window.fixed_size(vec2(panel_w, 0.0)).anchor(
        egui::Align2::CENTER_CENTER,
        vec2(0.0, if compact { y_offset } else { -20.0 + y_offset }),
    );

    let border_color = sow_ui_kit::theme::palette::danger().linear_multiply(alpha);

    window
        .frame(
            sow_ui_kit::theme::standard_panel_frame(compact)
                .fill(sow_ui_kit::theme::palette::surface().linear_multiply(alpha))
                .stroke(egui::Stroke::new(2.0f32 * anim.scale, border_color))
                .inner_margin(if compact { 16.0 } else { 24.0 }),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                let ally_name = get_player_display_name(&state.players, ally_id, "Ally");

                sow_ui_kit::theme::outlined_label(
                    ui,
                    &strings.betrayal_title,
                    egui::FontId::proportional(if compact { 22.0 } else { 28.0 }),
                    border_color,
                );

                ui.add_space(if compact { 16.0 } else { 12.0 });

                ui.label(
                    RichText::new("If you attack this ally, other allies could attack you.")
                        .size(if compact { 14.0 } else { 16.0 })
                        .color(Color32::WHITE.linear_multiply(alpha)),
                );

                ui.add_space(if compact { 12.0 } else { 16.0 });

                if let Some(ally) = state.players.iter().find(|p| p.id == ally_id) {
                    draw_betrayal_ally_card(ui, ally, asset_loader, alpha, compact);
                } else {
                    crate::widgets::emoji_label(
                        ui,
                        &ally_name,
                        egui::FontId::proportional(if compact { 16.0 } else { 18.0 }),
                        Color32::WHITE.linear_multiply(alpha),
                    );
                }

                ui.add_space(if compact { 12.0 } else { 16.0 });

                ui.label(
                    RichText::new("Are you sure?")
                        .size(if compact { 15.0 } else { 18.0 })
                        .strong()
                        .color(sow_ui_kit::theme::palette::neon_gold().linear_multiply(alpha)),
                );

                ui.add_space(if compact { 32.0 } else { 24.0 });

                let btn_w = if compact {
                    (ui.available_width() - 8.0) / 2.0
                } else {
                    160.0
                };
                let btn_h = if compact { 40.0 } else { 44.0 };

                ui.horizontal(|ui| {
                    let spacing = if compact { 8.0 } else { 16.0 };
                    ui.spacing_mut().item_spacing.x = spacing;

                    let right_btn_w = if compact { btn_w } else { 140.0 };
                    let total_width = btn_w + spacing + right_btn_w;
                    let available = ui.available_width();
                    if available > total_width {
                        ui.add_space((available - total_width) / 2.0);
                    }

                    if ui
                        .add(
                            crate::widgets::ThemeButton::new(&strings.betrayal_keep)
                                .style(crate::widgets::ThemeButtonStyle::Tertiary)
                                .custom_fill(
                                    sow_ui_kit::theme::palette::button_inactive()
                                        .linear_multiply(alpha),
                                )
                                .custom_text_color(Color32::WHITE.linear_multiply(alpha))
                                .min_size(vec2(btn_w, btn_h))
                                .text_size(if compact { 13.0 } else { 16.0 }),
                        )
                        .clicked()
                    {
                        state.show_betrayal_warning = None;
                    }

                    if ui
                        .add(
                            crate::widgets::ThemeButton::new(&strings.betrayal_yes)
                                .style(crate::widgets::ThemeButtonStyle::Danger)
                                .custom_fill(
                                    sow_ui_kit::theme::palette::danger().linear_multiply(alpha),
                                )
                                .custom_text_color(Color32::WHITE.linear_multiply(alpha))
                                .min_size(vec2(right_btn_w, btn_h))
                                .text_size(if compact { 13.0 } else { 16.0 }),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        cancel_intents.push(sow_core::protocol::GameplayIntent::BreakAlliance {
                            target_player: ally_id,
                        });
                        cancel_intents.push(intent);
                        state.show_betrayal_warning = None;
                    }
                });
            });
        });
}
