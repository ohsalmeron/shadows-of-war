use crate::ui::theme::palette;
use crate::UiAction;
use egui::{Align, Color32, Frame, Layout, RichText, ScrollArea, Slider, Stroke};
pub use sow_i18n::Language;

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
    pub applied_hint_until: Option<web_time::Instant>,
    pub reduced_motion: bool,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            graphics_quality: GraphicsQuality::High,
            music_volume: 0.8,
            sfx_volume: 0.8,
            mute_all: false,
            language: Language::English,
            applied_hint_until: None,
            reduced_motion: false,
        }
    }
}

fn touch_applied(state: &mut SettingsState) {
    state.applied_hint_until = Some(web_time::Instant::now());
}

fn quality_help<'a>(strings: &'a sow_i18n::SettingsStrings, q: &GraphicsQuality) -> &'a str {
    match q {
        GraphicsQuality::Low => &strings.quality_low_help,
        GraphicsQuality::Medium => &strings.quality_medium_help,
        GraphicsQuality::High => &strings.quality_high_help,
    }
}


pub fn draw(root_ui: &mut egui::Ui, state: &mut SettingsState) -> Option<UiAction> {
    let mut action = None;
    let strings = &sow_i18n::get(state.language).settings;

    egui::Panel::top("settings_header")
        .frame(crate::ui::theme::screen_panel_frame())
        .show_inside(root_ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(&strings.title);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if crate::ui::theme::modal_close_button(ui).clicked() {
                        action = Some(UiAction::ToggleSettings);
                    }
                });
            });
        });

    egui::Panel::bottom("settings_footer")
        .frame(crate::ui::theme::screen_panel_frame())
        .show_inside(root_ui, |ui| {
            let back_btn = crate::widgets::ThemeButton::new(&strings.back_button)
                .style(crate::widgets::ThemeButtonStyle::Tertiary)
                .min_size(egui::vec2(ui.available_width(), 36.0));
            if ui.add(back_btn).clicked() {
                action = Some(UiAction::ToggleSettings);
            }
        });

    egui::CentralPanel::default()
        .frame(Frame::new().fill(egui::Color32::from_rgb(8, 10, 14)))
        .show_inside(root_ui, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("settings_grid")
                        .num_columns(2)
                        .spacing([24.0, 10.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new(&strings.graphics_quality).strong());
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    let qualities = [
                                        (GraphicsQuality::Low, &strings.quality_low),
                                        (GraphicsQuality::Medium, &strings.quality_medium),
                                        (GraphicsQuality::High, &strings.quality_high),
                                    ];
                                    for (q, label) in qualities {
                                        let selected = state.graphics_quality == q;
                                        let btn = egui::Button::new(label)
                                            .fill(if selected {
                                                palette::neon_cyan()
                                            } else {
                                                palette::button_inactive()
                                            })
                                            .stroke(if selected {
                                                Stroke::new(1.0_f32, palette::neon_cyan_hover())
                                            } else {
                                                Stroke::NONE
                                            });
                                        if ui.add(btn).clicked() {
                                            state.graphics_quality = q;
                                            touch_applied(state);
                                        }
                                    }
                                });
                                ui.label(
                                    RichText::new(quality_help(strings, &state.graphics_quality))
                                        .small()
                                        .color(palette::text_muted()),
                                );
                            });
                            ui.end_row();

                            ui.label(RichText::new(&strings.mute_all).color(palette::text_muted()));
                            if ui.checkbox(&mut state.mute_all, "").changed() {
                                touch_applied(state);
                            }
                            ui.end_row();

                            ui.label(RichText::new(&strings.music_volume).color(palette::text_muted()));
                            ui.add_enabled_ui(!state.mute_all, |ui| {
                                if ui
                                    .add(
                                        Slider::new(&mut state.music_volume, 0.0..=1.0)
                                            .show_value(true),
                                    )
                                    .changed()
                                {
                                    touch_applied(state);
                                }
                            });
                            ui.end_row();

                            ui.label(RichText::new(&strings.sfx_volume).color(palette::text_muted()));
                            ui.add_enabled_ui(!state.mute_all, |ui| {
                                if ui
                                    .add(
                                        Slider::new(&mut state.sfx_volume, 0.0..=1.0)
                                            .show_value(true),
                                    )
                                    .changed()
                                {
                                    touch_applied(state);
                                }
                            });
                            ui.end_row();

                            ui.label(RichText::new(&strings.language).strong());
                            let lang_label = match state.language {
                                Language::English => strings.lang_english.clone(),
                                Language::Spanish => strings.lang_spanish.clone(),
                                _ => strings.lang_english.clone(),
                            };
                            egui::ComboBox::from_id_salt("language_select")
                                .selected_text(&lang_label)
                                .width(ui.available_width())
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_value(
                                            &mut state.language,
                                            Language::English,
                                            &strings.lang_english,
                                        )
                                        .clicked()
                                    {
                                        touch_applied(state);
                                    }
                                    if ui
                                        .selectable_value(
                                            &mut state.language,
                                            Language::Spanish,
                                            &strings.lang_spanish,
                                        )
                                        .clicked()
                                    {
                                        touch_applied(state);
                                    }
                                });
                            ui.end_row();

                            ui.label("");
                            if ui
                                .checkbox(&mut state.reduced_motion, &strings.reduced_motion)
                                .on_hover_text(&strings.reduced_motion_help)
                                .changed()
                            {
                                touch_applied(state);
                            }
                            ui.end_row();
                        });

                    if state
                        .applied_hint_until
                        .is_some_and(|t| t.elapsed().as_secs_f32() < 2.0)
                    {
                        ui.label(
                            RichText::new(&strings.settings_applied).color(palette::neon_cyan()),
                        );
                    }

                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        let link = |label: &str| {
                            egui::Button::new(RichText::new(label).color(palette::neon_cyan()))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                        };
                        if ui.add(link(&strings.privacy_policy)).clicked() {
                            action = Some(UiAction::TogglePrivacy);
                        }
                        ui.add_space(12.0);
                        if ui.add(link(&strings.terms_of_service)).clicked() {
                            action = Some(UiAction::ToggleTerms);
                        }
                        ui.add_space(12.0);
                        if ui.add(link(&strings.credits_licenses)).clicked() {
                            action = Some(UiAction::ToggleCredits);
                        }
                    });
                });
        });

    action
}
