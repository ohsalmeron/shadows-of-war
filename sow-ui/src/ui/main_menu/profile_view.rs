use super::super::MainMenuState;
use super::NativeProfileTab;
use crate::kit::components::{Button, Card, Heading};
use crate::ui::asset_loader::AssetLoader;
use crate::ui::main_menu::layout;
use egui::{
    Align, Color32, CornerRadius, Layout, Margin, Rect, RichText, Sense, Stroke, TextureHandle, Ui,
    Vec2,
};
use sow_core::player::Leader;
use sow_ui_kit::theme::palette;

const GAP: f32 = 8.0;
const SECTION_GAP: f32 = 16.0;

fn leader_for_profile(
    view: Option<&sow_data::profile::PublicProfileView>,
    state: &MainMenuState,
) -> Leader {
    view.and_then(|view| view.preferred_leader.as_deref())
        .and_then(sow_data::commerce::leader_from_id)
        .unwrap_or(state.selected_leader)
}

fn leader_for_id(value: Option<&str>, fallback: Leader) -> Leader {
    value
        .and_then(sow_data::commerce::leader_from_id)
        .unwrap_or(fallback)
}

fn leader_texture(
    assets: &AssetLoader,
    leader: Leader,
    mobile: bool,
    portrait: bool,
) -> Option<&TextureHandle> {
    if portrait {
        assets
            .leader_portrait_texture(leader, mobile)
            .or_else(|| assets.avatars.get(&leader))
    } else {
        assets
            .avatars
            .get(&leader)
            .or_else(|| assets.leader_portrait_texture(leader, mobile))
    }
}

/// Paint an image inside a fixed frame without stretching or cropping its source ratio.
fn draw_aspect_fit(ui: &mut Ui, rect: Rect, texture: &TextureHandle, radius: u8) {
    ui.painter()
        .rect_filled(rect, radius as f32, Color32::from_rgb(24, 28, 38));
    let source = texture.size_vec2();
    if source.x <= 0.0 || source.y <= 0.0 {
        return;
    }
    let scale = (rect.width() / source.x).min(rect.height() / source.y);
    let image_rect = Rect::from_center_size(rect.center(), source * scale);
    ui.put(
        image_rect,
        egui::Image::new(texture)
            .fit_to_exact_size(image_rect.size())
            .maintain_aspect_ratio(true)
            .corner_radius(CornerRadius::same(radius)),
    );
}

fn draw_leader_visual(
    ui: &mut Ui,
    assets: &AssetLoader,
    leader: Leader,
    mobile: bool,
    size: Vec2,
    portrait: bool,
) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let rgb = leader.filler_rgb();
    let fill = Color32::from_rgb(
        (rgb[0] * 255.0).round() as u8,
        (rgb[1] * 255.0).round() as u8,
        (rgb[2] * 255.0).round() as u8,
    );
    ui.painter().rect_filled(rect, 8.0, fill);
    if let Some(texture) = leader_texture(assets, leader, mobile, portrait) {
        draw_aspect_fit(ui, rect, texture, 8);
    }
    ui.painter().rect_stroke(
        rect,
        8.0,
        Stroke::new(1.0_f32, palette::field_border()),
        egui::StrokeKind::Inside,
    );
}

fn draw_identity_text(
    ui: &mut Ui,
    view: Option<&sow_data::profile::PublicProfileView>,
    state: &MainMenuState,
    leader: Leader,
    phone: bool,
) {
    let name = view
        .map(|view| view.display_name.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(&state.player_name);
    let handle = view.map(|view| view.handle.as_str()).unwrap_or("");
    let level = view.map(|view| view.level).unwrap_or(state.account_level);
    ui.vertical(|ui| {
        ui.label(
            RichText::new("PLAYER PROFILE")
                .size(11.0)
                .strong()
                .color(palette::neon_gold()),
        );
        ui.add_space(2.0);
        ui.label(
            RichText::new(name.to_uppercase())
                .size(if phone { 22.0 } else { 26.0 })
                .strong(),
        );
        if !handle.is_empty() {
            ui.label(
                RichText::new(handle)
                    .size(12.0)
                    .color(palette::text_muted()),
            );
        }
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!("LEVEL {}", level))
                .size(12.0)
                .strong()
                .color(palette::neon_cyan()),
        );
        ui.label(
            RichText::new(leader.name())
                .size(12.0)
                .color(palette::text_muted()),
        );
    });
}

