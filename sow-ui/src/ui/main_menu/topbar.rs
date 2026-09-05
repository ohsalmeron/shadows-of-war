use super::MainMenuState;
use crate::ui::asset_loader::AssetLoader;
use egui::{Color32, Rect, Sense, Stroke, Ui};

const AVATAR_SIZE: f32 = 44.0;
const TOPBAR_HEIGHT: f32 = 44.0;
const AVATAR_RECT_KEY: &str = "main_menu_avatar_rect";

pub fn draw(
    ui: &mut Ui,
    state: &mut MainMenuState,
    asset_loader: &AssetLoader,
    lang: sow_i18n::Language,
    action: &mut Option<crate::UiAction>,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let metrics = super::layout::main_menu_metrics(ui.ctx());
    let phone = metrics.is_phone();
    let gap = if phone { 6.0 } else { 10.0 };
    let sign_in_width = if phone { 74.0 } else { 88.0 };
    egui::Frame::NONE
        .fill(Color32::from_rgba_unmultiplied(12, 14, 18, 220))
        .stroke(Stroke::new(1.0_f32, Color32::from_white_alpha(41)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(if phone { 8 } else { 10 }, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(TOPBAR_HEIGHT);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                draw_avatar(ui, state, asset_loader);
                draw_identity(ui, state, strings, if phone { 112.0 } else { 190.0 }, phone, action);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (settings_rect, settings_response) = ui.allocate_exact_size(
                        egui::vec2(AVATAR_SIZE, 42.0),
                        egui::Sense::click(),
                    );
                    paint_settings_icon(ui, settings_rect, settings_response.hovered());
                    if settings_response.clicked() {
                        *action = Some(crate::UiAction::ToggleSettings);
                    }

                    let account_label = if state.name_locked {
                        "ACCOUNT"
                    } else {
                        strings.sign_in.as_str()
                    };
                    let sign_in = crate::widgets::ThemeButton::new(account_label)
                        .style(crate::widgets::ThemeButtonStyle::Secondary)
                        .min_size(egui::vec2(sign_in_width, 42.0))
                        .text_size(12.0);
                    if ui.add(sign_in).clicked() {
                        *action = Some(crate::UiAction::PortalShowAuthPrompt);
                    }

                    if !phone && draw_progression(ui, state).clicked() {
                        *action = Some(crate::UiAction::OpenProfilePage);
                    }
                });
            });
        });
}

fn draw_avatar(ui: &mut Ui, state: &mut MainMenuState, asset_loader: &AssetLoader) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(AVATAR_SIZE, AVATAR_SIZE), Sense::click());
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::new(AVATAR_RECT_KEY), rect));
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        state.show_leader_picker = true;
    }

    let rgb = state.selected_leader.filler_rgb();
    let fill = Color32::from_rgb(
        (rgb[0] * 255.0).round() as u8,
        (rgb[1] * 255.0).round() as u8,
        (rgb[2] * 255.0).round() as u8,
    );
    ui.painter().rect_filled(rect, 8.0, fill);
    if let Some(texture) = asset_loader.avatars.get(&state.selected_leader) {
        ui.put(
            rect,
            egui::Image::new(texture)
                .fit_to_exact_size(rect.size())
                .corner_radius(egui::CornerRadius::same(8)),
        );
    }
    ui.painter().rect_stroke(
        rect,
        8.0,
        Stroke::new(
            if response.hovered() { 1.5_f32 } else { 1.0_f32 },
            if response.hovered() {
                crate::kit::theme::palette::neon_gold()
            } else {
                Color32::from_rgba_unmultiplied(243, 177, 43, 184)
            },
        ),
        egui::StrokeKind::Inside,
    );
    let dot = egui::pos2(rect.max.x - 2.0, rect.max.y - 2.0);
    ui.painter()
        .circle_filled(dot, 6.0, Color32::from_rgb(12, 14, 18));
    ui.painter()
        .circle_filled(dot, 4.0, Color32::from_rgb(34, 197, 94));
}

