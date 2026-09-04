use super::MainMenuState;
use crate::kit::components::{Button, Card, Heading};
use egui::{Color32, Rect, RichText, Stroke, Ui};
use sow_ui_kit::theme::palette;

/// Native profile data is a presentation snapshot. Network requests stay in sow-client.
#[derive(Default)]
pub struct NativeProfileState {
    pub public_id: Option<String>,
    pub view: Option<sow_data::profile::PublicProfileView>,
    pub history: Vec<sow_data::profile::PublicMatchSummary>,
    pub ratings: Vec<sow_data::profile::PublicRatingView>,
    pub search_results: Vec<sow_data::profile::PublicProfileSummary>,
    pub search_query: String,
    pub history_cursor: usize,
    pub history_has_next: bool,
    pub match_detail: Option<sow_data::profile::PublicMatchDetail>,
    pub ratings_loaded: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub active_tab: NativeProfileTab,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeProfileTab {
    #[default]
    Overview,
    Leaders,
    History,
    Ranked,
}

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
    const OUTER_AVATAR_SIZE: f32 = 40.0;
    const PROFILE_H: f32 = 56.0;
    let screen = ctx.content_rect();
    Rect::from_min_size(
        egui::pos2(
            screen.min.x + OUTER_PAD + 8.0,
            screen.min.y + OUTER_PAD + (PROFILE_H - OUTER_AVATAR_SIZE) * 0.5,
        ),
        egui::vec2(OUTER_AVATAR_SIZE, OUTER_AVATAR_SIZE),
    )
}

pub fn draw_user_profile_header(
    ui: &mut Ui,
    state: &mut MainMenuState,
    profile_height: f32,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    lang: sow_i18n::Language,
    action: &mut Option<crate::UiAction>,
) {
    let strings = &sow_i18n::get(lang).main_menu;
    if super::layout::main_menu_metrics(ui.ctx()).is_phone() {
        draw_phone_profile_header(ui, state, asset_loader, strings, action);
        return;
    }

    // Use right_padding = 0.0 to perfectly align with the left padding of CentralPanel.
    // Both edges will have exactly 16px gap from the screen border.
    let right_padding = 0.0;
    let desired_width = ui.available_width() - right_padding;
    let frame_height = profile_height.max(56.0);

    let header_frame = egui::Frame::NONE
        .fill(sow_ui_kit::theme::palette::button_inactive())
        .stroke(Stroke::new(
            1.0_f32,
            sow_ui_kit::theme::palette::field_border(),
        ))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(8, 6));

    // Allocate exact rect and create a child UI to enforce the width boundary
    let (rect, _response) = ui.allocate_exact_size(
        egui::vec2(desired_width, frame_height),
        egui::Sense::hover(),
    );
    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );

    header_frame.show(&mut child_ui, |ui| {
        ui.set_height(frame_height);
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            // Reduce item spacing between children inside the row
            ui.spacing_mut().item_spacing.x = 6.0;

            // --- 1. Leader Avatar Picker (Button on the left) ---
            let avatar_size = (profile_height * 0.78).clamp(40.0, 44.0);
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

            // Defensive check to ensure nickname doesn't exceed 16 characters
            if state.player_name.chars().count() > 16 {
                state.player_name = state.player_name.chars().take(16).collect();
            }

            if state.name_locked {
                // Signed into a portal: show the platform avatar (e.g. CrazyGames
                // profile picture) next to the locked name.
                if let Some(tex) = asset_loader.portal_avatar.as_ref() {
                    ui.add(
                        egui::Image::new(tex)
                            .fit_to_exact_size(egui::vec2(24.0, 24.0))
                            .corner_radius(egui::CornerRadius::same(12)),
                    );
                }
                ui.label(
                    egui::RichText::new(&state.player_name)
                        .font(egui::FontId::proportional(18.0))
                        .color(Color32::WHITE),
                );
            } else {
                let btn_text_size = 18.0;
                let btn_font = egui::FontId::proportional(btn_text_size);
                let btn_galley =
                    ui.painter()
                        .layout_no_wrap(strings.sign_in.clone(), btn_font, Color32::WHITE);

                // Flexible, wider button width
                let btn_w = btn_galley.size().x + 42.0;
                // Flexible, centered height (slightly smaller than avatar height so it fits perfectly)
                let btn_h = 44.0;

                // Reserve the progression badge before sizing the editable name field.
                // This is the critical invariant that prevents the 848px shell from
                // painting the level badge over STORE/PROFILE.
                let progression_w = 188.0;
                let nickname_w = (ui.available_width() - btn_w - progression_w - 18.0).max(48.0);

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
                            .desired_width(nickname_w - 32.0)
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
                        if output_name.response.lost_focus()
                            || (output_name.response.has_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                        {
                            *action =
                                Some(crate::UiAction::SaveDisplayName(state.player_name.clone()));
                        }
                    });
                });

                let sign_in_btn = crate::widgets::ThemeButton::new(&strings.sign_in)
                    .style(crate::widgets::ThemeButtonStyle::Secondary)
                    .min_size(egui::vec2(btn_w, btn_h))
                    .text_size(btn_text_size);
                if ui.add(sign_in_btn).clicked() {
                    *action = Some(crate::UiAction::PortalShowAuthPrompt);
                }
            }

            let prog_frame = egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(26, 30, 40, 200))
                .stroke(Stroke::new(
                    1.0_f32,
                    egui::Color32::from_rgba_unmultiplied(243, 177, 43, 80),
                ))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(8, 4));
            prog_frame.show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(
                        egui::RichText::new(format!("LV {}", state.account_level))
                            .font(egui::FontId::proportional(12.0))
                            .strong()
                            .color(egui::Color32::WHITE),
                    );
                    ui.label(
                        egui::RichText::new("·")
                            .font(egui::FontId::proportional(12.0))
                            .color(sow_ui_kit::theme::palette::text_muted()),
                    );
                    ui.label(
                        egui::RichText::new(format!("{} XP", state.account_xp))
                            .font(egui::FontId::proportional(12.0))
                            .strong()
                            .color(sow_ui_kit::theme::palette::neon_cyan()),
                    );
                    ui.label(
                        egui::RichText::new("·")
                            .font(egui::FontId::proportional(12.0))
                            .color(sow_ui_kit::theme::palette::text_muted()),
                    );
                    ui.label(
                        egui::RichText::new(format!("✦ {}", state.laurels)) // emoji-ok
                            .font(egui::FontId::proportional(12.0))
                            .strong()
                            .color(sow_ui_kit::theme::palette::neon_gold()),
                    );
                });
            });
        });
    });
}

