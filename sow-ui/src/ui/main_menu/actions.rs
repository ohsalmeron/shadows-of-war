use crate::UiAction;
use egui::Ui;
use crate::widgets::{NeonButton, NeonButtonStyle};

pub fn draw_right_column(
    ui: &mut Ui,
    state: &mut crate::ui::main_menu::MainMenuState,
    section_gap: f32,
    action_min_h: f32,
    compact: bool,
    action: &mut Option<UiAction>,
    lang: sow_lang::Language,
) {
    let strings = &sow_lang::get(lang).main_menu;
    let solo_primary = if compact { 24.0 } else { 28.0 };

    let tutorial_btn = NeonButton::new(&strings.play_tutorial)
        .style(NeonButtonStyle::Primary)
        .min_size(egui::vec2(ui.available_width(), action_min_h))
        .text_size(solo_primary);

    if ui.add(tutorial_btn).clicked() {
        *action = Some(UiAction::StartTutorial);
    }

    ui.add_space(section_gap);

    let solo_btn = NeonButton::new(&strings.single_player)
        .style(NeonButtonStyle::Outline)
        .min_size(egui::vec2(ui.available_width(), action_min_h))
        .text_size(solo_primary);

    if ui.add(solo_btn).clicked() {
        state.show_single_player_setup = true;
    }

    ui.add_space(section_gap);

    let ranked = NeonButton::new(&strings.ranked_match)
        .style(NeonButtonStyle::Secondary)
        .min_size(egui::vec2(
            ui.available_width(),
            (action_min_h - 10.0).max(60.0),
        ))
        .text_size(18.0);

    if ui.add(ranked).clicked() {
        log::info!("Ranked match (stub — not implemented)");
    }

    ui.add_space(section_gap);

    let editor_btn = NeonButton::new(&strings.map_editor)
        .style(NeonButtonStyle::Outline)
        .min_size(egui::vec2(
            ui.available_width(),
            (action_min_h - 10.0).max(60.0),
        ))
        .text_size(if compact { 16.0 } else { 18.0 });

    if ui.add(editor_btn).clicked() {
        *action = Some(UiAction::OpenMapEditor);
    }

    ui.add_space(section_gap);

    let h = if compact { 48.0 } else { 52.0 };
    let btn = NeonButton::new(&strings.settings)
        .style(NeonButtonStyle::Outline)
        .min_size(egui::vec2(ui.available_width(), h))
        .text_size(if compact { 16.0 } else { 18.0 });

    if ui.add(btn).clicked() {
        *action = Some(UiAction::ToggleSettings);
    }
}

