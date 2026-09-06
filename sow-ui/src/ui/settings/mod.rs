use crate::UiAction;
use egui::{Color32, RichText, Slider, Stroke};
pub use sow_i18n::Language;
use sow_ui_kit::theme::palette;

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
    pub show_fps_ping: bool,
    pub custom_theme: bool,
    pub is_fullscreen: bool,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            graphics_quality: GraphicsQuality::High,
            music_volume: 0.8,
            sfx_volume: 0.5,
            mute_all: false,
            language: Language::English,
            applied_hint_until: None,
            reduced_motion: false,
            show_fps_ping: false,
            custom_theme: true,
            is_fullscreen: false,
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

pub fn draw(root_ui: &mut egui::Ui, state: &mut SettingsState, is_open: bool) -> Option<UiAction> {
    let mut open = is_open;
    let mut action = None;
    let strings = &sow_i18n::get(state.language).settings;

    let res = sow_ui_kit::theme::draw_standard_modal(
        root_ui.ctx(),
        &mut open,
        "settings",
        &strings.title,
        &strings.back_button,
        state.reduced_motion,
        |ui| {
            egui::Grid::new("settings_grid")
                .num_columns(2)
                .spacing([24.0, 10.0])
                .show(ui, |ui| {
                    ui.label(RichText::new(&strings.graphics_quality).strong());
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.style_mut().spacing.button_padding = egui::vec2(8.0, 4.0);
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
                            .add(Slider::new(&mut state.music_volume, 0.0..=1.0).show_value(true))
                            .changed()
                        {
                            touch_applied(state);
                        }
                    });
                    ui.end_row();

                    // Language picker hidden for launch — freeze on detected locale

                    ui.label(RichText::new(&strings.reduced_motion).color(palette::text_muted()));
                    if ui
                        .checkbox(&mut state.reduced_motion, "")
                        .on_hover_text(&strings.reduced_motion_help)
                        .changed()
                    {
                        touch_applied(state);
                    }
                    ui.end_row();

                    ui.label(RichText::new(&strings.show_fps_ping).color(palette::text_muted()));
                    if ui
                        .checkbox(&mut state.show_fps_ping, "")
                        .on_hover_text(&strings.show_fps_ping_help)
                        .changed()
                    {
                        touch_applied(state);
                    }
                    ui.end_row();

                    ui.label(RichText::new("Custom Theme (Super Rounded & Pitch Black)").strong());
                    if ui.checkbox(&mut state.custom_theme, "").changed() {
                        touch_applied(state);
                    }
                    ui.end_row();

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        ui.label(RichText::new(&strings.fullscreen).color(palette::text_muted()));
                        if ui.checkbox(&mut state.is_fullscreen, "").changed() {
                            touch_applied(state);
                            action = Some(UiAction::SetFullscreen(state.is_fullscreen));
                        }
                        ui.end_row();
                    }
                });

            if state
                .applied_hint_until
                .is_some_and(|t| t.elapsed().as_secs_f32() < 2.0)
            {
                ui.label(RichText::new(&strings.settings_applied).color(palette::neon_cyan()));
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
                ui.add_space(12.0);
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("UI Kit Showcase").color(palette::neon_gold()),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE),
                    )
                    .on_hover_text("Open full-screen UI Kit Component Showcase")
                    .clicked()
                {
                    action = Some(UiAction::ToggleShowcase);
                }
            });

            // Dev tools entry point — only compiled into dev/local-QA builds, never prod.
            // On wasm the leaderboard's dev button is stripped, so this is the way back in.
            #[cfg(any(feature = "dev", debug_assertions))]
            {
                ui.add_space(8.0);
                if ui
                    .add(
                        egui::Button::new(RichText::new("Dev Tools").color(palette::neon_gold()))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                    )
                    .on_hover_text("Toggle the developer sidebar")
                    .clicked()
                {
                    action = Some(UiAction::ToggleDevSidebar);
                }
            }
        },
    );

    if res.is_some() && !open {
        action = Some(UiAction::ToggleSettings);
    }
    action
}
