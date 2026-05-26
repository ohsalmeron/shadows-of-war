use crate::ui::theme::{
    accent_solo_cyan, accent_solo_cyan_hover, menu_secondary_button, text_secondary,
};
use crate::UiAction;
use egui::{Align, Color32, Layout, RichText, Slider, Stroke};
pub use sow_lang::Language;

#[derive(Debug, Clone, PartialEq)]
pub enum GraphicsQuality {
    Low,
    Medium,
    High,
}

pub struct SettingsState {
    pub graphics_quality: GraphicsQuality,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub mute_all: bool,
    pub language: Language,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            graphics_quality: GraphicsQuality::High,
            music_volume: 0.8,
            sfx_volume: 0.8,
            mute_all: false,
            language: Language::English,
        }
    }
}

pub fn draw(root_ui: &mut egui::Ui, state: &mut SettingsState, is_open: bool) -> Option<UiAction> {
    let mut action = None;
    let compact = root_ui.ctx().content_rect().width() < 900.0;
    let panel_w = if compact {
        root_ui.ctx().content_rect().width()
    } else {
        520.0
    };

    let strings = &sow_lang::get(state.language).settings;

    let progress = root_ui.ctx().animate_bool_with_time(
        egui::Id::new("settings_animation_progress"),
        is_open,
        0.22,
    );
    if progress <= 0.01 && !is_open {
        return None;
    }

    // Dark scrim behind the modal
    let screen_rect = root_ui.ctx().content_rect();
    root_ui
        .ctx()
        .layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("settings_scrim"),
        ))
        .rect_filled(
            screen_rect,
            0.0,
            Color32::from_black_alpha((200.0 * progress) as u8),
        );

    // Disney overshoot curve (open) / cubic ease-out (close)
    let anim_scale = if is_open {
        let t = progress;
        if t >= 1.0 {
            1.0
        } else {
            1.0 - (t * 7.5).cos() * (-3.5 * t).exp()
        }
    } else {
        // Cubic ease-out: fast start, smooth deceleration to zero
        let t = progress.clamp(0.0, 1.0);
        t * t * t
    };

    // Slide down from above center (-300px) and bounce
    let y_offset = -300.0 * (1.0 - anim_scale);

    egui::Window::new("settings_modal")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, y_offset))
        .fixed_size(egui::vec2(panel_w, 0.0))
        .frame(crate::ui::theme::standard_panel_frame(compact))
        .show(root_ui.ctx(), |ui| {
            // Header
            ui.horizontal(|ui| {
                crate::ui::theme::outlined_label(
                    ui,
                    &strings.title,
                    egui::FontId::proportional(if compact { 26.0 } else { 32.0 }),
                    Color32::WHITE,
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("✖").size(20.0).color(text_secondary()),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                        )
                        .clicked()
                    {
                        action = Some(UiAction::ToggleSettings);
                    }
                });
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(16.0);

            // --- Graphics ---
            crate::ui::theme::outlined_label(
                ui,
                &strings.graphics_quality,
                egui::FontId::proportional(18.0),
                Color32::WHITE,
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let qualities = [
                    (GraphicsQuality::Low, &strings.quality_low),
                    (GraphicsQuality::Medium, &strings.quality_medium),
                    (GraphicsQuality::High, &strings.quality_high),
                ];

                for (q, label) in qualities {
                    let is_selected = state.graphics_quality == q;
                    let btn_fill = if is_selected {
                        accent_solo_cyan()
                    } else {
                        menu_secondary_button()
                    };
                    let text_color = if is_selected {
                        Color32::BLACK
                    } else {
                        Color32::WHITE
                    };
                    let btn_stroke = if is_selected {
                        Stroke::new(1.0_f32, accent_solo_cyan_hover())
                    } else {
                        Stroke::new(1.0_f32, Color32::from_gray(80))
                    };

                    let btn = egui::Button::new(RichText::new(label).size(16.0).color(text_color))
                        .fill(btn_fill)
                        .stroke(btn_stroke)
                        .min_size(egui::vec2(if compact { 80.0 } else { 120.0 }, 40.0));

                    if ui.add(btn).clicked() {
                        state.graphics_quality = q;
                    }
                }
            });

            ui.add_space(24.0);

            // --- Audio ---
            crate::ui::theme::outlined_label(
                ui,
                &strings.audio,
                egui::FontId::proportional(18.0),
                Color32::WHITE,
            );
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut state.mute_all,
                    RichText::new(&strings.mute_all)
                        .size(16.0)
                        .color(text_secondary()),
                );
            });
            ui.add_space(12.0);

            ui.add_enabled_ui(!state.mute_all, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&strings.music_volume)
                            .size(16.0)
                            .color(text_secondary()),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(Slider::new(&mut state.music_volume, 0.0..=1.0).show_value(false));
                    });
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&strings.sfx_volume)
                            .size(16.0)
                            .color(text_secondary()),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(Slider::new(&mut state.sfx_volume, 0.0..=1.0).show_value(false));
                    });
                });
            });

            ui.add_space(24.0);

            // --- Language ---
            crate::ui::theme::outlined_label(
                ui,
                &strings.language,
                egui::FontId::proportional(18.0),
                Color32::WHITE,
            );
            ui.add_space(8.0);
            egui::ComboBox::from_id_salt("language_select")
                .selected_text(RichText::new(format!("{:?}", state.language)).size(16.0))
                .width(if compact { 200.0 } else { 300.0 })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.language, Language::English, "English");
                    ui.selectable_value(&mut state.language, Language::Spanish, "Spanish");
                    ui.selectable_value(&mut state.language, Language::French, "French");
                    ui.selectable_value(&mut state.language, Language::German, "German");
                });

            ui.add_space(32.0);

            // --- Back Button ---
            ui.vertical_centered(|ui| {
                let back_btn = crate::widgets::ThemeButton::new(&strings.back_button)
                    .style(crate::widgets::ThemeButtonStyle::Tertiary)
                    .min_size(egui::vec2(
                        if compact { ui.available_width() } else { 200.0 },
                        50.0,
                    ));

                if ui.add(back_btn).clicked() {
                    action = Some(UiAction::ToggleSettings);
                }
            });
        });

    action
}
