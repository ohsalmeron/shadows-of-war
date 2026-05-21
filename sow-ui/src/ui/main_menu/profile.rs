use egui::{Color32, CornerRadius, Stroke, Ui};
use super::MainMenuState;

pub fn draw_user_profile_header(
    ui: &mut Ui,
    state: &mut MainMenuState,
    compact: bool,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_lang::Language,
) {
    let strings = &sow_lang::get(lang).main_menu;
    let desired_width = if compact { ui.available_width() } else { 250.0 };
    let desired_height = 56.0;

    let (rect, response) = ui.allocate_exact_size(egui::vec2(desired_width, desired_height), egui::Sense::hover());
    let is_hovered = response.hovered();

    if is_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }

    let bg_color = if is_hovered {
        crate::ui::theme::menu_secondary_button_hover()
    } else {
        crate::ui::theme::menu_secondary_button()
    };
    
    ui.painter().rect_filled(rect, 8.0, bg_color);
    ui.painter().rect_stroke(
        rect,
        8.0,
        Stroke::new(1.0_f32, crate::ui::theme::nickname_field_border()),
        egui::StrokeKind::Inside,
    );

    let avatar_size = 40.0;
    let avatar_rect = egui::Rect::from_min_size(
        egui::pos2(
            rect.min.x + 8.0,
            rect.min.y + (rect.height() - avatar_size) / 2.0,
        ),
        egui::vec2(avatar_size, avatar_size),
    );
    
    let avatar_response = ui.interact(avatar_rect, ui.id().with("avatar_btn"), egui::Sense::click());
    if avatar_response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if avatar_response.clicked() {
        state.show_avatar_picker = true;
    }

    if state.selected_avatar_id < 8 && (state.selected_avatar_id as usize) < asset_loader.avatars.len() {
        let tex = &asset_loader.avatars[state.selected_avatar_id as usize];
        let image = egui::Image::new(tex).fit_to_exact_size(avatar_rect.size()).corner_radius(CornerRadius::same(6));
        ui.put(avatar_rect, image);
    } else if let Some(tex) = &asset_loader.avatar_fallback {
        let image = egui::Image::new(tex).fit_to_exact_size(avatar_rect.size()).corner_radius(CornerRadius::same(6));
        ui.put(avatar_rect, image);
    } else {
        ui.painter()
            .rect_filled(avatar_rect, 6.0, crate::ui::theme::accent_solo_cyan());
    }

    let dot_center = egui::pos2(avatar_rect.max.x - 2.0, avatar_rect.max.y - 2.0);
    ui.painter()
        .circle_filled(dot_center, 4.0, crate::ui::theme::accent_danger());

    let text_edit_rect = egui::Rect::from_min_max(
        egui::pos2(avatar_rect.max.x + 12.0, rect.min.y),
        egui::pos2(rect.max.x - 8.0, rect.max.y),
    );

    ui.scope_builder(egui::UiBuilder::new().max_rect(text_edit_rect), |ui| {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            let output = egui::TextEdit::singleline(&mut state.player_name)
                .id(egui::Id::new("main_menu_nickname"))
                .hint_text(&strings.nickname_hint)
                .char_limit(48)
                .desired_width(ui.available_width())
                .frame(egui::Frame::NONE)
                .font(egui::FontId::proportional(24.0))
                .text_color(Color32::WHITE)
                .show(ui);

            if output.response.gained_focus() {
                if let Some(mut edit_state) = egui::text_edit::TextEditState::load(ui.ctx(), output.response.id) {
                    let char_count = state.player_name.chars().count();
                    let c_start = egui::text::CCursor::new(0);
                    let c_end = egui::text::CCursor::new(char_count);
                    let range = egui::text_selection::CCursorRange::two(c_start, c_end);
                    edit_state.cursor.set_char_range(Some(range));
                    edit_state.store(ui.ctx(), output.response.id);
                }
            }
        });
    });
}