fn draw_phone_profile_header(
    ui: &mut Ui,
    state: &mut MainMenuState,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
    strings: &sow_i18n::MainMenuStrings,
    action: &mut Option<crate::UiAction>,
) {
    let frame = egui::Frame::NONE
        .fill(sow_ui_kit::theme::palette::button_inactive())
        .stroke(Stroke::new(1.0_f32, palette::field_border()))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(8, 8));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let avatar_size = 44.0;
            let (avatar_rect, response) = ui.allocate_exact_size(
                egui::vec2(avatar_size, avatar_size),
                egui::Sense::click(),
            );
            ui.ctx().data_mut(|d| d.insert_temp(main_menu_avatar_rect_id(), avatar_rect));
            if response.clicked() {
                state.show_leader_picker = true;
            }
            let rgb = state.selected_leader.filler_rgb();
            let fill = Color32::from_rgb(
                (rgb[0] * 255.0).round() as u8,
                (rgb[1] * 255.0).round() as u8,
                (rgb[2] * 255.0).round() as u8,
            );
            ui.painter().rect_filled(avatar_rect, 6.0, fill);
            if let Some(tex) = asset_loader.avatars.get(&state.selected_leader) {
                ui.put(
                    avatar_rect,
                    egui::Image::new(tex)
                        .fit_to_exact_size(avatar_rect.size())
                        .corner_radius(egui::CornerRadius::same(6)),
                );
            }
            ui.painter().rect_stroke(
                avatar_rect,
                6.0,
                Stroke::new(1.0_f32, if response.hovered() { palette::neon_cyan() } else { fill }),
                egui::StrokeKind::Inside,
            );
            ui.painter().circle_filled(
                egui::pos2(avatar_rect.max.x - 2.0, avatar_rect.max.y - 2.0),
                4.0,
                Color32::from_rgb(34, 197, 94),
            );
            ui.add_space(6.0);
            ui.vertical(|ui| {
                if state.player_name.chars().count() > 16 {
                    state.player_name = state.player_name.chars().take(16).collect();
                }
                let field_frame = egui::Frame::NONE
                    .fill(palette::field_bg())
                    .stroke(Stroke::new(1.0_f32, palette::field_border()))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(8, 3));
                field_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let output = ui.add_sized(
                        [ui.available_width(), 34.0],
                        egui::TextEdit::singleline(&mut state.player_name)
                            .id(egui::Id::new("main_menu_nickname"))
                            .hint_text(&strings.nickname_hint)
                            .char_limit(16)
                            .frame(egui::Frame::NONE)
                            .font(egui::FontId::proportional(16.0))
                            .text_color(Color32::WHITE),
                    );
                    if output.lost_focus()
                        || (output.has_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                    {
                        *action = Some(crate::UiAction::SaveDisplayName(state.player_name.clone()));
                    }
                });
                let sign_in = crate::widgets::ThemeButton::new(&strings.sign_in)
                    .style(crate::widgets::ThemeButtonStyle::Secondary)
                    .min_size(egui::vec2(ui.available_width(), 44.0))
                    .text_size(16.0);
                if ui.add(sign_in).clicked() {
                    *action = Some(crate::UiAction::PortalShowAuthPrompt);
                }
            });
        });
        ui.add_space(6.0);
        draw_progression_phone(ui, state);
    });
}

