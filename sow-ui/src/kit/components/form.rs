use egui::{Color32, FontId, Response, Sense, Stroke, Ui, Vec2, vec2};
use sow_ui_kit::theme::palette;
use sow_ui_kit::theme::radius;
use std::ops::RangeInclusive;

pub fn checkbox(ui: &mut Ui, value: &mut bool, label: &str) -> Response {
    let box_size = 18.0;
    let font_id = FontId::proportional(14.0);
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        font_id,
        if *value {
            Color32::WHITE
        } else {
            palette::text_muted()
        },
    );

    let total_w = box_size + 8.0 + galley.size().x;
    let total_h = box_size.max(galley.size().y);

    let (rect, mut response) = ui.allocate_exact_size(vec2(total_w, total_h), Sense::click());

    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let box_rect = egui::Rect::from_min_size(
            rect.left_top() + vec2(0.0, (total_h - box_size) * 0.5),
            Vec2::splat(box_size),
        );

        let (fill, stroke) = if *value {
            (
                palette::neon_cyan(),
                Stroke::new(1.0_f32, palette::neon_cyan_hover()),
            )
        } else if response.hovered() {
            (
                palette::button_hovered(),
                Stroke::new(1.0_f32, palette::neon_cyan_glow()),
            )
        } else {
            (
                palette::field_bg(),
                Stroke::new(1.0_f32, palette::field_border()),
            )
        };

        ui.painter().rect(
            box_rect,
            radius::xs(),
            fill,
            stroke,
            egui::StrokeKind::Inside,
        );

        if *value {
            let p1 = box_rect.min + vec2(4.0, 9.0);
            let p2 = box_rect.min + vec2(7.0, 13.0);
            let p3 = box_rect.min + vec2(14.0, 5.0);
            ui.painter()
                .line_segment([p1, p2], Stroke::new(2.0_f32, Color32::BLACK));
            ui.painter()
                .line_segment([p2, p3], Stroke::new(2.0_f32, Color32::BLACK));
        }

        let text_pos = egui::pos2(
            box_rect.max.x + 8.0,
            rect.top() + (total_h - galley.size().y) * 0.5,
        );
        if *value {
            sow_ui_kit::theme::paint_premium_glow_galley(
                ui.painter(),
                text_pos,
                galley,
                Color32::WHITE,
                Color32::BLACK,
            );
        } else {
            ui.painter().galley(text_pos, galley, palette::text_muted());
        }
    }

    response
}

pub fn slider(ui: &mut Ui, value: &mut f32, range: RangeInclusive<f32>, label: &str) -> Response {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(value, range).show_value(true))
    })
    .inner
}
