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
    profile_height: f32,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    action: &mut Option<crate::UiAction>,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let scale = sow_ui_kit::theme::viewport_scale(ui.ctx());

    let parent_w = ui.available_width();
    let desired_width = if compact {
        parent_w
    } else {
        let val = parent_w * 0.35;
        val.clamp(parent_w * 0.30, parent_w * 0.50).max(360.0).min(parent_w)
    };

    ui.allocate_ui_with_layout(
        egui::vec2(desired_width, profile_height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            let header_frame = egui::Frame::NONE
                .fill(sow_ui_kit::theme::palette::button_inactive())
                .stroke(Stroke::new(
                    1.0_f32,
                    sow_ui_kit::theme::palette::field_border(),
                ))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin {
                    left: 8,
                    right: 14,
                    top: 6,
                    bottom: 6,
                });

            header_frame.show(ui, |ui| {
                ui.set_height(profile_height - 12.0);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    // --- 1. Leader Avatar Picker (Button on the left) ---
                    let avatar_size = (profile_height * 0.72).clamp(32.0, 40.0);
                    let (avatar_rect, avatar_response) =
                        ui.allocate_exact_size(egui::vec2(avatar_size, avatar_size), egui::Sense::click());

                    ui.ctx().data_mut(|d| {
                        d.insert_temp(main_menu_avatar_rect_id(), avatar_rect);
                    });

                    if avatar_response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if avatar_response.clicked() {
                        state.show_leader_picker = true;
                    }

                    let btn_bg = if avatar_response.hovered() {
                        sow_ui_kit::theme::palette::field_border()
                    } else {
                        sow_ui_kit::theme::palette::field_bg()
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
                        sow_ui_kit::theme::palette::neon_cyan()
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

                    ui.add_space(8.0 * scale);

                    // Defensive check to ensure nickname doesn't exceed 16 characters
                    if state.player_name.chars().count() > 16 {
                        state.player_name = state.player_name.chars().take(16).collect();
                    }

                    if state.name_locked {
                        ui.label(
                            egui::RichText::new(&state.player_name)
                                .font(egui::FontId::proportional(18.0))
                                .color(Color32::WHITE),
                        );
                        ui.add_space(8.0 * scale);
                    } else {
                        let gap = 8.0 * scale;
                        let btn_text_size = 11.0 * scale;
                        let btn_font = egui::FontId::proportional(btn_text_size);
                        let btn_galley =
                            ui.painter()
                                .layout_no_wrap(strings.sign_in.clone(), btn_font, Color32::WHITE);

                        // Flexible, wider button width
                        let btn_w = btn_galley.size().x + 28.0 * scale;
                        // Flexible, centered height (slightly smaller than avatar height so it fits perfectly)
                        let btn_h = (avatar_size - 6.0 * scale).clamp(24.0, 36.0);

                        let nickname_w = (ui.available_width() - btn_w - gap).max(50.0);

                        let field_frame = egui::Frame::NONE
                            .fill(sow_ui_kit::theme::palette::field_bg())
                            .stroke(Stroke::new(
                                1.0_f32,
                                sow_ui_kit::theme::palette::field_border(),
                            ))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(8, 4));

                        field_frame.show(ui, |ui| {
                            ui.set_width(nickname_w - 16.0);
                            ui.set_height(btn_h - 8.0);
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                let output_name = egui::TextEdit::singleline(&mut state.player_name)
                                    .id(egui::Id::new("main_menu_nickname"))
                                    .hint_text(&strings.nickname_hint)
                                    .char_limit(16)
                                    .desired_width(nickname_w - 32.0 * scale)
                                    .frame(egui::Frame::NONE)
                                    .font(egui::FontId::proportional(16.0))
                                    .text_color(Color32::WHITE)
                                    .show(ui);

                                if output_name.response.gained_focus() {
                                    if let Some(mut edit_state) = egui::text_edit::TextEditState::load(
                                        ui.ctx(),
                                        output_name.response.id,
                                    ) {
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

                        ui.add_space(gap);

                        let sign_in_btn = crate::widgets::ThemeButton::new(&strings.sign_in)
                            .style(crate::widgets::ThemeButtonStyle::Secondary)
                            .min_size(egui::vec2(btn_w, btn_h))
                            .text_size(btn_text_size);
                        if ui.add(sign_in_btn).clicked() {
                            *action = Some(crate::UiAction::PortalShowAuthPrompt);
                        }
                        ui.add_space(8.0 * scale);
                    }
                });
            });
        },
    );
}
