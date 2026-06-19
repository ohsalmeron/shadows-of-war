use egui::{pos2, vec2, Align2, Color32, RichText, Stroke};
use sow_i18n::Language;

use super::super::state::HudState;

pub(in crate::ui::hud) fn draw_troop_bar(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    troops: f64,
    max_troops: f64,
    troop_rate: f64,
    compact: bool,
    is_increasing: bool,
) {
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

    let bg_color = crate::ui::theme::palette::field_bg();
    let green_color = crate::ui::theme::palette::neon_cyan_hover();
    let orange_color = crate::ui::theme::palette::neon_cyan();

    // Draw background
    ui.painter().rect(
        rect,
        0,
        bg_color,
        Stroke::new(1.0_f32, crate::ui::theme::palette::field_border()),
        egui::StrokeKind::Inside,
    );

    // Draw green fill
    if green_pct_f32 > 0.0 {
        let green_rect =
            egui::Rect::from_min_size(rect.min, vec2(rect.width() * green_pct_f32, rect.height()));
        ui.painter().rect_filled(green_rect, 0, green_color);
    }

    // Draw orange fill (backfiller)
    if orange_pct_f32 > 0.0 {
        let orange_start = rect.min.x + rect.width() * green_pct_f32;
        let orange_rect = egui::Rect::from_min_size(
            pos2(orange_start, rect.min.y),
            vec2(rect.width() * orange_pct_f32, rect.height()),
        );
        ui.painter().rect_filled(orange_rect, 0, orange_color);
    }

    // Overlay text
    let shadow = Color32::BLACK;
    if compact {
        let troop_text = crate::utils::format_number(troops);
        crate::ui::theme::paint_premium_glow_text(
            ui.painter(),
            pos2(rect.left() + 6.0, rect.center().y),
            Align2::LEFT_CENTER,
            &troop_text,
            egui::FontId::proportional(12.5),
            Color32::from_rgb(220, 230, 220),
            shadow,
        );
        let max_text = crate::utils::format_number(max_troops);
        crate::ui::theme::paint_premium_glow_text(
            ui.painter(),
            pos2(rect.right() - 6.0, rect.center().y),
            Align2::RIGHT_CENTER,
            &max_text,
            egui::FontId::proportional(12.5),
            Color32::from_rgb(220, 230, 220),
            shadow,
        );
        let rate_color = if is_increasing {
            crate::ui::theme::palette::neon_cyan_hover()
        } else {
            crate::ui::theme::palette::danger()
        };
        let rate_text = format!("+{}/s", crate::utils::format_number(troop_rate));

        let font_id = egui::FontId::proportional(12.5);
        let galley = ui
            .painter()
            .layout_no_wrap(rate_text.clone(), font_id.clone(), rate_color);
        let icon_size = 11.0;
        let total_w = icon_size + 3.0 + galley.rect.width();
        let mut start_x = rect.center().x - total_w / 2.0;

        crate::widgets::try_paint_emoji(
            ui.painter(),
            "⚔",
            egui::Rect::from_center_size(
                pos2(start_x + icon_size / 2.0, rect.center().y),
                egui::vec2(icon_size, icon_size),
            ),
            Color32::WHITE,
        );
        start_x += icon_size + 3.0;

        crate::ui::theme::paint_premium_glow_text(
            ui.painter(),
            pos2(start_x, rect.center().y),
            Align2::LEFT_CENTER,
            &rate_text,
            font_id,
            rate_color,
            shadow,
        );
    } else {
        let text = format!(
            "{} / {}",
            crate::utils::format_number(troops),
            crate::utils::format_number(max_troops)
        );
        let font_id = egui::FontId::proportional(11.0);
        let galley = ui.painter().layout_no_wrap(
            text.clone(),
            font_id.clone(),
            Color32::from_rgb(220, 230, 220),
        );
        let icon_size = 11.0;
        let total_w = galley.rect.width() + 3.0 + icon_size;
        let mut start_x = rect.center().x - total_w / 2.0;

        crate::ui::theme::paint_premium_glow_text(
            ui.painter(),
            pos2(start_x, rect.center().y),
            Align2::LEFT_CENTER,
            &text,
            font_id,
            Color32::from_rgb(220, 230, 220),
            shadow,
        );
        start_x += galley.rect.width() + 3.0;

        crate::widgets::try_paint_emoji(
            ui.painter(),
            "⚔",
            egui::Rect::from_center_size(
                pos2(start_x + icon_size / 2.0, rect.center().y),
                egui::vec2(icon_size, icon_size),
            ),
            Color32::WHITE,
        );
    }
}