fn draw_identity(
    ui: &mut Ui,
    view: Option<&sow_data::profile::PublicProfileView>,
    state: &MainMenuState,
    assets: &AssetLoader,
    mobile: bool,
    phone: bool,
) {
    let leader = leader_for_profile(view, state);
    Card::glass()
        .padding(Margin::symmetric(12, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if phone {
                let width = ui.available_width();
                draw_leader_visual(
                    ui,
                    assets,
                    leader,
                    mobile,
                    Vec2::new(width, (width * 9.0 / 16.0).min(164.0)),
                    true,
                );
                ui.add_space(10.0);
                draw_identity_text(ui, view, state, leader, true);
            } else {
                ui.horizontal(|ui| {
                    let width = ui.available_width().min(220.0);
                    draw_leader_visual(
                        ui,
                        assets,
                        leader,
                        mobile,
                        Vec2::new(width, width * 9.0 / 16.0),
                        true,
                    );
                    ui.add_space(16.0);
                    draw_identity_text(ui, view, state, leader, false);
                });
            }
        });
}

fn draw_stat(ui: &mut Ui, value: impl std::fmt::Display, label: &str, color: Color32) {
    let available = ui.available_width();
    egui::Frame::NONE
        .fill(Color32::from_rgba_unmultiplied(10, 13, 20, 220))
        .stroke(Stroke::new(1.0_f32, palette::field_border()))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width((available - 24.0).max(1.0));
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(value.to_string())
                        .size(24.0)
                        .strong()
                        .color(color),
                );
                ui.label(
                    RichText::new(label)
                        .size(10.0)
                        .strong()
                        .color(palette::text_muted()),
                );
            });
        });
}

fn draw_stats(ui: &mut Ui, view: Option<&sow_data::profile::PublicProfileView>, phone: bool) {
    let stats = [
        (
            view.map(|view| view.matches_played)
                .unwrap_or(0)
                .to_string(),
            "MATCHES",
            palette::text_normal(),
        ),
        (
            view.map(|view| view.wins).unwrap_or(0).to_string(),
            "WINS",
            palette::neon_cyan(),
        ),
        (
            format!(
                "{}%",
                view.map(|view| (view.win_rate * 100.0).round() as u32)
                    .unwrap_or(0)
            ),
            "WIN RATE",
            palette::neon_gold(),
        ),
        (
            view.map(|view| view.kills).unwrap_or(0).to_string(),
            "KILLS",
            palette::text_normal(),
        ),
    ];
    if phone {
        for row in 0..2 {
            ui.columns(2, |columns| {
                for column in 0..2 {
                    let (value, label, color) = &stats[row * 2 + column];
                    draw_stat(&mut columns[column], value, label, *color);
                }
            });
            if row == 0 {
                ui.add_space(GAP);
            }
        }
    } else {
        ui.columns(4, |columns| {
            for (column, (value, label, color)) in columns.iter_mut().zip(stats) {
                draw_stat(column, value, label, color);
            }
        });
    }
}

