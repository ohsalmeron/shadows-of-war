use egui::{Color32, RichText, Stroke};
use sow_i18n::Language;
use web_time::Instant;

use super::super::state::HudState;

pub(in crate::ui::hud) fn event_log_icon(message: &str) -> &'static str {
    let lower = message.to_lowercase();
    if message.contains('☢') || lower.contains("nuke") || lower.contains("missile") {
        "☢️"
    } else if message.contains('🪙') || lower.contains("gold") {
        "🪙"
    } else if message.contains('❌') || lower.contains("rejected") {
        "❌"
    } else if message.contains('🕊') || lower.contains("eliminated") {
        "🕊️"
    } else if message.contains('🎁') || message.contains('🛡') {
        "🎁"
    } else {
        "•"
    }
}

pub(in crate::ui::hud) fn format_relative_time(at: Instant, lang: Language) -> String {
    let strings = &sow_i18n::get(lang).hud;
    let secs = at.elapsed().as_secs();
    if secs < 60 {
        strings.event_time_seconds.replace("{}", &secs.to_string())
    } else {
        strings
            .event_time_minutes
            .replace("{}", &(secs / 60).to_string())
    }
}

pub(in crate::ui::hud) fn draw_event_log_tab(
    ui: &mut egui::Ui,
    state: &mut HudState,
    width: f32,
    compact: bool,
    lang: Language,
) {
    let strings = &sow_i18n::get(lang).hud;
    let log_h = if compact { 120.0 } else { 140.0 };
    let now = Instant::now();

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(&strings.event_log_title)
                .size(10.0)
                .color(sow_ui_kit::theme::palette::text_muted())
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let clear_btn = crate::widgets::ThemeButton::new(&strings.event_log_clear)
                .style(crate::widgets::ThemeButtonStyle::Tertiary)
                .custom_fill(sow_ui_kit::theme::palette::button_inactive())
                .text_size(10.0);
            if ui.add(clear_btn).clicked() {
                state.event_log.clear();
                state.event_log_seen_count = 0;
            }
        });
    });

    if state.event_log.is_empty() {
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            let icon_rect = egui::Rect::from_center_size(
                ui.cursor().min + egui::vec2(ui.available_width() * 0.5, 14.0),
                egui::vec2(28.0, 28.0),
            );
            if !crate::widgets::try_paint_emoji(ui.painter(), "📋", icon_rect, Color32::GRAY) {
                ui.label(RichText::new("📋").size(28.0).color(Color32::GRAY)); // emoji-ok: fallback when try_paint_emoji misses
            }
            ui.add_space(28.0);
            ui.label(
                RichText::new(&strings.event_log_empty)
                    .size(11.0)
                    .color(Color32::GRAY)
                    .italics(),
            );
        });
        return;
    }

    egui::ScrollArea::vertical()
        .max_height(log_h)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = sow_ui_kit::theme::margin::TIGHT as f32;

            for entry in &state.event_log {
                let age_secs = now.duration_since(entry.spawned_at).as_secs();
                let alpha = if age_secs > 60 { 0.7 } else { 1.0 };
                let icon = event_log_icon(&entry.message);
                let text_color = entry.color.linear_multiply(alpha);
                let stripe = entry.color.linear_multiply(0.9 * alpha);

                egui::Frame::NONE
                    .fill(Color32::from_rgba_unmultiplied(
                        15,
                        10,
                        5,
                        (180.0 * alpha) as u8,
                    ))
                    .stroke(Stroke::new(
                        sow_ui_kit::theme::stroke::HAIRLINE,
                        entry.color.linear_multiply(0.5 * alpha),
                    ))
                    .corner_radius(sow_ui_kit::theme::radius::sm())
                    .inner_margin(egui::Margin::symmetric(
                        sow_ui_kit::theme::margin::COZY,
                        sow_ui_kit::theme::margin::TIGHT,
                    ))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let stripe_rect = egui::Rect::from_min_size(
                                ui.cursor().min,
                                egui::vec2(2.0, if compact { 36.0 } else { 40.0 }),
                            );
                            ui.painter().rect_filled(stripe_rect, 0, stripe);
                            ui.add_space(6.0);

                            ui.label(RichText::new(icon).size(14.0).color(text_color));
                            ui.add_space(4.0);

                            ui.vertical(|ui| {
                                crate::widgets::emoji_label(
                                    ui,
                                    &entry.message,
                                    egui::FontId::proportional(if compact { 10.0 } else { 11.0 }),
                                    text_color,
                                );
                                ui.label(
                                    RichText::new(format_relative_time(entry.spawned_at, lang))
                                        .size(9.0)
                                        .color(sow_ui_kit::theme::palette::text_muted()),
                                );
                            });
                        });
                    });
            }
        });
}
