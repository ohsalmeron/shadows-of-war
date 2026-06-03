use crate::ui::theme::{
    accent_solo_cyan, accent_solo_cyan_hover, compact_viewport, menu_secondary_button,
    text_secondary, viewport_scale,
};
use crate::UiAction;
use egui::{Align, Color32, Layout, RichText, Slider, Stroke};
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

pub fn draw(root_ui: &mut egui::Ui, state: &mut SettingsState, is_open: bool) -> Option<UiAction> {
    let mut action = None;
    let ctx = root_ui.ctx();
    let compact = compact_viewport(ctx);
    let scale = viewport_scale(ctx);
    let strings = &sow_i18n::get(state.language).settings;

    let progress = ctx.animate_bool_with_time(
        egui::Id::new("settings_animation_progress"),
        is_open,
        crate::ui::theme::anim_duration(state.reduced_motion),
    );
    if progress <= 0.01 && !is_open {
        return None;
    }

    let screen_rect = ctx.content_rect();
    let fade = if state.reduced_motion {
        if is_open { 1.0 } else { progress }
    } else {
        progress
    };

    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("settings_scrim"),
    ))
    .rect_filled(
        screen_rect,
        0.0,
        Color32::from_black_alpha((200.0 * fade) as u8),
    );

    let pad = if compact { 32.0 } else { 50.0 };
    let inner_size = screen_rect.size() - egui::vec2(pad, pad);
    let footer_h = 58.0 * scale;
    let section_gap = (if compact { 16.0 } else { 24.0 }) * scale;

    egui::Area::new(egui::Id::new("settings_modal"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen_rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(screen_rect.size());
            crate::ui::theme::standard_panel_frame(compact).show(ui, |ui| {
                if compact {
                    ui.set_min_height(inner_size.y);
                } else {
                    ui.set_min_size(inner_size);
                }

                ui.horizontal(|ui| {
                    crate::ui::theme::outlined_label(
                        ui,
                        &strings.title,
                        egui::FontId::proportional(if compact { 26.0 } else { 32.0 } * scale),
                        Color32::WHITE,
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if crate::ui::theme::modal_close_button(ui).clicked() {
                            action = Some(UiAction::ToggleSettings);
                        }
                    });
                });

                ui.add_space(8.0 * scale);
                ui.separator();
                ui.add_space(8.0 * scale);

                let middle_h = (ui.available_height() - footer_h - section_gap).max(0.0);

                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), middle_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let quality_btn_w =
                            ((ui.available_width() - 16.0) / 3.0).max(64.0);

                        crate::ui::theme::outlined_label(
                            ui,
                            &strings.graphics_quality,
                            egui::FontId::proportional(18.0 * scale),
                            Color32::WHITE,
                        );
                        ui.add_space(8.0 * scale);
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

                                let btn = egui::Button::new(
                                    RichText::new(label).size(16.0 * scale).color(text_color),
                                )
                                .fill(btn_fill)
                                .stroke(btn_stroke)
                                .min_size(egui::vec2(quality_btn_w, 40.0 * scale));

                                if ui.add(btn).clicked() {
                                    state.graphics_quality = q;
                                    touch_applied(state);
                                }
                            }
                        });
                        ui.label(
                            RichText::new(quality_help(strings, &state.graphics_quality))
                                .size(12.0 * scale)
                                .color(text_secondary()),
                        );

                        ui.add_space(section_gap);

                        crate::ui::theme::outlined_label(
                            ui,
                            &strings.audio,
                            egui::FontId::proportional(18.0 * scale),
                            Color32::WHITE,
                        );
                        ui.add_space(8.0 * scale);

                        ui.horizontal(|ui| {
                            if ui
                                .checkbox(
                                    &mut state.mute_all,
                                    RichText::new(&strings.mute_all)
                                        .size(16.0 * scale)
                                        .color(text_secondary()),
                                )
                                .changed()
                            {
                                touch_applied(state);
                            }
                        });
                        ui.add_space(8.0 * scale);

                        ui.add_enabled_ui(!state.mute_all, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&strings.music_volume)
                                        .size(16.0 * scale)
                                        .color(text_secondary()),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui
                                        .add(Slider::new(
                                            &mut state.music_volume,
                                            0.0..=1.0,
                                        )
                                        .show_value(true))
                                        .changed()
                                    {
                                        touch_applied(state);
                                    }
                                });
                            });
                            ui.add_space(8.0 * scale);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&strings.sfx_volume)
                                        .size(16.0 * scale)
                                        .color(text_secondary()),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui
                                        .add(Slider::new(&mut state.sfx_volume, 0.0..=1.0)
                                            .show_value(true))
                                        .changed()
                                    {
                                        touch_applied(state);
                                    }
                                });
                            });
                        });

                        ui.add_space(section_gap);

                        crate::ui::theme::outlined_label(
                            ui,
                            &strings.language,
                            egui::FontId::proportional(18.0 * scale),
                            Color32::WHITE,
                        );
                        ui.add_space(8.0 * scale);
                        let lang_label = match state.language {
                            Language::English => strings.lang_english.clone(),
                            Language::Spanish => strings.lang_spanish.clone(),
                            _ => strings.lang_english.clone(),
                        };
                        let combo_w = ui.available_width().min(300.0);
                        egui::ComboBox::from_id_salt("language_select")
                            .selected_text(RichText::new(&lang_label).size(16.0 * scale))
                            .width(combo_w)
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

                        if state
                            .applied_hint_until
                            .is_some_and(|t| t.elapsed().as_secs_f32() < 2.0)
                        {
                            ui.add_space(8.0 * scale);
                            ui.label(
                                RichText::new(&strings.settings_applied)
                                    .size(12.0 * scale)
                                    .color(accent_solo_cyan()),
                            );
                        }

                        ui.add_space(8.0 * scale);
                        if ui
                            .checkbox(&mut state.reduced_motion, &strings.reduced_motion)
                            .on_hover_text(&strings.reduced_motion_help)
                            .changed()
                        {
                            touch_applied(state);
                        }

                        ui.add_space(section_gap);

                        ui.horizontal_wrapped(|ui| {
                            let link_btn = |label: &str| {
                                egui::Button::new(
                                    RichText::new(label)
                                        .size(16.0 * scale)
                                        .color(accent_solo_cyan()),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE)
                            };
                            if ui.add(link_btn(&strings.privacy_policy)).clicked() {
                                action = Some(UiAction::TogglePrivacy);
                            }
                            ui.add_space(12.0);
                            if ui.add(link_btn(&strings.terms_of_service)).clicked() {
                                action = Some(UiAction::ToggleTerms);
                            }
                            ui.add_space(12.0);
                            if ui.add(link_btn(&strings.credits_licenses)).clicked() {
                                action = Some(UiAction::ToggleCredits);
                            }
                        });
                    },
                );

                ui.add_space(section_gap);
                ui.vertical_centered(|ui| {
                    let back_btn = crate::widgets::ThemeButton::new(&strings.back_button)
                        .style(crate::widgets::ThemeButtonStyle::Tertiary)
                        .min_size(egui::vec2(
                            if compact {
                                ui.available_width()
                            } else {
                                200.0
                            },
                            50.0 * scale,
                        ));

                    if ui.add(back_btn).clicked() {
                        action = Some(UiAction::ToggleSettings);
                    }
                });
            });
        });

    action
}
