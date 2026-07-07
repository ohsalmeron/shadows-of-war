use crate::widgets::{ThemeButton, ThemeButtonStyle};
use crate::UiAction;
use egui::Ui;

pub fn draw_right_column(
    ui: &mut Ui,
    state: &mut crate::ui::main_menu::MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    compact: bool,
    action: &mut Option<UiAction>,
    lang: sow_i18n::Language,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    let scale = sow_ui_kit::theme::viewport_scale(ui.ctx());
    let primary_text = (if compact { 24.0 } else { 28.0 }) * scale;
    let secondary_text = primary_text - 4.0;
    let settings_text = (if compact { 16.0 } else { 18.0 }) * scale;
    let rail_btn_fill = sow_ui_kit::theme::palette::button_inactive();
    let settings_h = action_min_h * 0.75;
    let w = ui.available_width();

    let solo_btn = ThemeButton::new(&strings.single_player)
        .style(ThemeButtonStyle::Tertiary)
        .custom_fill(rail_btn_fill)
        .min_size(egui::vec2(w, action_min_h))
        .text_size(primary_text);
    if ui.add(solo_btn).clicked() {
        state.show_custom_game = true;
        state.custom_game_is_sp = true;
    }

    ui.add_space(section_gap);

    let create_btn = ThemeButton::new(&strings.create_game_btn)
        .style(ThemeButtonStyle::Tertiary)
        .custom_fill(rail_btn_fill)
        .min_size(egui::vec2(w, action_min_h))
        .text_size(secondary_text);
    if ui.add(create_btn).clicked() {
        state.show_custom_game = true;
        state.custom_game_is_sp = false;
    }

    ui.add_space(section_gap);

    let join_btn = ThemeButton::new(&strings.join_game_btn)
        .style(ThemeButtonStyle::Tertiary)
        .custom_fill(rail_btn_fill)
        .min_size(egui::vec2(w, action_min_h))
        .text_size(secondary_text);
    if ui.add(join_btn).clicked() {
        *action = Some(UiAction::OpenJoinBrowser);
    }

    ui.add_space(section_gap);

    let settings_btn = ThemeButton::new(&strings.settings)
        .style(ThemeButtonStyle::Tertiary)
        .custom_fill(rail_btn_fill)
        .min_size(egui::vec2(w, settings_h))
        .text_size(settings_text);
    if ui.add(settings_btn).clicked() {
        *action = Some(UiAction::ToggleSettings);
    }
}