fn draw_progression_phone(ui: &mut Ui, state: &MainMenuState) {
    egui::Frame::NONE
        .fill(Color32::from_rgba_unmultiplied(26, 30, 40, 200))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(243, 177, 43, 80)))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(RichText::new(format!("LV {}", state.account_level)).size(12.0).strong());
                ui.label(RichText::new("·").size(12.0).color(palette::text_muted()));
                ui.label(RichText::new(format!("{} XP", state.account_xp)).size(12.0).strong().color(palette::neon_cyan()));
                ui.label(RichText::new("·").size(12.0).color(palette::text_muted()));
                ui.label(RichText::new(format!("✦ {}", state.laurels)).size(12.0).strong().color(palette::neon_gold()));
            });
        });
}

fn draw_stat(ui: &mut Ui, value: impl std::fmt::Display, label: &str, color: Color32) {
    egui::Frame::NONE
        .fill(Color32::from_rgba_unmultiplied(10, 13, 20, 220))
        .stroke(Stroke::new(1.0_f32, palette::field_border()))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width().min(145.0).max(112.0));
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(value.to_string()).size(24.0).strong().color(color));
                ui.label(egui::RichText::new(label).size(10.0).strong().color(palette::text_muted()));
            });
        });
}

fn profile_match_row(
    ui: &mut Ui,
    summary: &sow_data::profile::PublicMatchSummary,
    action: &mut Option<crate::UiAction>,
) {
    let result = if summary.won { "WIN" } else { "LOSS" };
    let result_color = if summary.won { palette::neon_cyan() } else { palette::danger() };
    Card::inset().show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(result).strong().color(result_color));
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(&summary.mode).strong());
                ui.label(egui::RichText::new(format!("{} · {}", summary.map_name, summary.queue)).size(11.0).color(palette::text_muted()));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if Button::ghost("DETAILS")
                    .small()
                    .min_size(egui::vec2(0.0, 44.0))
                    .show(ui)
                    .clicked()
                {
                    *action = Some(crate::UiAction::LoadMatchDetail(summary.match_id.clone()));
                }
                ui.add_space(10.0);
                ui.label(egui::RichText::new(format!("{} / {} / {}", summary.kills, summary.deaths, summary.assists)).size(12.0).color(palette::text_muted()));
            });
        });
    });
}

