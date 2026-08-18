use egui::{Color32, Context, ScrollArea, Ui, pos2, vec2};
use crate::kit::components::{
    BodyText, Button, Card, Heading, Modal, Subtitle, checkbox, slider,
};
use sow_ui_kit::theme::palette;

#[derive(Default)]
pub struct ShowcaseState {
    pub click_count: u32,
    pub checkbox_a: bool,
    pub checkbox_b: bool,
    pub slider_val: f32,
    pub modal_open: bool,
}

pub fn draw_showcase(ctx: &Context, state: &mut ShowcaseState, open: &mut bool) {
    if !*open {
        return;
    }

    egui::Area::new(egui::Id::new("ui_kit_showcase_root"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(0.0, 0.0))
        .show(ctx, |ui| {
            let screen = ctx.content_rect();
            ui.set_width(screen.width());
            ui.set_height(screen.height());

            // Dark Backdrop
            ui.painter().rect_filled(screen, 0.0, Color32::from_rgb(8, 9, 12));

            // Top Navigation Bar
            let nav_h = 60.0;
            let nav_rect = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(screen.width(), nav_h));
            ui.painter().rect_filled(
                nav_rect,
                0.0,
                palette::surface(),
            );
            ui.painter().line_segment(
                [nav_rect.left_bottom(), nav_rect.right_bottom()],
                egui::Stroke::new(1.0_f32, palette::field_border()),
            );

            ui.scope_builder(egui::UiBuilder::new().max_rect(nav_rect), |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    Heading::new("UI KIT SHOWCASE").cyan().size(22.0).show(ui);
                    Subtitle::new("• SHADOWS OF WAR DESIGN SYSTEM").muted().size(14.0).show(ui);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(24.0);
                        if Button::primary("CLOSE / VOLVER").show(ui).clicked() {
                            *open = false;
                        }
                    });
                });
            });

            // Main Scrollable Content
            let content_rect = egui::Rect::from_min_max(
                pos2(0.0, nav_h),
                pos2(screen.width(), screen.height()),
            );

            ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(24.0);
                        let max_w = 900.0_f32.min(screen.width() - 48.0);

                        ui.vertical_centered(|ui| {
                            ui.set_width(max_w);

                            // --- 1. TYPOGRAPHY SECTION ---
                            draw_section_header(ui, "1. TYPOGRAPHY & 7-PASS GLOW");
                            Card::surface().show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    Heading::new("HEADING 28PX CYAN").cyan().show(ui);
                                    Heading::new("HEADING GOLD").gold().show(ui);
                                });
                                ui.add_space(8.0);
                                Subtitle::new("SUBTITLE 18PX CRISP OUTLINE").show(ui);
                                Subtitle::new("SUBTITLE MUTED TEXT").muted().show(ui);
                                ui.add_space(8.0);
                                BodyText::new("BodyText paragraph with automatic line wrap. Built on WorkSans-Black with clean high-contrast legibility and authentic outline.")
                                    .wrap(max_w - 40.0)
                                    .show(ui);
                            });

                            ui.add_space(24.0);

                            // --- 2. BUTTONS SECTION ---
                            draw_section_header(ui, "2. BUTTONS & INTERACTION");
                            Card::surface().show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    if Button::primary("PRIMARY").show(ui).clicked() {
                                        state.click_count += 1;
                                    }
                                    if Button::secondary("SECONDARY").show(ui).clicked() {
                                        state.click_count += 1;
                                    }
                                    if Button::ghost("GHOST").show(ui).clicked() {
                                        state.click_count += 1;
                                    }
                                    if Button::danger("DANGER").show(ui).clicked() {
                                        state.click_count += 1;
                                    }
                                    Button::primary("DISABLED").disabled(true).show(ui);
                                });

                                ui.add_space(12.0);
                                ui.horizontal(|ui| {
                                    Button::primary("SMALL").small().show(ui);
                                    Button::primary("MEDIUM").show(ui);
                                    Button::primary("LARGE").large().show(ui);
                                });

                                ui.add_space(12.0);
                                BodyText::new(&format!("Interaction counter: {} clicks recorded", state.click_count))
                                    .color(palette::neon_cyan())
                                    .show(ui);
                            });

                            ui.add_space(24.0);

                            // --- 3. CARDS & SURFACES ---
                            draw_section_header(ui, "3. CARDS & SURFACES");
                            ui.horizontal(|ui| {
                                let card_w = (max_w - 24.0) * 0.5;
                                Card::surface().min_width(card_w).show(ui, |ui| {
                                    Subtitle::new("SURFACE CARD").show(ui);
                                    BodyText::new("Default pitch black container with subtle border.")
                                        .muted()
                                        .show(ui);
                                });

                                Card::accent().min_width(card_w).show(ui, |ui| {
                                    Subtitle::new("ACCENT CARD").cyan().show(ui);
                                    BodyText::new("Highlighted container with neon cyan active border.")
                                        .muted()
                                        .show(ui);
                                });
                            });

                            ui.add_space(24.0);

                            // --- 4. FORM CONTROLS ---
                            draw_section_header(ui, "4. FORM CONTROLS");
                            Card::surface().show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    checkbox(ui, &mut state.checkbox_a, "Sound Effects Active");
                                    ui.add_space(24.0);
                                    checkbox(ui, &mut state.checkbox_b, "Hardware Acceleration");
                                });
                                ui.add_space(12.0);
                                slider(ui, &mut state.slider_val, 0.0..=100.0, "Master Volume %: ");
                            });

                            ui.add_space(24.0);

                            // --- 5. MODAL PREVIEW ---
                            draw_section_header(ui, "5. MODAL DIALOG COMPONENT");
                            Card::surface().show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    BodyText::new("Click to trigger a modal component overlay:")
                                        .show(ui);
                                    if Button::primary("OPEN MODAL PREVIEW").show(ui).clicked() {
                                        state.modal_open = true;
                                    }
                                });
                            });

                            ui.add_space(48.0);
                        });
                    });
            });

            // Live Modal Dialog Trigger
            Modal::new("showcase_sample_modal", "SAMPLE COMPONENT MODAL")
                .show(ctx, &mut state.modal_open, |ui| {
                    BodyText::new("This modal is composed purely from the new Kit primitives. It handles the fullscreen backdrop, title bar glow, click-outside dismissal and action buttons automatically.")
                        .show(ui);
                    ui.add_space(16.0);
                    Card::inset().show(ui, |ui| {
                        BodyText::new("+ Fully isolated\n+ Zero manual pixel math in callers\n+ Poka-yoke contract")
                            .color(palette::neon_cyan())
                            .show(ui);
                    });
                });
        });
}

fn draw_section_header(ui: &mut Ui, title: &str) {
    ui.horizontal(|ui| {
        Subtitle::new(title).gold().size(15.0).show(ui);
    });
    ui.add_space(6.0);
}
