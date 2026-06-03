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
    let solo_primary = if compact { 24.0 } else { 28.0 };
    let rail_btn_fill = crate::ui::theme::menu_secondary_button();
    let secondary_h = (action_min_h - 10.0).max(action_min_h * 0.875);
    let settings_h = action_min_h * 0.75;

    let solo_btn = ThemeButton::new(&strings.single_player)
        .style(ThemeButtonStyle::Tertiary)
        .custom_fill(rail_btn_fill)
        .min_size(egui::vec2(ui.available_width(), action_min_h))
        .text_size(solo_primary);

    if ui.add(solo_btn).clicked() {
        state.show_single_player_setup = true;
    }

    ui.add_space(section_gap);

    let tutorial_btn = ThemeButton::new(&strings.play_tutorial)
        .style(ThemeButtonStyle::Tertiary)
        .custom_fill(rail_btn_fill)
        .min_size(egui::vec2(ui.available_width(), secondary_h))
        .text_size(if compact { 16.0 } else { 18.0 });

    if ui.add(tutorial_btn).clicked() {
        *action = Some(UiAction::StartTutorial);
    }

    ui.add_space(section_gap);

    let btn = ThemeButton::new(&strings.settings)
        .style(ThemeButtonStyle::Tertiary)
        .custom_fill(rail_btn_fill)
        .min_size(egui::vec2(ui.available_width(), settings_h))
        .text_size(if compact { 16.0 } else { 18.0 });

    if ui.add(btn).clicked() {
        *action = Some(UiAction::ToggleSettings);
    }
}