fn draw_search(
    ui: &mut Ui,
    state: &mut MainMenuState,
    action: &mut Option<crate::UiAction>,
    phone: bool,
) {
    ui.label(
        RichText::new("FIND A PLAYER")
            .size(11.0)
            .strong()
            .color(palette::neon_gold()),
    );
    ui.add_space(6.0);
    if phone {
        let response = ui.add_sized(
            [ui.available_width(), 40.0],
            egui::TextEdit::singleline(&mut state.profile.search_query)
                .hint_text("Name or handle..."),
        );
        ui.add_space(GAP);
        let submit = Button::secondary("SEARCH")
            .small()
            .min_size(Vec2::new(ui.available_width(), 40.0))
            .show(ui)
            .clicked()
            || (response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
        if submit && !state.profile.search_query.trim().is_empty() && action.is_none() {
            *action = Some(crate::UiAction::SearchProfiles(
                state.profile.search_query.clone(),
            ));
        }
    } else {
        ui.horizontal(|ui| {
            ui.set_width(ui.available_width());
            let button_width = 96.0;
            let input_width = (ui.available_width() - button_width - GAP).max(140.0);
            let response = ui.add_sized(
                [input_width, 40.0],
                egui::TextEdit::singleline(&mut state.profile.search_query)
                    .hint_text("Name or handle..."),
            );
            ui.add_space(GAP);
            let submit = Button::secondary("SEARCH")
                .small()
                .min_size(Vec2::new(button_width, 40.0))
                .show(ui)
                .clicked()
                || (response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
            if submit && !state.profile.search_query.trim().is_empty() && action.is_none() {
                *action = Some(crate::UiAction::SearchProfiles(
                    state.profile.search_query.clone(),
                ));
            }
        });
    }
    for result in state.profile.search_results.clone() {
        ui.add_space(4.0);
        Card::inset()
            .padding(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&result.display_name).strong());
                        ui.label(
                            RichText::new(format!("{} / LEVEL {}", result.handle, result.level))
                                .size(11.0)
                                .color(palette::text_muted()),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if Button::ghost("OPEN")
                            .small()
                            .min_size(Vec2::new(72.0, 40.0))
                            .show(ui)
                            .clicked()
                            && action.is_none()
                        {
                            *action = Some(crate::UiAction::OpenPublicProfilePage(
                                result.public_id.clone(),
                            ));
                        }
                    });
                });
            });
    }
}

fn draw_tabs(ui: &mut Ui, state: &mut MainMenuState, phone: bool) {
    let tabs = [
        (NativeProfileTab::Overview, "OVERVIEW"),
        (NativeProfileTab::Leaders, "LEADERS"),
        (NativeProfileTab::History, "HISTORY"),
        (NativeProfileTab::Ranked, "RANKED"),
    ];
    if phone {
        for row in 0..2 {
            ui.columns(2, |columns| {
                for column in 0..2 {
                    let (tab, label) = tabs[row * 2 + column];
                    let width = columns[column].available_width();
                    let button = if state.profile.active_tab == tab {
                        Button::primary(label)
                    } else {
                        Button::secondary(label)
                    };
                    if button
                        .small()
                        .min_size(Vec2::new(width, 40.0))
                        .show(&mut columns[column])
                        .clicked()
                    {
                        state.profile.active_tab = tab;
                    }
                }
            });
            if row == 0 {
                ui.add_space(GAP);
            }
        }
    } else {
        ui.horizontal(|ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.x = GAP;
            let width = ((ui.available_width() - GAP * 3.0) / 4.0).max(80.0);
            for (tab, label) in tabs {
                let button = if state.profile.active_tab == tab {
                    Button::primary(label)
                } else {
                    Button::secondary(label)
                };
                if button
                    .small()
                    .min_size(Vec2::new(width, 40.0))
                    .show(ui)
                    .clicked()
                {
                    state.profile.active_tab = tab;
                }
            }
        });
    }
}

