use crate::app::SowApp;
use egui::{Color32, RichText, Vec2};
use sow_ui_kit::theme::dev_config::DevConfig;

impl SowApp {
    pub(super) fn render_dev_sidebar(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let mut cfg = DevConfig::get();
        let max_height = (ctx.content_rect().height() - ui.cursor().top() - 20.0).max(120.0);

        egui::ScrollArea::vertical()
            .id_salt("dev_sidebar_scroll")
            .max_height(max_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.style_mut().spacing.slider_width = 80.0;
                    ui.style_mut().spacing.item_spacing = Vec2::new(3.0, 3.0);
                    ui.style_mut().override_text_style = Some(egui::TextStyle::Small);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Dev Tools")
                        .strong()
                        .size(13.0)
                        .color(Color32::WHITE),
                );
            });
            ui.add_space(1.0);
            ui.separator();
            ui.add_space(1.0);

            ui.collapsing(
                RichText::new("Map & Borders")
                    .strong()
                    .size(11.5)
                    .color(Color32::WHITE),
                |ui| {
                    ui.add(egui::Slider::new(&mut cfg.thickness, 0.0..=1.0).text("Border Thk"));
                    ui.add(egui::Slider::new(&mut cfg.darkness, 0.0..=1.0).text("Border Drk"));
                    ui.add(
                        egui::Slider::new(&mut cfg.shore_thickness, 0.0..=1.0).text("Shore Thk"),
                    );
                    ui.add(
                        egui::Slider::new(&mut cfg.conquest_duration, 0.1..=10.0)
                            .text("Conquest Dur"),
                    );
                    ui.add(
                        egui::Slider::new(&mut cfg.territory_opacity, 0.0..=1.0).text("Opacity"),
                    );

                    egui::ComboBox::from_label("Blend Mode")
                        .selected_text(match cfg.blend_mode as i32 {
                            0 => "Normal Mix",
                            1 => "Multiply",
                            2 => "Overlay",
                            3 => "All Albedo",
                            _ => "Overlay",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut cfg.blend_mode, 0.0f32, "Normal Mix");
                            ui.selectable_value(&mut cfg.blend_mode, 1.0f32, "Multiply");
                            ui.selectable_value(&mut cfg.blend_mode, 2.0f32, "Overlay");
                            ui.selectable_value(&mut cfg.blend_mode, 3.0f32, "All Albedo");
                        });

                    if ui.button("Reset").clicked() {
                        let d = DevConfig::default();
                        cfg.thickness = d.thickness;
                        cfg.darkness = d.darkness;
                        cfg.shore_thickness = d.shore_thickness;
                        cfg.conquest_duration = d.conquest_duration;
                        cfg.territory_opacity = d.territory_opacity;
                        cfg.blend_mode = d.blend_mode;
                    }
                },
            );

            ui.collapsing(
                RichText::new("Custom HUD Theme")
                    .strong()
                    .size(11.5)
                    .color(Color32::WHITE),
                |ui| {
                    ui.add(egui::Slider::new(&mut cfg.theme_roundness, 0.0..=48.0).text("Round"));
                    ui.add(
                        egui::Slider::new(&mut cfg.theme_outline_thickness, 0.0..=10.0)
                            .text("Outline Thk"),
                    );
                    ui.add(
                        egui::Slider::new(&mut cfg.theme_glow_spread, 0.0..=4.0)
                            .text("Glow Spread"),
                    );
                    ui.add(
                        egui::Slider::new(&mut cfg.theme_glow_thickness, 0.0..=5.0)
                            .text("Glow Thk"),
                    );
                    ui.horizontal(|ui| {
                        ui.label("Top Color:");
                        ui.color_edit_button_rgba_unmultiplied(&mut cfg.theme_color_top);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Bot Color:");
                        ui.color_edit_button_rgba_unmultiplied(&mut cfg.theme_color_bottom);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Outline:");
                        ui.color_edit_button_rgba_unmultiplied(&mut cfg.theme_color_outline);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Glow:");
                        ui.color_edit_button_rgba_unmultiplied(&mut cfg.theme_color_glow);
                    });

                    if ui.button("Reset").clicked() {
                        let d = DevConfig::default();
                        cfg.theme_roundness = d.theme_roundness;
                        cfg.theme_outline_thickness = d.theme_outline_thickness;
                        cfg.theme_glow_spread = d.theme_glow_spread;
                        cfg.theme_glow_thickness = d.theme_glow_thickness;
                        cfg.theme_color_top = d.theme_color_top;
                        cfg.theme_color_bottom = d.theme_color_bottom;
                        cfg.theme_color_outline = d.theme_color_outline;
                        cfg.theme_color_glow = d.theme_color_glow;
                    }
                },
            );

            ui.separator();

            ui.collapsing(
                RichText::new("Font Settings (SDF)")
                    .strong()
                    .size(11.5)
                    .color(Color32::WHITE),
                |ui| {
                    ui.add(
                        egui::Slider::new(&mut cfg.font_size_scale, 0.5..=2.5).text("Font Size"),
                    );
                    ui.add(
                        egui::Slider::new(&mut cfg.font_face_dilate, -1.0..=2.0)
                            .text("Face Dilate"),
                    );
                    ui.add(
                        egui::Slider::new(&mut cfg.font_outline_thickness, 0.0..=3.0)
                            .text("Outline"),
                    );
                    ui.add(egui::Slider::new(&mut cfg.font_shadow_y, 0.0..=5.0).text("Shadow Y"));
                    ui.add(
                        egui::Slider::new(&mut cfg.font_underlay_softness, 0.0..=2.0)
                            .text("Softness"),
                    );
                    ui.add(
                        egui::Slider::new(&mut cfg.font_char_spacing, 0.8..=1.8).text("Spacing"),
                    );
                    ui.add(
                        egui::Slider::new(&mut cfg.font_offset_x, -16.0..=16.0).text("Offset X"),
                    );

                    if ui.button("Reset").clicked() {
                        let d = DevConfig::default();
                        cfg.font_face_dilate = d.font_face_dilate;
                        cfg.font_outline_thickness = d.font_outline_thickness;
                        cfg.font_shadow_y = d.font_shadow_y;
                        cfg.font_underlay_softness = d.font_underlay_softness;
                        cfg.font_char_spacing = d.font_char_spacing;
                        cfg.font_size_scale = d.font_size_scale;
                        cfg.font_offset_x = d.font_offset_x;
                    }
                },
            );

            ui.separator();
            ui.collapsing(
                RichText::new("Building Settings")
                    .strong()
                    .size(11.5)
                    .color(Color32::WHITE),
                |ui| {
                    ui.add(egui::Slider::new(&mut cfg.building_scale, 0.3..=3.0).text("Scale"));
                    ui.add(
                        egui::Slider::new(&mut cfg.emoji_size_scale, 0.5..=3.0).text("Emoji Size"),
                    );

                    ui.separator();
                    ui.add(egui::Checkbox::new(
                        &mut cfg.clamp_text_zoom,
                        "Clamp Text Zoom",
                    ));
                    ui.add(egui::Checkbox::new(
                        &mut cfg.clamp_emoji_zoom,
                        "Clamp Emoji Zoom",
                    ));

                    if ui.button("Reset").clicked() {
                        let d = DevConfig::default();
                        cfg.building_scale = d.building_scale;
                        cfg.emoji_size_scale = d.emoji_size_scale;
                        cfg.clamp_text_zoom = d.clamp_text_zoom;
                        cfg.clamp_emoji_zoom = d.clamp_emoji_zoom;
                    }
                },
            );

            ui.separator();
            ui.collapsing(
                RichText::new("Objectives Bar")
                    .strong()
                    .size(11.5)
                    .color(Color32::WHITE),
                |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Filler Top:");
                        ui.color_edit_button_rgba_unmultiplied(&mut cfg.obj_filler_top);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Filler Bot:");
                        ui.color_edit_button_rgba_unmultiplied(&mut cfg.obj_filler_bottom);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Backplate Top:");
                        ui.color_edit_button_rgba_unmultiplied(&mut cfg.obj_backplate_top);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Backplate Bot:");
                        ui.color_edit_button_rgba_unmultiplied(&mut cfg.obj_backplate_bottom);
                    });
                    if ui.button("Reset").clicked() {
                        let d = DevConfig::default();
                        cfg.obj_filler_top = d.obj_filler_top;
                        cfg.obj_filler_bottom = d.obj_filler_bottom;
                        cfg.obj_backplate_top = d.obj_backplate_top;
                        cfg.obj_backplate_bottom = d.obj_backplate_bottom;
                    }
                },
            );

            ui.separator();
            ui.collapsing(
                RichText::new("Bunker Laser")
                    .strong()
                    .size(11.5)
                    .color(Color32::WHITE),
                |ui| {
                    ui.add(egui::Checkbox::new(
                        &mut cfg.bunker_laser_target,
                        "Target seeking",
                    ));
                    ui.add(egui::Checkbox::new(&mut cfg.bunker_laser_arc, "Plasma arc"));
                    ui.add(egui::Checkbox::new(
                        &mut cfg.bunker_laser_scatter,
                        "Volley scatter",
                    ));

                    if ui.button("Reset").clicked() {
                        let d = DevConfig::default();
                        cfg.bunker_laser_target = d.bunker_laser_target;
                        cfg.bunker_laser_arc = d.bunker_laser_arc;
                        cfg.bunker_laser_scatter = d.bunker_laser_scatter;
                    }
                },
            );

            ui.separator();
            ui.collapsing(
                RichText::new("VFX Toggles (Benchmark)")
                    .strong()
                    .size(11.5)
                    .color(Color32::WHITE),
                |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("All On").clicked() {
                            cfg.vfx_conquer = true;
                            cfg.vfx_border_breathe = true;
                            cfg.vfx_energy_flow = true;
                            cfg.vfx_heartbeat = true;
                            cfg.vfx_war_fog = true;
                            cfg.fog_of_war = true;
                            cfg.vfx_fallout = true;
                            cfg.vfx_ambient_grade = true;
                            cfg.vfx_holo_grid = true;
                            cfg.vfx_tower = true;
                            cfg.vfx_tower_range = true;
                            cfg.vfx_attack_lines = true;
                            cfg.vfx_attack_badges = true;
                            cfg.vfx_click_markers = true;
                            cfg.vfx_nuke_preview = true;
                            cfg.vfx_floating_notices = true;
                            cfg.vfx_status_emojis = true;
                            cfg.vfx_upgrade_plate = true;
                            cfg.vfx_placement_preview = true;
                            cfg.vfx_world_buildings = true;
                            cfg.vfx_mover_trails = true;
                            cfg.vfx_railways = true;
                            cfg.vfx_fleet_blink = true;
                            cfg.vfx_bot_avatars = true;
                            cfg.vfx_nameplate_names = true;
                            cfg.vfx_nameplate_troops = true;
                        }
                        if ui.button("All Off").clicked() {
                            cfg.vfx_conquer = false;
                            cfg.vfx_border_breathe = false;
                            cfg.vfx_energy_flow = false;
                            cfg.vfx_heartbeat = false;
                            cfg.vfx_war_fog = false;
                            cfg.fog_of_war = false;
                            cfg.vfx_fallout = false;
                            cfg.vfx_ambient_grade = false;
                            cfg.vfx_holo_grid = false;
                            cfg.vfx_tower = false;
                            cfg.vfx_tower_range = false;
                            cfg.vfx_attack_lines = false;
                            cfg.vfx_attack_badges = false;
                            cfg.vfx_click_markers = false;
                            cfg.vfx_nuke_preview = false;
                            cfg.vfx_floating_notices = false;
                            cfg.vfx_status_emojis = false;
                            cfg.vfx_upgrade_plate = false;
                            cfg.vfx_placement_preview = false;
                            cfg.vfx_world_buildings = false;
                            cfg.vfx_mover_trails = false;
                            cfg.vfx_railways = false;
                            cfg.vfx_fleet_blink = false;
                            cfg.vfx_bot_avatars = false;
                            cfg.vfx_nameplate_names = false;
                            cfg.vfx_nameplate_troops = false;
                        }
                    });

                    ui.small("GPU Effects");
                    ui.checkbox(&mut cfg.vfx_conquer, "Conquer shockwave");
                    ui.checkbox(&mut cfg.vfx_border_breathe, "Border breathe");
                    ui.checkbox(&mut cfg.vfx_energy_flow, "Contested shimmer");
                    ui.checkbox(&mut cfg.vfx_heartbeat, "Territory heartbeat");
                    ui.checkbox(&mut cfg.vfx_war_fog, "War fog / Frontier");
                    ui.checkbox(&mut cfg.fog_of_war, "Fog of War");
                    ui.checkbox(&mut cfg.vfx_fallout, "Nuclear fallout");
                    ui.checkbox(&mut cfg.vfx_ambient_grade, "Ambient grading");
                    ui.checkbox(&mut cfg.vfx_holo_grid, "Holographic grid");

                    ui.separator();
                    ui.small("Tower & Combat VFX");
                    ui.checkbox(&mut cfg.vfx_tower, "Bunker laser");
                    ui.checkbox(&mut cfg.vfx_tower_range, "Bunker range circle");
                    ui.checkbox(&mut cfg.vfx_attack_lines, "Attack threat lines");
                    ui.checkbox(&mut cfg.vfx_attack_badges, "Attack troop badges");

                    ui.separator();
                    ui.small("World & UI VFX");
                    ui.checkbox(&mut cfg.vfx_click_markers, "Click markers");
                    ui.checkbox(&mut cfg.vfx_nuke_preview, "Nuke preview");
                    ui.checkbox(&mut cfg.vfx_floating_notices, "Floating notices");
                    ui.checkbox(&mut cfg.vfx_status_emojis, "Status emojis");
                    ui.checkbox(&mut cfg.vfx_upgrade_plate, "Upgrade plate");
                    ui.checkbox(&mut cfg.vfx_placement_preview, "Placement preview");
                    ui.checkbox(&mut cfg.vfx_world_buildings, "World buildings");
                    ui.checkbox(&mut cfg.vfx_mover_trails, "Mover trails");
                    ui.checkbox(&mut cfg.vfx_railways, "Railways");
                    ui.checkbox(&mut cfg.vfx_fleet_blink, "Fleet retreat cross");

                    ui.separator();
                    ui.small("Nameplate Benchmark");
                    ui.checkbox(&mut cfg.vfx_bot_avatars, "Bot avatars");
                    ui.checkbox(&mut cfg.vfx_nameplate_names, "Nameplate names");
                    ui.checkbox(&mut cfg.vfx_nameplate_troops, "Nameplate troops");

                    if ui.button("Reset").clicked() {
                        DevConfig::set(DevConfig::default());
                    }
                },
            );
        });
    });

    DevConfig::set(cfg);
}
}
