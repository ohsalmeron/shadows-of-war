use crate::UiAction;
use egui::{Color32, RichText};
use crate::ui::theme;
use super::MainMenuState;

pub fn draw_modal(
    ctx: &egui::Context,
    state: &mut MainMenuState,
    action: &mut Option<UiAction>,
) {
    let mut close = false;

    egui::Area::new(egui::Id::new("single_player_setup_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let screen_rect = ctx.content_rect();
            // Consume clicks on backdrop
            let response = ui.allocate_rect(screen_rect, egui::Sense::click());
            ui.painter()
                .rect_filled(screen_rect, 0.0, Color32::from_black_alpha(200));

            if response.clicked() {
                close = true;
            }

            let modal_size = egui::vec2(500.0, 600.0);
            let modal_rect = egui::Rect::from_center_size(screen_rect.center(), modal_size);

            // Glow border
            ui.painter()
                .rect_filled(modal_rect.expand(2.0), 8.0, theme::avatar_cyan());
            ui.painter()
                .rect_filled(modal_rect, 8.0, theme::panel_bg());

            ui.scope_builder(egui::UiBuilder::new().max_rect(modal_rect), |ui| {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.heading(
                        RichText::new("Single Player Setup")
                            .color(Color32::WHITE)
                            .size(24.0),
                    );
                });
                ui.add_space(24.0);

                // Use a ScrollArea for the content
                egui::ScrollArea::vertical()
                    .max_height(modal_size.y - 120.0)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 16.0);
                        let config = &mut state.single_player_config;

                        egui::Grid::new("sp_setup_grid")
                            .num_columns(2)
                            .spacing([40.0, 20.0])
                            .show(ui, |ui| {
                                // Map
                                ui.label(RichText::new("Map").color(theme::text_secondary()).size(16.0));
                                egui::ComboBox::from_id_salt("sp_map")
                                    .selected_text(&config.map_name)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut config.map_name, "world".to_string(), "world");
                                        ui.selectable_value(&mut config.map_name, "europe".to_string(), "europe");
                                        ui.selectable_value(&mut config.map_name, "europe_classic".to_string(), "europe_classic");
                                    });
                                ui.end_row();

                                // Difficulty
                                ui.label(RichText::new("Bot Difficulty").color(theme::text_secondary()).size(16.0));
                                egui::ComboBox::from_id_salt("sp_diff")
                                    .selected_text(format!("{:?}", config.bot_difficulty))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut config.bot_difficulty, sow_core::game_config::BotDifficulty::BrainDead, "BrainDead");
                                        ui.selectable_value(&mut config.bot_difficulty, sow_core::game_config::BotDifficulty::Vanilla, "Vanilla");
                                        ui.selectable_value(&mut config.bot_difficulty, sow_core::game_config::BotDifficulty::Terminator, "Terminator");
                                    });
                                ui.end_row();

                                // Bot count
                                ui.label(RichText::new("Tribes (Bots)").color(theme::text_secondary()).size(16.0));
                                ui.add(egui::Slider::new(&mut config.bot_count, 0..=1000));
                                ui.end_row();

                                // Nation count
                                ui.label(RichText::new("Nations").color(theme::text_secondary()).size(16.0));
                                ui.add(egui::Slider::new(&mut config.nation_count, 0..=400));
                                ui.end_row();
                                
                                // Random spawn
                                ui.label(RichText::new("Random Spawn").color(theme::text_secondary()).size(16.0));
                                ui.checkbox(&mut config.random_spawn, "");
                                ui.end_row();
                                
                                // Global Speed
                                ui.label(RichText::new("Global Speed Multiplier").color(theme::text_secondary()).size(16.0));
                                ui.add(egui::Slider::new(&mut config.global_speed_multiplier, 0.1..=5.0));
                                ui.end_row();
                            });
                    });

                // Bottom section for buttons
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(20.0); // Padding from bottom edge
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(20.0, 0.0);
                        
                        let cancel_btn = egui::Button::new(
                            RichText::new("Cancel").size(18.0).color(theme::text_secondary()),
                        )
                        .fill(theme::menu_secondary_button())
                        .min_size(egui::vec2(120.0, 40.0));
                        
                        if ui.add(cancel_btn).clicked() {
                            close = true;
                        }

                        let start_btn = egui::Button::new(
                            RichText::new("START GAME").size(18.0).color(Color32::WHITE).strong(),
                        )
                        .fill(theme::accent_solo_cyan())
                        .min_size(egui::vec2(160.0, 40.0));
                        
                        if ui.add(start_btn).clicked() {
                            *action = Some(UiAction::StartSinglePlayer(Box::new(*state.single_player_config.clone())));
                            close = true;
                        }
                    });
                });
            });
        });

    if close {
        state.show_single_player_setup = false;
    }
}
