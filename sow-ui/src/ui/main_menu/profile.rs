use super::MainMenuState;
use egui::{Color32, Rect, Stroke, Ui};

const MAIN_MENU_AVATAR_RECT_KEY: &str = "main_menu_avatar_rect";

fn main_menu_avatar_rect_id() -> egui::Id {
    egui::Id::new(MAIN_MENU_AVATAR_RECT_KEY)
}

/// Screen rect of the main-menu leader avatar button (same frame the picker opens from).
pub fn main_menu_avatar_button_rect(ctx: &egui::Context) -> Rect {
    let id = main_menu_avatar_rect_id();
    if let Some(rect) = ctx.data(|d| d.get_temp::<Rect>(id)) {
        return rect;
    }
    // Fallback mirrors [`draw_user_profile_header`] layout at scroll top.
    const OUTER_PAD: f32 = 16.0;
    const AVATAR_SIZE: f32 = 40.0;
    const PROFILE_H: f32 = 56.0;
    let screen = ctx.content_rect();
    Rect::from_min_size(
        egui::pos2(
            screen.min.x + OUTER_PAD + 8.0,
            screen.min.y + OUTER_PAD + (PROFILE_H - AVATAR_SIZE) * 0.5,
        ),
        egui::vec2(AVATAR_SIZE, AVATAR_SIZE),
    )
}

pub fn draw_user_profile_header(
    ui: &mut Ui,
    state: &mut MainMenuState,
    compact: bool,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let desired_width = if compact { ui.available_width() } else { 280.0 };
    let desired_height = 56.0;

    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(desired_width, desired_height),
        egui::Sense::hover(),
    );
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

    // --- 1. Leader Avatar Picker (Button on the left) ---
    let avatar_size = 40.0;
    let avatar_rect = egui::Rect::from_min_size(
        egui::pos2(
            rect.min.x + 8.0,
            rect.min.y + (rect.height() - avatar_size) / 2.0,
        ),
        egui::vec2(avatar_size, avatar_size),
    );

    ui.ctx().data_mut(|d| {
        d.insert_temp(main_menu_avatar_rect_id(), avatar_rect);
    });

    let avatar_response = ui.interact(
        avatar_rect,
        ui.id().with("avatar_btn"),
        egui::Sense::click(),
    );
    if avatar_response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if avatar_response.clicked() {
        state.show_leader_picker = true;
    }

    let btn_bg = if avatar_response.hovered() {
        crate::ui::theme::nickname_field_border()
    } else {
        crate::ui::theme::nickname_field_bg()
    };

    ui.painter().rect_filled(avatar_rect, 6.0, btn_bg);

    let leader_rgb = state.selected_leader.filler_rgb();
    let leader_fill = egui::Color32::from_rgb(
        (leader_rgb[0] * 255.0).round() as u8,
        (leader_rgb[1] * 255.0).round() as u8,
        (leader_rgb[2] * 255.0).round() as u8,
    );
    ui.painter().rect_filled(avatar_rect, 6.0, leader_fill);

    // Render the pre-loaded high-quality avatar image texture
    if let Some(tex) = asset_loader.avatars.get(&state.selected_leader) {
        let image = egui::Image::new(tex)
            .fit_to_exact_size(avatar_rect.size())
            .corner_radius(egui::CornerRadius::same(6));
        ui.put(avatar_rect, image);
    }

    let frame_color = if avatar_response.hovered() {
        crate::ui::theme::accent_solo_cyan()
    } else {
        leader_fill
    };
    ui.painter().rect_stroke(
        avatar_rect,
        6.0,
        Stroke::new(
            if avatar_response.hovered() {
                1.5_f32
            } else {
                1.0_f32
            },
            frame_color,
        ),
        egui::StrokeKind::Inside,
    );

    // Green online indicator dot
    let dot_center = egui::pos2(avatar_rect.max.x - 2.0, avatar_rect.max.y - 2.0);
    ui.painter()
        .circle_filled(dot_center, 4.0, Color32::from_rgb(34, 197, 94));

    // --- 2. Compact Text Column (Name only, vertically centered) ---
    let text_rect = egui::Rect::from_min_max(
        egui::pos2(avatar_rect.max.x + 12.0, rect.min.y),
        egui::pos2(rect.max.x - 8.0, rect.max.y),
    );

    ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;

            // Nickname Field
            let output_name = egui::TextEdit::singleline(&mut state.player_name)
                .id(egui::Id::new("main_menu_nickname"))
                .hint_text(&strings.nickname_hint)
                .char_limit(48)
                .desired_width(ui.available_width() - 4.0)
                .frame(egui::Frame::NONE)
                .font(egui::FontId::proportional(18.0))
                .text_color(Color32::WHITE)
                .show(ui);

            if output_name.response.gained_focus() {
                if let Some(mut edit_state) =
                    egui::text_edit::TextEditState::load(ui.ctx(), output_name.response.id)
                {
                    let char_count = state.player_name.chars().count();
                    let range = egui::text_selection::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(char_count),
                    );
                    edit_state.cursor.set_char_range(Some(range));
                    edit_state.store(ui.ctx(), output_name.response.id);
                }
            }
        });
    });
}
