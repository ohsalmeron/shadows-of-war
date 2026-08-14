pub fn draw_map_editor(
    ui: &mut Ui,
    ctx: &Context,
    state: &mut MapEditorUiState,
    viewport: MapEditorViewport,
    osm_view: Option<&OsmPickerView>,
    lang: sow_i18n::Language,
) -> MapEditorAction {
    let strings = &sow_i18n::get(lang).map_editor;
    let compact = sow_ui_kit::theme::compact_viewport(ctx);
    let busy = state.is_busy();
    let mut action = MapEditorAction::None;
    state.map_canvas_rect = None;
    if state.mode != EditorMode::OsmPicker {
        state.osm_drag_anchor = None;
        state.osm_selection_screen = None;
    }

    let top_frame = sow_ui_kit::theme::map_editor_glass_frame(
        sow_ui_kit::theme::MapEditorGlassPanel::Top,
        compact,
    );
    let side_frame = sow_ui_kit::theme::map_editor_glass_frame(
        sow_ui_kit::theme::MapEditorGlassPanel::Side,
        compact,
    );

    egui::Panel::top("editor_menu")
        .frame(top_frame)
        .show_inside(ui, |ui| {
            let rail_fill = sow_ui_kit::theme::palette::button_inactive();

            // Row 1: title + exit
            ui.horizontal(|ui| {
                crate::widgets::outlined_emoji_label(
                    ui,
                    &strings.title,
                    egui::FontId::proportional(18.0),
                    Color32::WHITE,
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        let exit_resp = toolbar_button(
                            ui,
                            &strings.btn_exit,
                            ThemeButtonStyle::Tertiary,
                            Some(rail_fill),
                        );
                        if exit_resp.clicked() {
                            if state.is_dirty {
                                state.show_exit_confirm = true;
                            } else {
                                action = MapEditorAction::Exit;
                            }
                        }
                    });
                });
            });

            ui.add_space(6.0);

            // Row 2: map name, actions, size
            ui.horizontal(|ui| {
                ui.label(RichText::new(&strings.label_map_name).size(13.0).strong());
                ui.add(
                    egui::TextEdit::singleline(&mut state.map_name)
                        .desired_width(140.0)
                        .hint_text("my_map"),
                );
                let slug = sow_core::maps::map_key(&state.map_name);
                ui.label(
                    RichText::new(strings.label_map_slug_hint.replace("{}", &slug))
                        .size(11.0)
                        .color(sow_ui_kit::theme::palette::text_muted()),
                );

                ui.separator();

                ui.add_enabled_ui(!busy, |ui| {
                    if toolbar_button(
                        ui,
                        &strings.btn_new,
                        ThemeButtonStyle::Tertiary,
                        Some(rail_fill),
                    )
                    .clicked()
                    {
                        action = MapEditorAction::ToggleNewDialog;
                    }
                });

                ui.separator();

                if state.mode == EditorMode::Brush {
                    #[cfg(not(target_arch = "wasm32"))]
                    ui.add_enabled_ui(!busy, |ui| {
                        if toolbar_button(
                            ui,
                            &strings.btn_from_osm,
                            ThemeButtonStyle::Secondary,
                            None,
                        )
                        .clicked()
                        {
                            action = MapEditorAction::EnterOsmPicker;
                        }
                    });
                } else {
                    ui.add_enabled_ui(!busy, |ui| {
                        let back_resp = toolbar_button(
                            ui,
                            &strings.btn_back_to_brush,
                            ThemeButtonStyle::Secondary,
                            None,
                        )
                        .on_hover_text(&strings.tooltip_back_to_brush);
                        if back_resp.clicked() {
                            action = MapEditorAction::ExitOsmPicker;
                        }
                    });
                }

                ui.separator();

                if state.mode == EditorMode::Brush {
                    ui.add_enabled_ui(!busy, |ui| {
                        let export_label = {
                            #[cfg(target_arch = "wasm32")]
                            {
                                &strings.btn_export_download
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                &strings.btn_export
                            }
                        };
                        let export_resp =
                            toolbar_button(ui, export_label, ThemeButtonStyle::Primary, None)
                                .on_hover_text(&strings.tooltip_export);
                        if export_resp.clicked() {
                            state.show_export_confirm = true;
                        }
                    });
                } else {
                    let has_selection = osm_view.and_then(|v| v.selection_screen_rect).is_some();
                    let mut generate_btn = ThemeButton::new(&strings.btn_generate_osm)
                        .style(if has_selection {
                            ThemeButtonStyle::Primary
                        } else {
                            ThemeButtonStyle::Tertiary
                        })
                        .min_size(Vec2::new(TOOLBAR_BTN_MIN_W, TOOLBAR_BTN_H))
                        .text_size(TOOLBAR_TEXT);
                    if !has_selection {
                        generate_btn =
                            generate_btn.custom_fill(sow_ui_kit::theme::palette::button_inactive());
                    }
                    let generate_resp = ui.add_enabled(has_selection && !busy, generate_btn);
                    if !has_selection {
                        generate_resp.on_hover_text(&strings.msg_osm_no_selection);
                    } else if generate_resp.clicked() {
                        action = MapEditorAction::GenerateFromOsm;
                    }
                }

                ui.separator();

                ui.label(
                    RichText::new(
                        strings
                            .label_size
                            .replacen("{}", &state.width.to_string(), 1)
                            .replacen("{}", &state.height.to_string(), 1),
                    )
                    .size(13.0)
                    .color(sow_ui_kit::theme::palette::text_muted()),
                );
            });

            if busy {
                ui.add_space(6.0);
                if let Some(ref msg) = state.busy_message {
                    ui.label(
                        RichText::new(msg)
                            .size(13.0)
                            .color(sow_ui_kit::theme::palette::text_muted()),
                    );
                }
                ui.add(
                    egui::ProgressBar::new(0.0)
                        .animate(true)
                        .fill(sow_ui_kit::theme::palette::neon_cyan()),
                );
            }
        });

    egui::Panel::left("tools_panel")
        .default_size(240.0)
        .frame(side_frame)
        .show_inside(ui, |ui| {
            ui.heading(&strings.heading_tools);
            ui.separator();
            ui.add_space(8.0);

            if state.mode == EditorMode::OsmPicker {
                let view = osm_view.cloned().unwrap_or_default();
                ui.label(RichText::new(&strings.heading_osm).strong());
                ui.add_space(6.0);
                ui.label(strings.label_osm_zoom.replace("{}", &view.zoom.to_string()));
                ui.label(
                    strings
                        .label_osm_center
                        .replacen("{}", &format!("{:.4}", view.center_lon), 1)
                        .replacen("{}", &format!("{:.4}", view.center_lat), 1),
                );
                ui.add_space(8.0);
                ui.label(&strings.label_osm_target);
                ui.add(egui::DragValue::new(&mut state.osm.target_size).range(100..=1000));
                state.osm.target_size = state
                    .osm
                    .target_size
                    .clamp(100, sow_core::maps::MAX_MAP_AXIS);
                state.osm.target_size -= state.osm.target_size % 4;
                ui.add_space(12.0);
                ui.small(&strings.instructions_osm);
                if let Some((min_lon, min_lat, max_lon, max_lat)) = view.selection_bbox {
                    ui.add_space(6.0);
                    ui.label(
                        strings
                            .label_osm_bbox_sw
                            .replace("{}", &format!("{min_lon:.3}, {min_lat:.3}")),
                    );
                    ui.label(
                        strings
                            .label_osm_bbox_ne
                            .replace("{}", &format!("{max_lon:.3}, {max_lat:.3}")),
                    );
                    if let Some(n) = view.overpass_tile_estimate {
                        ui.label(
                            strings
                                .label_osm_overpass_tiles
                                .replace("{}", &n.to_string()),
                        );
                        if n > 144 {
                            ui.colored_label(
                                egui::Color32::from_rgb(220, 120, 80),
                                &strings.hint_osm_overpass_limit,
                            );
                        }
                    }
                }
                ui.add_space(8.0);
                ui.small(&strings.osm_attribution);
                if state.osm.generating {
                    ui.add_space(8.0);
                    if let Some(ref msg) = state.busy_message {
                        ui.label(msg);
                    }
                    ui.add(egui::ProgressBar::new(0.0).animate(true));
                }
            } else {
                ui.label(RichText::new(&strings.heading_brush).strong());
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    let undo_resp = toolbar_button(
                        ui,
                        &strings.btn_undo,
                        ThemeButtonStyle::Tertiary,
                        Some(sow_ui_kit::theme::palette::button_inactive()),
                    )
                    .on_hover_text(&strings.tooltip_undo);
                    if undo_resp.clicked() {
                        action = MapEditorAction::Undo;
                    }
                });
                ui.add_space(6.0);

                ui.label(&strings.label_terrain);
                if paint_chip(
                    ui,
                    &strings.paint_plains,
                    state.selected_paint == EditorPaintKind::Plains,
                )
                .clicked()
                {
                    state.selected_paint = EditorPaintKind::Plains;
                }
                if paint_chip(
                    ui,
                    &strings.paint_highlands,
                    state.selected_paint == EditorPaintKind::Highlands,
                )
                .clicked()
                {
                    state.selected_paint = EditorPaintKind::Highlands;
                }
                if paint_chip(
                    ui,
                    &strings.paint_mountains,
                    state.selected_paint == EditorPaintKind::Mountains,
                )
                .clicked()
                {
                    state.selected_paint = EditorPaintKind::Mountains;
                }
                if paint_chip(
                    ui,
                    &strings.paint_lake,
                    state.selected_paint == EditorPaintKind::Water,
                )
                .clicked()
                {
                    state.selected_paint = EditorPaintKind::Water;
                }
                if paint_chip(
                    ui,
                    &strings.paint_ocean,
                    state.selected_paint == EditorPaintKind::Ocean,
                )
                .clicked()
                {
                    state.selected_paint = EditorPaintKind::Ocean;
                }
                if paint_chip(
                    ui,
                    &strings.paint_shoreline,
                    state.selected_paint == EditorPaintKind::Shoreline,
                )
                .clicked()
                {
                    state.selected_paint = EditorPaintKind::Shoreline;
                }

                ui.add_space(15.0);
                ui.label(
                    strings
                        .label_brush_size
                        .replace("{}", &state.brush_size.to_string()),
                );
                ui.add(egui::Slider::new(&mut state.brush_size, 1..=20).show_value(false));

                ui.add_space(15.0);
                ui.label(
                    strings
                        .label_strength
                        .replace("{:.1}", &format!("{:.1}", state.brush_strength)),
                );
                ui.add(egui::Slider::new(&mut state.brush_strength, 1.0..=31.0).show_value(false));

                ui.add_space(30.0);
                ui.heading(&strings.heading_instructions);
                ui.small(&strings.instructions_body);
                if !compact {
                    ui.add_space(4.0);
                    ui.small(
                        RichText::new(&strings.hint_shortcuts)
                            .color(sow_ui_kit::theme::palette::text_muted()),
                    );
                }
            }
        });

    if state.mode == EditorMode::Brush {
        egui::Panel::right("npcs_panel")
            .default_size(260.0)
            .frame(side_frame)
            .show_inside(ui, |ui| {
                let response = egui::CollapsingHeader::new(&strings.heading_npcs)
                    .default_open(state.show_npcs_panel)
                    .show(ui, |ui| {
                        ui.add_space(6.0);

                        if toolbar_button(
                            ui,
                            &strings.btn_place_spawn,
                            ThemeButtonStyle::Secondary,
                            None,
                        )
                        .clicked()
                        {
                            action = MapEditorAction::PlaceSpawn;
                        }

                        ui.add_space(12.0);
                        ui.label(&strings.label_placed_spawns);

                        let mut to_remove = None;
                        egui::ScrollArea::vertical()
                            .max_height(280.0)
                            .show(ui, |ui| {
                                for (i, spawn) in state.spawns.iter_mut().enumerate() {
                                    ui.group(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.text_edit_singleline(&mut spawn.name)
                                                .on_hover_text(&strings.hover_nation_name);
                                            ui.text_edit_singleline(&mut spawn.flag)
                                                .on_hover_text(&strings.hover_flag);
                                            if ui
                                                .add(
                                                    ThemeButton::new("🗑")
                                                        .style(ThemeButtonStyle::Danger)
                                                        .min_size(Vec2::new(28.0, 28.0))
                                                        .text_size(14.0),
                                                )
                                                .clicked()
                                            {
                                                to_remove = Some(i);
                                            }
                                        });
                                        ui.horizontal(|ui| {
                                            ui.label("X:");
                                            ui.add(egui::DragValue::new(&mut spawn.x));
                                            ui.label("Y:");
                                            ui.add(egui::DragValue::new(&mut spawn.y));
                                        });
                                    });
                                }
                            });

                        if let Some(idx) = to_remove {
                            action = MapEditorAction::RemoveSpawn(idx);
                        }
                    });
                state.show_npcs_panel = response.fully_open();
            });
    }

    // Pass pointer events through the map viewport; draw spawn markers and brush preview here.
    egui::CentralPanel::default()
        .frame(Frame::NONE)
        .show_inside(ui, |ui| {
            let map_rect = ui.max_rect();
            state.map_canvas_rect = Some(map_rect);
            if state.mode == EditorMode::OsmPicker {
                if let Some(view) = osm_view {
                    draw_osm_picker_canvas(ui, view, state);
                }
            } else {
                ui.allocate_rect(map_rect, Sense::empty());
                draw_viewport_overlay(ui, viewport, state);
            }
        });

    if state.show_new_dialog {
        let mut open = true;
        egui::Window::new(&strings.win_new_title)
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .frame(sow_ui_kit::theme::standard_panel_frame(compact))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(&strings.label_width);
                    ui.add(egui::DragValue::new(&mut state.new_map_w));
                    state.new_map_w = state.new_map_w.clamp(100, sow_core::maps::MAX_MAP_AXIS);
                });
                ui.horizontal(|ui| {
                    ui.label(&strings.label_height);
                    ui.add(egui::DragValue::new(&mut state.new_map_h));
                    state.new_map_h = state.new_map_h.clamp(100, sow_core::maps::MAX_MAP_AXIS);
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            ThemeButton::new(&strings.btn_create_map)
                                .style(ThemeButtonStyle::Primary)
                                .text_size(14.0),
                        )
                        .clicked()
                    {
                        action = MapEditorAction::CreateBlankMap;
                        state.show_new_dialog = false;
                    }
                    ui.add_space(8.0);
                    if ui
                        .add(
                            ThemeButton::new(&strings.btn_cancel)
                                .style(ThemeButtonStyle::Tertiary)
                                .text_size(14.0),
                        )
                        .clicked()
                    {
                        state.show_new_dialog = false;
                    }
                });
            });
        if !open {
            state.show_new_dialog = false;
        }
    }

    draw_confirm_dialog(
        ctx,
        (&strings.confirm_exit_title, &strings.confirm_exit_body),
        (&strings.confirm_yes, &strings.confirm_no),
        &mut state.show_exit_confirm,
        compact,
        || MapEditorAction::Exit,
        &mut action,
    );

    let slug = sow_core::maps::map_key(&state.map_name);
    let export_body = strings.confirm_export_body.replace("{}", &slug);
    draw_confirm_dialog(
        ctx,
        (&strings.confirm_export_title, &export_body),
        (&strings.confirm_yes, &strings.confirm_no),
        &mut state.show_export_confirm,
        compact,
        || MapEditorAction::Export,
        &mut action,
    );

    draw_toast(ctx, state);

    action
}

