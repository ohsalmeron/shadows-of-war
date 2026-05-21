use egui::{Color32, CornerRadius, Stroke};

pub fn draw_avatar_picker_modal(
    ctx: &egui::Context,
    selected_avatar_id: &mut u8,
    asset_loader: &crate::ui::asset_loader::AssetLoader,
) -> bool {
    let mut close = false;

    egui::Area::new(egui::Id::new("avatar_picker_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let screen_rect = ctx.content_rect();
            let response = ui.allocate_rect(screen_rect, egui::Sense::click());
            ui.painter().rect_filled(screen_rect, 0.0, Color32::from_black_alpha(200));

            if response.clicked() {
                close = true;
            }

            let is_mobile = screen_rect.width() < 600.0;
            let pad = if is_mobile { 32.0 } else { 50.0 };
            
            let modal_w = if is_mobile {
                screen_rect.width() - 32.0
            } else {
                380.0
            };
            let modal_size = egui::vec2(modal_w, 480.0);
            let modal_rect = egui::Rect::from_center_size(screen_rect.center(), modal_size);
            let inner_size = modal_size - egui::vec2(pad, pad);

            ui.scope_builder(egui::UiBuilder::new().max_rect(modal_rect), |ui| {
                crate::ui::theme::standard_panel_frame(is_mobile)
                    .show(ui, |ui| {
                        ui.set_min_size(inner_size);
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            ui.vertical_centered(|ui| {
                                crate::ui::theme::outlined_label(
                                    ui,
                                    "CHOOSE YOUR AVATAR",
                                    egui::FontId::proportional(22.0),
                                    Color32::WHITE,
                                );
                            });
                            ui.add_space(24.0);

                            let grid_w = inner_size.x;
                            let avatar_size = 64.0;
                            let spacing = (grid_w - (avatar_size * 4.0)) / 3.0;

                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
                                
                                for i in 0..8 {
                                    let is_selected = *selected_avatar_id == i;
                                    let (rect, response) = ui.allocate_exact_size(egui::vec2(avatar_size, avatar_size), egui::Sense::click());
                                    
                                    if response.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }

                                    let stroke_color = if is_selected {
                                        crate::ui::theme::avatar_pink()
                                    } else if response.hovered() {
                                        crate::ui::theme::avatar_cyan()
                                    } else {
                                        crate::ui::theme::nickname_field_border()
                                    };

                                    if let Some(tex) = asset_loader.avatars.get(i as usize) {
                                        let image = egui::Image::new(tex).fit_to_exact_size(rect.size()).corner_radius(CornerRadius::same(8));
                                        ui.put(rect, image);
                                    }
                                    
                                    ui.painter().rect_stroke(
                                        rect,
                                        8.0,
                                        Stroke::new(if is_selected { 2.0_f32 } else { 1.0_f32 }, stroke_color),
                                        egui::StrokeKind::Inside,
                                    );

                                    if response.clicked() {
                                        *selected_avatar_id = i;
                                        close = true;
                                    }
                                }
                            });

                            ui.add_space(32.0);
                            ui.vertical_centered(|ui| {
                                let btn = crate::widgets::ThemeButton::new("CONFIRM")
                                    .style(crate::widgets::ThemeButtonStyle::Primary)
                                    .min_size(egui::vec2(160.0, 44.0));
                                if ui.add(btn).clicked() {
                                    close = true;
                                }
                            });
                        });
                    });
            });
        });

    close
}