fn draw_match_row(
    ui: &mut Ui,
    summary: &sow_data::profile::PublicMatchSummary,
    action: &mut Option<crate::UiAction>,
    phone: bool,
) {
    let result = if summary.won { "WIN" } else { "LOSS" };
    let result_color = if summary.won {
        palette::neon_cyan()
    } else {
        palette::danger()
    };
    Card::inset()
        .padding(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if phone {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(result).strong().color(result_color));
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&summary.mode).strong());
                            ui.label(
                                RichText::new(format!("{} / {}", summary.map_name, summary.queue))
                                    .size(11.0)
                                    .color(palette::text_muted()),
                            );
                        });
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "KDA {} / {} / {}",
                                summary.kills, summary.deaths, summary.assists
                            ))
                            .size(11.0)
                            .color(palette::text_muted()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if Button::ghost("DETAILS")
                                .small()
                                .min_size(Vec2::new(76.0, 40.0))
                                .show(ui)
                                .clicked()
                            {
                                *action = Some(crate::UiAction::LoadMatchDetail(
                                    summary.match_id.clone(),
                                ));
                            }
                        });
                    });
                });
            } else {
                let row_width = ui.available_width();
                let result_width = 58.0;
                let details_width = 126.0;
                let context_width =
                    (row_width - result_width - details_width - GAP * 2.0).max(120.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = GAP;
                    ui.allocate_ui_with_layout(
                        Vec2::new(result_width, 44.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.add_space(12.0);
                            ui.label(RichText::new(result).strong().color(result_color));
                        },
                    );
                    ui.allocate_ui_with_layout(
                        Vec2::new(context_width, 44.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.label(RichText::new(&summary.mode).strong());
                            ui.label(
                                RichText::new(format!("{} / {}", summary.map_name, summary.queue))
                                    .size(11.0)
                                    .color(palette::text_muted()),
                            );
                        },
                    );
                    ui.allocate_ui_with_layout(
                        Vec2::new(details_width, 44.0),
                        Layout::right_to_left(Align::Center),
                        |ui| {
                            if Button::ghost("DETAILS")
                                .small()
                                .min_size(Vec2::new(76.0, 40.0))
                                .show(ui)
                                .clicked()
                            {
                                *action = Some(crate::UiAction::LoadMatchDetail(
                                    summary.match_id.clone(),
                                ));
                            }
                            ui.add_space(GAP);
                            ui.label(
                                RichText::new(format!(
                                    "{} / {} / {}",
                                    summary.kills, summary.deaths, summary.assists
                                ))
                                .size(11.0)
                                .color(palette::text_muted()),
                            );
                        },
                    );
                });
            }
        });
}

fn draw_recent_matches(
    ui: &mut Ui,
    matches: &[sow_data::profile::PublicMatchSummary],
    action: &mut Option<crate::UiAction>,
    phone: bool,
) {
    Heading::new("RECENT MATCHES").cyan().show(ui);
    ui.add_space(6.0);
    if matches.is_empty() {
        ui.label(RichText::new("No completed matches.").color(palette::text_muted()));
        return;
    }
    for summary in matches.iter().take(10) {
        draw_match_row(ui, summary, action, phone);
        ui.add_space(6.0);
    }
}

fn draw_leader_mastery(
    ui: &mut Ui,
    leaders: &[sow_data::profile::PublicLeaderSummary],
    state: &MainMenuState,
    assets: &AssetLoader,
    mobile: bool,
    heading: &str,
) {
    Heading::new(heading).cyan().show(ui);
    ui.add_space(6.0);
    if leaders.is_empty() {
        ui.label(RichText::new("No leader records.").color(palette::text_muted()));
        return;
    }
    for summary in leaders {
        let leader = leader_for_id(Some(&summary.leader), state.selected_leader);
        Card::inset()
            .padding(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    draw_leader_visual(ui, assets, leader, mobile, Vec2::new(52.0, 52.0), false);
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(leader.name()).strong());
                        ui.label(
                            RichText::new(format!(
                                "{} matches / {} wins / {} XP",
                                summary.matches_played, summary.wins, summary.xp
                            ))
                            .size(11.0)
                            .color(palette::text_muted()),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{}% win rate",
                                (summary.win_rate * 100.0).round() as u32
                            ))
                            .size(11.0)
                            .color(palette::neon_cyan()),
                        );
                    });
                });
            });
        ui.add_space(6.0);
    }
}

fn draw_ranked(
    ui: &mut Ui,
    state: &mut MainMenuState,
    action: &mut Option<crate::UiAction>,
    phone: bool,
) {
    Heading::new("RANKED RECORDS").cyan().show(ui);
    ui.add_space(6.0);
    for rating in &state.profile.ratings {
        Card::inset()
            .padding(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if phone {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&rating.season_name).strong());
                        ui.label(
                            RichText::new(format!("{} / {}", rating.queue, rating.mode))
                                .size(11.0)
                                .color(palette::text_muted()),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{} {} / {} SR",
                                rating.tier,
                                rating.division.as_deref().unwrap_or(""),
                                rating.score
                            ))
                            .strong()
                            .color(palette::neon_gold()),
                        );
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&rating.season_name).strong());
                            ui.label(
                                RichText::new(format!("{} / {}", rating.queue, rating.mode))
                                    .size(11.0)
                                    .color(palette::text_muted()),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} {} / {} SR",
                                    rating.tier,
                                    rating.division.as_deref().unwrap_or(""),
                                    rating.score
                                ))
                                .strong()
                                .color(palette::neon_gold()),
                            );
                        });
                    });
                }
            });
        ui.add_space(6.0);
    }
    if state.profile.ratings.is_empty() && !state.profile.loading {
        if state.profile.ratings_loaded {
            ui.label(RichText::new("No ranked records.").color(palette::text_muted()));
        } else if Button::secondary("LOAD RANKED RECORDS")
            .small()
            .min_size(Vec2::new(ui.available_width(), 40.0))
            .show(ui)
            .clicked()
            && action.is_none()
        {
            *action = Some(crate::UiAction::LoadProfileRatings);
        }
    }
    if state.profile.ratings.is_empty() && state.profile.loading {
        ui.spinner();
        ui.label("Loading ranked records...");
    }
}