/// Native profile page. It intentionally only renders state; sow-client owns loading and writes
/// the result into `MainMenuState::profile`.
pub fn draw_native(root_ui: &mut Ui, state: &mut MainMenuState, action: &mut Option<crate::UiAction>) {
    let screen = root_ui.ctx().content_rect();
    let metrics = super::layout::main_menu_metrics(root_ui.ctx());
    root_ui.painter().rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(5, 7, 12, 232));
    let header_name = state
        .profile
        .view
        .as_ref()
        .map(|view| view.display_name.clone())
        .unwrap_or_else(|| state.player_name.clone());
    let header_handle = state.profile.view.as_ref().map(|view| view.handle.clone());

    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(metrics.outer_pad as i8, 12))
        .show(root_ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if Button::ghost("← BACK")
                    .small()
                    .min_size(egui::vec2(92.0, 44.0))
                    .show(ui)
                    .clicked()
                {
                    state.go_home();
                }
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("PLAYER PROFILE").size(11.0).strong().color(palette::neon_gold()));
                    ui.label(egui::RichText::new(header_name.to_uppercase()).size(24.0).strong());
                    if let Some(handle) = &header_handle {
                        ui.label(RichText::new(handle).size(11.0).color(palette::text_muted()));
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("LV {}", state.account_level)).strong().color(palette::neon_cyan()));
                });
            });
        });

    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(metrics.outer_pad as i8, 8))
        .show(root_ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let view = state.profile.view.clone();
                let profile_id = state.profile.public_id.clone();
                let active_tab = state.profile.active_tab;
                ui.horizontal_wrapped(|ui| {
                    draw_stat(ui, view.as_ref().map(|v| v.matches_played).unwrap_or(0), "MATCHES", palette::text_normal());
                    draw_stat(ui, view.as_ref().map(|v| v.wins).unwrap_or(0), "WINS", palette::neon_cyan());
                    draw_stat(ui, view.as_ref().map(|v| (v.win_rate * 100.0).round() as u32).unwrap_or(0).to_string() + "%", "WIN RATE", palette::neon_gold());
                    draw_stat(ui, view.as_ref().map(|v| v.kills).unwrap_or(0), "KILLS", palette::text_normal());
                });
                ui.add_space(18.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("FIND A PLAYER").size(11.0).strong().color(palette::neon_gold()));
                    let search_w = (ui.available_width() - 104.0).clamp(120.0, 280.0);
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut state.profile.search_query)
                            .hint_text("Name or handle…")
                            .desired_width(search_w),
                    );
                    let submit = Button::secondary("SEARCH")
                        .small()
                        .min_size(egui::vec2(104.0, 44.0))
                        .show(ui)
                        .clicked()
                        || (response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                    if submit && !state.profile.search_query.trim().is_empty() && action.is_none() {
                        *action = Some(crate::UiAction::SearchProfiles(state.profile.search_query.clone()));
                    }
                });
                for result in state.profile.search_results.clone() {
                    let label = format!("{}  ·  {}  ·  LV {}", result.display_name, result.handle, result.level);
                    if Button::ghost(&label)
                        .small()
                        .min_size(egui::vec2(0.0, 44.0))
                        .show(ui)
                        .clicked()
                        && action.is_none()
                    {
                        *action = Some(crate::UiAction::OpenPublicProfilePage(result.public_id));
                    }
                }
                ui.add_space(18.0);
                ui.horizontal_wrapped(|ui| {
                    for (tab, label) in [
                        (NativeProfileTab::Overview, "OVERVIEW"),
                        (NativeProfileTab::Leaders, "LEADERS"),
                        (NativeProfileTab::History, "HISTORY"),
                        (NativeProfileTab::Ranked, "RANKED"),
                    ] {
                        let selected = state.profile.active_tab == tab;
                        let button = if selected { Button::primary(label) } else { Button::secondary(label) };
                        if button
                            .small()
                            .min_size(egui::vec2(0.0, 44.0))
                            .show(ui)
                            .clicked()
                        {
                            state.profile.active_tab = tab;
                        }
                    }
                });
                ui.add_space(12.0);
                match active_tab {
                    NativeProfileTab::Overview => {
                        Heading::new("RECENT MATCHES").cyan().show(ui);
                        ui.add_space(8.0);
                        if let Some(view) = view.as_ref() {
                            for summary in view.recent_matches.iter().take(10) {
                                profile_match_row(ui, summary, action);
                                ui.add_space(6.0);
                            }
                        } else if state.profile.loading {
                            ui.spinner();
                            ui.label("Loading profile…");
                        } else {
                            ui.label(egui::RichText::new("Profile data is not available yet.").color(palette::text_muted()));
                        }
                    }
                    NativeProfileTab::Leaders => {
                        Heading::new("LEADER MASTERY").cyan().show(ui);
                        ui.add_space(8.0);
                        if let Some(view) = view.as_ref() {
                            for leader in &view.leaders {
                                Card::inset().show(ui, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(egui::RichText::new(&leader.leader).strong());
                                        ui.label(egui::RichText::new(format!("{} matches · {} wins · {} XP", leader.matches_played, leader.wins, leader.xp)).size(12.0).color(palette::text_muted()));
                                    });
                                });
                                ui.add_space(6.0);
                            }
                        }
                    }
                    NativeProfileTab::History => {
                        Heading::new("MATCH HISTORY").cyan().show(ui);
                        ui.add_space(8.0);
                        for summary in state.profile.history.clone() {
                            profile_match_row(ui, &summary, action);
                            ui.add_space(6.0);
                        }
                        if state.profile.history.is_empty() && !state.profile.loading {
                            ui.label(egui::RichText::new("No completed matches.").color(palette::text_muted()));
                        }
                        if state.profile.history_has_next && !state.profile.loading {
                            if Button::secondary("LOAD MORE MATCHES")
                                .small()
                                .min_size(egui::vec2(0.0, 44.0))
                                .show(ui)
                                .clicked()
                                && action.is_none()
                            {
                                *action = Some(crate::UiAction::LoadProfileHistory);
                            }
                        }
                    }
                    NativeProfileTab::Ranked => {
                        Heading::new("RANKED RECORDS").cyan().show(ui);
                        ui.add_space(8.0);
                        for rating in &state.profile.ratings {
                            Card::inset().show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(egui::RichText::new(&rating.season_name).strong());
                                    ui.label(egui::RichText::new(format!("{} · {}", rating.queue, rating.mode)).size(12.0).color(palette::text_muted()));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(egui::RichText::new(format!("{} {} · {} SR", rating.tier, rating.division.as_deref().unwrap_or(""), rating.score)).strong().color(palette::neon_gold()));
                                    });
                                });
                            });
                            ui.add_space(6.0);
                        }
                        if state.profile.ratings.is_empty() && !state.profile.loading {
                            if state.profile.ratings_loaded {
                                ui.label(egui::RichText::new("No ranked records.").color(palette::text_muted()));
                            } else if Button::secondary("LOAD RANKED RECORDS")
                                .small()
                                .min_size(egui::vec2(0.0, 44.0))
                                .show(ui)
                                .clicked()
                                && action.is_none()
                            {
                                *action = Some(crate::UiAction::LoadProfileRatings);
                            }
                        }
                        if state.profile.ratings.is_empty() && state.profile.loading {
                            ui.spinner();
                            ui.label("Loading ranked records…");
                        }
                    }
                }
                if let Some(error) = &state.profile.error {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new(error).color(palette::danger()));
                }
                if action.is_none()
                    && state.profile.view.is_none()
                    && !state.profile.loading
                    && state.profile.error.is_none()
                {
                    *action = Some(crate::UiAction::LoadOwnProfile);
                    state.profile.loading = true;
                }
                if state.profile.loading {
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spinner();
                        ui.label("Loading…");
                    });
                }
                if state.profile.error.is_some() {
                    if let Some(profile_id) = profile_id
                        && Button::secondary("RETRY")
                            .small()
                            .min_size(egui::vec2(0.0, 44.0))
                            .show(ui)
                            .clicked()
                        && action.is_none()
                    {
                        *action = Some(crate::UiAction::OpenPublicProfilePage(profile_id));
                    }
                }
            });
        });

    if let Some(detail) = state.profile.match_detail.clone() {
        egui::Window::new("MATCH DETAILS")
            .collapsible(false)
            .resizable(true)
            .default_width((metrics.width - metrics.outer_pad * 2.0).clamp(280.0, 520.0))
            .max_width((metrics.width - metrics.outer_pad * 2.0).max(280.0))
            .show(root_ui.ctx(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&detail.mode).size(20.0).strong());
                    ui.label(RichText::new(format!("{} · {}", detail.map_name, detail.queue)).color(palette::text_muted()));
                });
                ui.label(RichText::new(if detail.verified { "VERIFIED MATCH" } else { "UNVERIFIED MATCH" }).size(11.0).strong().color(if detail.verified { palette::neon_cyan() } else { palette::text_muted() }));
                ui.add_space(10.0);
                for participant in detail.participants {
                    Card::inset().show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(if participant.won { "WIN" } else { "LOSS" }).strong().color(if participant.won { palette::neon_cyan() } else { palette::danger() }));
                            ui.label(RichText::new(&participant.handle).strong());
                            ui.label(RichText::new(format!("{} · {} / {} / {}", participant.leader.as_deref().unwrap_or("—"), participant.kills, participant.deaths, participant.assists)).size(11.0).color(palette::text_muted()));
                        });
                    });
                }
                if Button::ghost("CLOSE")
                    .small()
                    .min_size(egui::vec2(0.0, 44.0))
                    .show(ui)
                    .clicked()
                    && action.is_none()
                {
                    *action = Some(crate::UiAction::CloseMatchDetail);
                }
            });
    }
}