pub(in crate::ui::hud) fn draw_spawn_panel(
    ui: &mut egui::Ui,
    secs: f32,
    compact: bool,
    lang: Language,
) {
    ui.vertical_centered(|ui| {
        if compact {
            ui.add_space(8.0);
        }
        crate::ui::theme::outlined_label(
            ui,
            &sow_i18n::get(lang).hud.spawn_choose_location,
            egui::FontId::proportional(if compact { 16.0 } else { 20.0 }),
            crate::ui::theme::palette::neon_gold_hover(),
        );
        ui.label(
            RichText::new(format!(
                "{:.1}{}",
                secs,
                sow_i18n::get(lang).hud.spawn_seconds_remaining
            ))
            .size(14.0)
            .color(Color32::from_rgb(220, 230, 220)),
        );
        if compact {
            ui.add_space(8.0);
        }
    });
}

pub(in crate::ui::hud) fn draw_persistent_header(
    ui: &mut egui::Ui,
    state: &HudState,
    compact: bool,
    lang: Language,
) {
    if let Some(secs) = state.spawn_timer_secs {
        draw_spawn_panel(ui, secs, compact, lang);
        return;
    }

    let troop_rate = state.troop_rate;
    let is_increasing = true;
    let bar_h = if compact { 24.0 } else { 22.0 };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = if compact { 8.0 } else { 6.0 };

        if !compact {
            let rate_color = if is_increasing {
                crate::ui::theme::palette::neon_cyan_hover()
            } else {
                crate::ui::theme::palette::danger()
            };
            egui::Frame::NONE
                .stroke(Stroke::new(crate::ui::theme::stroke::HAIRLINE, rate_color))
                .corner_radius(crate::ui::theme::radius::sm())
                .inner_margin(egui::Margin::symmetric(
                    crate::ui::theme::margin::COZY,
                    crate::ui::theme::margin::TIGHT,
                ))
                .show(ui, |ui| {
                    crate::widgets::outlined_emoji_label(
                        ui,
                        &format!("⚔ +{}/s", crate::utils::format_number(troop_rate)),
                        egui::FontId::proportional(10.0),
                        rate_color,
                    );
                });
        }

        let bar_w = ui.available_width() - if compact { 100.0 } else { 90.0 };
        let (rect, _) = ui.allocate_exact_size(vec2(bar_w.max(80.0), bar_h), egui::Sense::hover());
        draw_troop_bar(
            ui,
            rect,
            state.troops,
            state.max_troops,
            troop_rate,
            compact,
            is_increasing,
        );

        egui::Frame::NONE
            .stroke(Stroke::new(
                crate::ui::theme::stroke::HAIRLINE,
                crate::ui::theme::palette::neon_gold_hover(),
            ))
            .corner_radius(crate::ui::theme::radius::sm())
            .inner_margin(egui::Margin::symmetric(
                crate::ui::theme::margin::COZY,
                crate::ui::theme::margin::TIGHT,
            ))
            .show(ui, |ui| {
                ui.add(crate::widgets::ResourceLabel::gold(state.gold).font(
                    egui::FontId::proportional(if compact { 11.0 } else { 12.0 }),
                ));
            });
    });
}