fn draw_tab_content(
    ui: &mut Ui,
    state: &mut MainMenuState,
    view: Option<&sow_data::profile::PublicProfileView>,
    assets: &AssetLoader,
    action: &mut Option<crate::UiAction>,
    phone: bool,
    mobile: bool,
) {
    match state.profile.active_tab {
        NativeProfileTab::Overview => {
            if let Some(view) = view {
                if phone {
                    draw_recent_matches(ui, &view.recent_matches, action, true);
                    ui.add_space(SECTION_GAP);
                    draw_leader_mastery(ui, &view.leaders, state, assets, mobile, "LEADER MASTERY");
                } else {
                    ui.columns(2, |columns| {
                        draw_recent_matches(&mut columns[0], &view.recent_matches, action, false);
                        draw_leader_mastery(
                            &mut columns[1],
                            &view.leaders,
                            state,
                            assets,
                            mobile,
                            "LEADER MASTERY",
                        );
                    });
                }
            } else if state.profile.loading {
                ui.spinner();
                ui.label("Loading profile...");
            } else {
                ui.label(
                    RichText::new("Profile data is not available yet.")
                        .color(palette::text_muted()),
                );
            }
        }
        NativeProfileTab::Leaders => {
            if let Some(view) = view {
                draw_leader_mastery(ui, &view.leaders, state, assets, mobile, "LEADER MASTERY");
            }
        }
        NativeProfileTab::History => {
            Heading::new("MATCH HISTORY").cyan().show(ui);
            ui.add_space(6.0);
            let history = state.profile.history.clone();
            for summary in &history {
                draw_match_row(ui, summary, action, phone);
                ui.add_space(6.0);
            }
            if history.is_empty() && !state.profile.loading {
                ui.label(RichText::new("No completed matches.").color(palette::text_muted()));
            }
            if state.profile.history_has_next
                && !state.profile.loading
                && Button::secondary("LOAD MORE MATCHES")
                    .small()
                    .min_size(Vec2::new(ui.available_width(), 40.0))
                    .show(ui)
                    .clicked()
                && action.is_none()
            {
                *action = Some(crate::UiAction::LoadProfileHistory);
            }
        }
        NativeProfileTab::Ranked => draw_ranked(ui, state, action, phone),
    }
}