fn draw_identity(
    ui: &mut Ui,
    state: &mut MainMenuState,
    strings: &sow_i18n::MainMenuStrings,
    identity_width: f32,
    phone: bool,
    action: &mut Option<crate::UiAction>,
) {
    if state.player_name.chars().count() > 16 {
        state.player_name = state.player_name.chars().take(16).collect();
    }
    let field_width = identity_width.min(if phone { 132.0 } else { 180.0 }).max(48.0);
    ui.allocate_ui_with_layout(
        egui::vec2(identity_width, TOPBAR_HEIGHT),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if state.name_locked {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(field_width, 25.0), Sense::hover());
                ui.painter().with_clip_rect(rect).text(
                    rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    state.player_name.to_uppercase(),
                    egui::FontId::proportional(15.0),
                    Color32::WHITE,
                );
                ui.painter().line_segment(
                    [rect.left_bottom(), rect.right_bottom()],
                    Stroke::new(1.0_f32, Color32::from_white_alpha(56)),
                );
            } else {
                let output = ui.add_sized(
                    [field_width, 25.0],
                    egui::TextEdit::singleline(&mut state.player_name)
                        .id(egui::Id::new("main_menu_nickname"))
                        .hint_text(&strings.nickname_hint)
                        .char_limit(16)
                        .frame(egui::Frame::NONE)
                        .font(egui::FontId::proportional(15.0))
                        .text_color(Color32::WHITE),
                );
                ui.painter().line_segment(
                    [output.rect.left_bottom(), output.rect.right_bottom()],
                    Stroke::new(
                        1.0_f32,
                        if output.has_focus() {
                            crate::kit::theme::palette::neon_gold()
                        } else {
                            Color32::from_white_alpha(56)
                        },
                    ),
                );
                if output.gained_focus() {
                    if let Some(mut edit_state) =
                        egui::text_edit::TextEditState::load(ui.ctx(), output.id)
                    {
                        let range = egui::text_selection::CCursorRange::two(
                            egui::text::CCursor::new(0),
                            egui::text::CCursor::new(state.player_name.chars().count()),
                        );
                        edit_state.cursor.set_char_range(Some(range));
                        edit_state.store(ui.ctx(), output.id);
                    }
                }
                if output.lost_focus()
                    || (output.has_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                {
                    *action = Some(crate::UiAction::SaveDisplayName(state.player_name.clone()));
                }
            }

            let label = if phone {
                state.selected_leader.name().to_uppercase()
            } else {
                format!(
                    "{} · {}",
                    state.selected_leader.name(),
                    state.selected_civilization.name()
                )
                .to_uppercase()
            };
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(field_width, 17.0), Sense::click());
            ui.painter().with_clip_rect(rect).text(
                rect.left_center(),
                egui::Align2::LEFT_CENTER,
                label,
                egui::FontId::proportional(10.0),
                crate::kit::theme::palette::neon_gold(),
            );
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if response.clicked() {
                *action = Some(crate::UiAction::OpenProfilePage);
            }
        },
    );
}

fn draw_progression(ui: &mut Ui, state: &MainMenuState) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(176.0, 42.0), Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    ui.painter().rect_filled(
        rect,
        8.0,
        Color32::from_rgba_unmultiplied(20, 23, 31, 238),
    );
    ui.painter().rect_stroke(
        rect,
        8.0,
        Stroke::new(
            1.0_f32,
            if response.hovered() {
                Color32::from_rgba_unmultiplied(243, 177, 43, 150)
            } else {
                Color32::from_rgba_unmultiplied(243, 177, 43, 76)
            },
        ),
        egui::StrokeKind::Inside,
    );

    let level_right = rect.min.x + 48.0;
    let xp_right = level_right + 78.0;
    let divider = Stroke::new(1.0_f32, Color32::from_white_alpha(30));
    ui.painter().line_segment(
        [
            egui::pos2(level_right, rect.min.y + 6.0),
            egui::pos2(level_right, rect.max.y - 6.0),
        ],
        divider,
    );
    ui.painter().line_segment(
        [
            egui::pos2(xp_right, rect.min.y + 6.0),
            egui::pos2(xp_right, rect.max.y - 6.0),
        ],
        divider,
    );
    ui.painter().text(
        egui::pos2(rect.min.x + 9.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "LV",
        egui::FontId::proportional(9.0),
        crate::kit::theme::palette::neon_gold(),
    );
    ui.painter().text(
        egui::pos2(rect.min.x + 28.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        state.account_level.to_string(),
        egui::FontId::proportional(12.0),
        Color32::WHITE,
    );
    ui.painter().text(
        egui::pos2(level_right + 10.0, rect.min.y + 13.0),
        egui::Align2::LEFT_CENTER,
        format!("{} XP", state.account_xp),
        egui::FontId::proportional(11.0),
        crate::kit::theme::palette::neon_cyan(),
    );
    let track = Rect::from_min_size(
        egui::pos2(level_right + 10.0, rect.max.y - 11.0),
        egui::vec2(58.0, 3.0),
    );
    ui.painter().rect_filled(
        track,
        2.0,
        Color32::from_rgba_unmultiplied(56, 189, 248, 46),
    );
    let fill = Rect::from_min_size(
        track.min,
        egui::vec2(track.width() * ((state.account_xp % 100) as f32 / 100.0), track.height()),
    );
    ui.painter()
        .rect_filled(fill, 2.0, crate::kit::theme::palette::neon_cyan());
    ui.painter().text(
        egui::pos2(xp_right + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("{} L", state.laurels),
        egui::FontId::proportional(12.0),
        crate::kit::theme::palette::neon_gold(),
    );
    response
}

fn paint_settings_icon(ui: &mut Ui, rect: Rect, hovered: bool) {
    let color = if hovered {
        crate::kit::theme::palette::neon_cyan()
    } else {
        crate::kit::theme::palette::text_muted()
    };
    let stroke = Stroke::new(2.0_f32, color);
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(8),
        if hovered {
            Color32::from_white_alpha(18)
        } else {
            Color32::TRANSPARENT
        },
        Stroke::new(1.0_f32, Color32::from_white_alpha(24)),
        egui::StrokeKind::Inside,
    );
    ui.painter().circle_stroke(rect.center(), 8.0, stroke);
    ui.painter().circle_filled(rect.center(), 3.0, color);
    for (dx, dy) in [(0.0, -11.0), (0.0, 11.0), (-11.0, 0.0), (11.0, 0.0)] {
        let center = rect.center() + egui::vec2(dx, dy);
        ui.painter().line_segment(
            [center - egui::vec2(if dx == 0.0 { 2.0 } else { 0.0 }, if dy == 0.0 { 2.0 } else { 0.0 }),
             center + egui::vec2(if dx == 0.0 { 2.0 } else { 0.0 }, if dy == 0.0 { 2.0 } else { 0.0 })],
            stroke,
        );
    }
}
