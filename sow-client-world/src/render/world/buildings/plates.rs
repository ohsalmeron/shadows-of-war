pub(crate) fn upgrade_level_label(level: u8) -> String {
    format!("Lvl {} -> {}", level, level + 1)
}

pub(crate) struct BuildingUpgradePlateLine {
    pub text: String,
    pub color: egui::Color32,
    pub scale: f32, // 1.0 main, 0.85 secondary
}

pub(crate) struct BuildingUpgradePlate {
    pub anchor: egui::Pos2,
    pub base_size: f32,
    pub bobbing: f32,
    pub border_color: egui::Color32,
    pub lines: Vec<BuildingUpgradePlateLine>,
}

pub(crate) fn paint_building_upgrade_plate(
    painter: &egui::Painter,
    plate: BuildingUpgradePlate,
    camera_zoom: f32,
    sf: f32,
) {
    let font_size = (8.0_f32 * camera_zoom / sf).clamp(7.0, 10.0).round();

    let padding_x = 10.0_f32;
    let padding_y = 6.0_f32;
    let column_gap = 6.0_f32;
    let line_gap = 3.0_f32;

    let emoji_size = font_size * 1.4;

    let mut text_w = 0.0_f32;
    let mut text_h = 0.0_f32;
    let mut line_sizes = Vec::new();

    for (i, line) in plate.lines.iter().enumerate() {
        let line_font_size = (font_size * line.scale).round();
        let font_id = egui::FontId::proportional(line_font_size);
        let size = sow_ui_kit::widgets::measure_emoji_text(painter, &line.text, &font_id);
        text_w = text_w.max(size.x);
        if i > 0 {
            text_h += line_gap;
        }
        text_h += size.y;
        line_sizes.push(size);
    }

    let box_w = padding_x * 2.0 + emoji_size + column_gap + text_w;
    let box_h = padding_y * 2.0 + text_h.max(emoji_size);

    let building_top = plate.anchor.y - plate.base_size * 0.5;
    let gap = 4.0_f32; // small air between icon and plate
    let plate_center_y = building_top - gap - box_h * 0.5 + plate.bobbing;
    let badge_rect = egui::Rect::from_center_size(
        egui::pos2(plate.anchor.x, plate_center_y),
        egui::vec2(box_w, box_h),
    );

    painter.rect(
        badge_rect,
        6.0_f32,
        egui::Color32::from_rgba_unmultiplied(15, 23, 42, 210), // Glass slate dark
        egui::Stroke::new(
            1.2_f32,
            egui::Color32::from_rgba_unmultiplied(
                plate.border_color.r(),
                plate.border_color.g(),
                plate.border_color.b(),
                200,
            ),
        ),
        egui::StrokeKind::Inside,
    );

    // Left column: 🏗️ emoji centered
    let emoji_center_x = badge_rect.left() + padding_x + emoji_size * 0.5;
    let emoji_center_y = badge_rect.center().y;
    let emoji_center = egui::pos2(emoji_center_x, emoji_center_y);

    if !sow_ui_kit::widgets::paint_emoji_centered(
        painter,
        "🏗️",
        emoji_center,
        emoji_size,
        egui::Color32::WHITE,
    ) {
        painter.text(
            emoji_center,
            egui::Align2::CENTER_CENTER,
            "🏗️",
            egui::FontId::proportional(emoji_size * 0.7),
            egui::Color32::WHITE,
        );
    }

    // Right column: left-aligned lines
    let text_start_x = badge_rect.left() + padding_x + emoji_size + column_gap;
    let text_start_y = badge_rect.center().y - text_h * 0.5;

    let mut current_y = text_start_y;
    for (i, line) in plate.lines.iter().enumerate() {
        let line_font_size = (font_size * line.scale).round();
        let font_id = egui::FontId::proportional(line_font_size);
        let size = line_sizes[i];

        let line_pos = egui::pos2(text_start_x, current_y + size.y * 0.5);

        sow_ui_kit::widgets::paint_emoji_text_at(
            painter,
            line_pos,
            egui::Align2::LEFT_CENTER,
            &line.text,
            font_id,
            line.color,
            false,
        );

        current_y += size.y + line_gap;
    }
}

pub(crate) fn building_kind_emoji(kind: sow_core::game::BuildingKind) -> &'static str {
    match kind {
        sow_core::game::BuildingKind::City => "🏛️",
        sow_core::game::BuildingKind::Factory => "🏭",
        sow_core::game::BuildingKind::Port => "⚓",
        sow_core::game::BuildingKind::Bunker => "🛡️",
    }
}

pub(crate) fn paint_new_build_ghost(
    painter: &egui::Painter,
    kind: sow_core::game::BuildingKind,
    center: egui::Pos2,
    base_size: f32,
) {
    let rect = egui::Rect::from_center_size(center, egui::vec2(base_size, base_size));
    let emoji = building_kind_emoji(kind);
    if !sow_ui_kit::widgets::try_paint_emoji(painter, emoji, rect, egui::Color32::WHITE) {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            emoji,
            egui::FontId::proportional(base_size * 0.7),
            egui::Color32::WHITE,
        );
    }
}

pub(crate) fn paint_gold_preview_indicator(
    painter: &egui::Painter,
    center: egui::Pos2,
    base_size: f32,
    amount_text: &str,
    text_color: egui::Color32,
    zoom_scaled: f32,
    final_scale: f32,
) {
    let font_size = (zoom_scaled * 0.65 * final_scale).clamp(10.0, 20.0).round();
    let font_id = egui::FontId::proportional(font_size);
    let emoji_size = font_size * 1.4;
    let amount_size = sow_ui_kit::widgets::measure_emoji_text(painter, amount_text, &font_id);
    let gap = 1.0_f32;
    let total_w = emoji_size + gap + amount_size.x;
    let start_x = center.x - total_w * 0.5;
    let indicator_y = center.y + base_size * 0.4;

    sow_ui_kit::widgets::paint_emoji_centered(
        painter,
        "🪙",
        egui::pos2(start_x + emoji_size * 0.5, indicator_y),
        emoji_size,
        egui::Color32::WHITE,
    );

    sow_ui_kit::widgets::paint_emoji_text_at(
        painter,
        egui::pos2(start_x + emoji_size + gap, indicator_y),
        egui::Align2::LEFT_CENTER,
        amount_text,
        font_id,
        text_color,
        true,
    );
}