fn draw_match_detail(
    root_ui: &mut Ui,
    screen: Rect,
    pad: f32,
    detail: sow_data::profile::PublicMatchDetail,
    action: &mut Option<crate::UiAction>,
) {
    let width = (screen.width() - pad * 2.0).clamp(280.0, 560.0);
    let height = (screen.height() - 32.0).clamp(260.0, 680.0);
    egui::Window::new("MATCH DETAILS")
        .collapsible(false)
        .resizable(true)
        .default_width(width)
        .default_height(height)
        .max_width(width)
        .max_height(height)
        .show(root_ui.ctx(), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("native_profile_match_detail")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label(RichText::new(&detail.mode).size(20.0).strong());
                    ui.label(
                        RichText::new(format!("{} / {}", detail.map_name, detail.queue))
                            .color(palette::text_muted()),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(if detail.verified {
                            "VERIFIED MATCH"
                        } else {
                            "UNVERIFIED MATCH"
                        })
                        .size(11.0)
                        .strong()
                        .color(if detail.verified {
                            palette::neon_cyan()
                        } else {
                            palette::text_muted()
                        }),
                    );
                    ui.add_space(8.0);
                    for participant in &detail.participants {
                        Card::inset()
                            .padding(Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(if participant.won {
                                                "WIN"
                                            } else {
                                                "LOSS"
                                            })
                                            .strong()
                                            .color(
                                                if participant.won {
                                                    palette::neon_cyan()
                                                } else {
                                                    palette::danger()
                                                },
                                            ),
                                        );
                                        ui.add_space(8.0);
                                        ui.label(RichText::new(&participant.handle).strong());
                                    });
                                    ui.label(
                                        RichText::new(format!(
                                            "{} / {} / {} / {}",
                                            participant.leader.as_deref().unwrap_or("-"),
                                            participant.kills,
                                            participant.deaths,
                                            participant.assists
                                        ))
                                        .size(11.0)
                                        .color(palette::text_muted()),
                                    );
                                });
                            });
                        ui.add_space(6.0);
                    }
                    if Button::ghost("CLOSE")
                        .small()
                        .min_size(Vec2::new(ui.available_width(), 40.0))
                        .show(ui)
                        .clicked()
                        && action.is_none()
                    {
                        *action = Some(crate::UiAction::CloseMatchDetail);
                    }
                });
        });
}

/// Refactored native profile presentation. Data loading and actions remain owned by sow-client.
pub fn draw_native(
    root_ui: &mut Ui,
    state: &mut MainMenuState,
    assets: &AssetLoader,
    action: &mut Option<crate::UiAction>,
) {
    let screen = root_ui.available_rect_before_wrap();
    let metrics = layout::main_menu_metrics(root_ui.ctx());
    let phone = metrics.is_phone();
    let mobile = phone;
    let pad = metrics.outer_pad;

    let name = state
        .profile
        .view
        .as_ref()
        .map(|view| view.display_name.clone())
        .unwrap_or_else(|| state.player_name.clone());
    let body_rect = screen.shrink2(Vec2::new(pad, 8.0));
    root_ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(body_rect)
            .layout(Layout::top_down(Align::Min)),
        |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(body_rect));
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("PROFILE")
                        .size(if phone { 20.0 } else { 28.0 })
                        .strong()
                        .color(palette::neon_cyan()),
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new(name.to_uppercase())
                        .size(11.0)
                        .strong()
                        .color(palette::text_muted()),
                );
            });
            ui.add_space(8.0);
            let view = state.profile.view.clone();
            let profile_id = state.profile.public_id.clone();

            // Identity, stats, search, and tabs are the fixed profile chrome.
            // Only the selected tab's data scrolls, so changing tabs never
            // makes the navigation disappear below a long history list.
            draw_identity(ui, view.as_ref(), state, assets, mobile, phone);
            ui.add_space(SECTION_GAP);
            draw_stats(ui, view.as_ref(), phone);
            ui.add_space(SECTION_GAP);
            draw_search(ui, state, action, phone);
            ui.add_space(SECTION_GAP);
            draw_tabs(ui, state, phone);
            ui.add_space(GAP);

            if action.is_none()
                && state.profile.view.is_none()
                && !state.profile.loading
                && state.profile.error.is_none()
            {
                *action = Some(crate::UiAction::LoadOwnProfile);
                state.profile.loading = true;
            }

            egui::ScrollArea::vertical()
                .id_salt("native_profile_tab_content")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    draw_tab_content(ui, state, view.as_ref(), assets, action, phone, mobile);

                    if let Some(error) = &state.profile.error {
                        ui.add_space(SECTION_GAP);
                        ui.label(RichText::new(error).color(palette::danger()));
                    }
                    if state.profile.loading {
                        ui.add_space(GAP);
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Loading...");
                        });
                    }
                    if state.profile.error.is_some()
                        && let Some(profile_id) = profile_id
                        && Button::secondary("RETRY")
                            .small()
                            .min_size(Vec2::new(ui.available_width(), 40.0))
                            .show(ui)
                            .clicked()
                        && action.is_none()
                    {
                        *action = Some(crate::UiAction::OpenPublicProfilePage(profile_id));
                    }
                });
        },
    );

    if let Some(detail) = state.profile.match_detail.clone() {
        draw_match_detail(root_ui, screen, pad, detail, action);
    }
}
