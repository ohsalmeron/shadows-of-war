impl MapEditorSession {

    pub(crate) fn gameplay_map_globals(
        &self,
        logical_w: f32,
        logical_h: f32,
        hover_hex: [f32; 2],
    ) -> MapGlobals {
        MapGlobals {
            camera_pos: [self.camera_x, self.camera_y],
            zoom: self.camera_zoom,
            time: self.start_time.elapsed().as_secs_f32() % 1000.0,
            screen_size: [logical_w, logical_h],
            map_size: [self.width as f32, self.height as f32],
            border_thickness: 0.5,
            border_darkness: 0.35,
            shore_thickness: 1.0,
            shore_darkness: 1.0,
            threat_slots: [[0.0; 4]; 8],
            effect_shockwave: 1.0,
            effect_breathe: 1.0,
            effect_energy_flow: 1.0,
            my_player_id: 0.0,
            hover_hex,
            hover_building_kind: 0.0,
            territory_opacity: 1.0,
            fallout_slots: [[0.0; 4]; 8],
            nobuild_slots: [[0.0; 4]; 32],
            blend_mode: 0.0,
            effect_heartbeat: 1.0,
            effect_war_fog: 1.0,
            effect_fallout: 1.0,
            effect_golden_hour: 1.0,
            effect_holo_grid: 1.0,
            _pad3: 0.0,
            _pad4: 0.0,
        }
    }

    #[cfg(feature = "osm")]
    pub fn update(&mut self, _event_loop: &dyn ActiveEventLoop) -> Option<sow_ui::UiAction> {
        self.check_surface();
        let mut transition = None;

        let sf = self
            .window
            .as_ref()
            .map_or(1.0, |w| w.scale_factor() as f32);
        self.egui_ctx.set_pixels_per_point(sf);
        self.raw_input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(self.screen_w / sf, self.screen_h / sf),
        ));

        let dt = Instant::now()
            .duration_since(self.last_frame_time)
            .as_secs_f32();
        self.last_frame_time = Instant::now();
        self.raw_input.predicted_dt = dt.min(0.1);

        let lang = self.client_app.settings_state.language;

        static REGISTER_ONCE: std::sync::Once = std::sync::Once::new();
        REGISTER_ONCE.call_once(|| {
            sow_ui_kit::register_game_assets(&self.egui_ctx);
        });

        self.editor_ui.width = self.width;
        self.editor_ui.height = self.height;

        #[cfg(feature = "osm")]
        if self.editor_ui.mode == sow_ui::ui::map_editor::EditorMode::OsmPicker {
            self.update_osm_tiles();
        }

        #[cfg(feature = "osm")]
        let osm_view = if self.editor_ui.mode == sow_ui::ui::map_editor::EditorMode::OsmPicker {
            Some(self.build_osm_view())
        } else {
            None
        };
        #[cfg(target_arch = "wasm32")]
        let osm_view: Option<sow_ui::ui::map_editor::OsmPickerView> = None;

        let mut ui_action = sow_ui::ui::map_editor::MapEditorAction::None;
        let viewport = self.map_editor_viewport();
        let egui_ctx = self.egui_ctx.clone();
        sow_ui_kit::theme::publish_reduced_motion(
            &egui_ctx,
            self.client_app.settings_state.reduced_motion,
        );
        let egui_output = egui_ctx.run_ui(self.raw_input.clone(), |ui| {
            ui_action = sow_ui::ui::map_editor::draw_map_editor(
                ui,
                &egui_ctx,
                &mut self.editor_ui,
                viewport,
                osm_view.as_ref(),
                lang,
            );
        });

        #[cfg(feature = "osm")]
        if self.editor_ui.mode == sow_ui::ui::map_editor::EditorMode::OsmPicker {
            self.apply_osm_selection_from_screen();
        }

        if self.dragging {
            #[cfg(feature = "osm")]
            if self.editor_ui.mode == sow_ui::ui::map_editor::EditorMode::OsmPicker {
                self.pan_osm(self.pending_pan.0, self.pending_pan.1);
            } else {
                self.camera_x += self.pending_pan.0;
                self.camera_y += self.pending_pan.1;
            }
            #[cfg(target_arch = "wasm32")]
            {
                self.camera_x += self.pending_pan.0;
                self.camera_y += self.pending_pan.1;
            }
            self.pending_pan = (0.0, 0.0);
        }

        if self.primary_button_down
            && self.pointer_on_map_canvas()
            && self.editor_ui.mode == sow_ui::ui::map_editor::EditorMode::Brush
        {
            self.paint_at_cursor();
        }

        if ui_action == sow_ui::ui::map_editor::MapEditorAction::None {
            egui_ctx.input(|i| {
                if i.key_pressed(egui::Key::Z) && i.modifiers.command {
                    ui_action = sow_ui::ui::map_editor::MapEditorAction::Undo;
                } else if i.key_pressed(egui::Key::B)
                    && self.editor_ui.mode == sow_ui::ui::map_editor::EditorMode::OsmPicker
                {
                    ui_action = sow_ui::ui::map_editor::MapEditorAction::ExitOsmPicker;
                }
            });
        }

        match ui_action {
            sow_ui::ui::map_editor::MapEditorAction::Exit => {
                transition = Some(sow_ui::UiAction::LeaveLobby);
            }
            sow_ui::ui::map_editor::MapEditorAction::Export => {
                self.export_map_package();
            }
            sow_ui::ui::map_editor::MapEditorAction::ToggleNewDialog => {
                self.editor_ui.show_new_dialog = !self.editor_ui.show_new_dialog;
            }
            sow_ui::ui::map_editor::MapEditorAction::CreateBlankMap => {
                self.new_blank_map(self.editor_ui.new_map_w, self.editor_ui.new_map_h);
            }
            sow_ui::ui::map_editor::MapEditorAction::PlaceSpawn => {
                let cx = self.width / 2;
                let cy = self.height / 2;
                let idx = self.editor_ui.spawns.len() + 1;
                self.editor_ui
                    .spawns
                    .push(sow_ui::ui::map_editor::SpawnRowUi {
                        x: cx,
                        y: cy,
                        name: format!("Nation {}", idx),
                        flag: "🏳".to_string(),
                    });
                self.notify_info(&sow_i18n::get(lang).map_editor.msg_spawn_placed);
                self.mark_dirty();
            }
            sow_ui::ui::map_editor::MapEditorAction::RemoveSpawn(idx) => {
                self.push_undo_snapshot();
                self.editor_ui.spawns.remove(idx);
                self.notify_info(&sow_i18n::get(lang).map_editor.msg_spawn_removed);
                self.mark_dirty();
            }
            sow_ui::ui::map_editor::MapEditorAction::EnterOsmPicker => {
                #[cfg(feature = "osm")]
                {
                    self.editor_ui.npcs_panel_saved = self.editor_ui.show_npcs_panel;
                    self.editor_ui.show_npcs_panel = false;
                    self.editor_ui.mode = sow_ui::ui::map_editor::EditorMode::OsmPicker;
                    self.enter_osm_view();
                }
            }
            sow_ui::ui::map_editor::MapEditorAction::ExitOsmPicker => {
                self.editor_ui.mode = sow_ui::ui::map_editor::EditorMode::Brush;
                self.editor_ui.show_npcs_panel = self.editor_ui.npcs_panel_saved;
                #[cfg(feature = "osm")]
                {
                    self.editor_ui.osm_drag_anchor = None;
                    self.editor_ui.osm_selection_screen = None;
                    self.osm_picker.textures.clear();
                }
                self.ensure_brush_renderer();
            }
            sow_ui::ui::map_editor::MapEditorAction::GenerateFromOsm => {
                #[cfg(feature = "osm")]
                self.generate_from_osm();
            }
            sow_ui::ui::map_editor::MapEditorAction::Undo => {
                self.undo_last_stroke();
            }
            sow_ui::ui::map_editor::MapEditorAction::None => {}
        }

        self.raw_input.events.clear();

        let (logical_w, logical_h) = self.logical_screen();
        let hover_hex = if self.pointer_on_map_canvas() {
            let world_x = (self.last_mouse_logical_x - self.camera_x) / self.camera_zoom;
            let world_y = (self.last_mouse_logical_y - self.camera_y) / self.camera_zoom;
            [world_x.round(), world_y.round()]
        } else {
            [0.0, 0.0]
        };

        // Tesselate and upload UI delta textures
        let sf_fact = self
            .window
            .as_ref()
            .map_or(1.0, |w| w.scale_factor() as f32);

        let draw_terrain = self.editor_ui.mode == sow_ui::ui::map_editor::EditorMode::Brush;

        let terrain_globals = if draw_terrain {
            Some(self.gameplay_map_globals(logical_w, logical_h, hover_hex))
        } else {
            None
        };

        if let Some(ref mut gp) = self.gui_painter {
            if let Some(ref mut s) = self.surface {
                let frame = s.acquire_frame();
                if let Some(sp) = self.prev_sync_point.take() {
                    let _ = self.render_ctx.context.wait_for(&sp, !0);
                }

                self.render_ctx.command_encoder.start();
                self.render_ctx
                    .command_encoder
                    .init_texture(frame.texture());

                let screen_desc = blade_egui::ScreenDescriptor {
                    physical_size: (self.screen_w as u32, self.screen_h as u32),
                    scale_factor: sf_fact,
                };
                let paint_jobs = self.egui_ctx.tessellate(egui_output.shapes, sf_fact);
                gp.update_textures(
                    &mut self.render_ctx.command_encoder,
                    &egui_output.textures_delta,
                    &self.render_ctx.context,
                );

                // OSM mode: black clear; map tiles are drawn by egui in the central panel.
                #[cfg(feature = "osm")]
                if !draw_terrain {
                    let _pass = self.render_ctx.command_encoder.render(
                        "osm_bg_clear",
                        gpu::RenderTargetSet {
                            colors: &[gpu::RenderTarget {
                                view: frame.texture_view(),
                                init_op: gpu::InitOp::Clear(gpu::TextureColor::OpaqueBlack),
                                finish_op: gpu::FinishOp::Store,
                            }],
                            depth_stencil: None,
                        },
                    );
                }

                #[cfg(target_arch = "wasm32")]
                if !draw_terrain {
                    let _pass = self.render_ctx.command_encoder.render(
                        "osm_bg_clear",
                        gpu::RenderTargetSet {
                            colors: &[gpu::RenderTarget {
                                view: frame.texture_view(),
                                init_op: gpu::InitOp::Clear(gpu::TextureColor::OpaqueBlack),
                                finish_op: gpu::FinishOp::Store,
                            }],
                            depth_stencil: None,
                        },
                    );
                }

                if draw_terrain {
                    if let Some(ref mut mr) = self.map_renderer {
                        if self.needs_first_upload {
                            self.render_ctx
                                .command_encoder
                                .init_texture(mr.terrain_texture);
                            self.render_ctx
                                .command_encoder
                                .init_texture(mr.owner_texture);
                            self.needs_first_upload = false;
                            mr.upload_terrain(&mut self.render_ctx.command_encoder);
                        }

                        if self.needs_owner_upload {
                            mr.upload_initial_owners(
                                &mut self.render_ctx.command_encoder,
                                &self.render_ctx.context,
                            );
                            self.needs_owner_upload = false;
                        }

                        // Push dirty terrain tiles to GPU (editor brush strokes).
                        if !self.dirty_tiles.is_empty() {
                            for &idx in &self.dirty_tiles {
                                if idx < self.terrain.len() {
                                    mr.terrain[idx] = self.terrain[idx];
                                }
                            }
                            mr.sync_terrain_to_gpu(
                                &mut self.render_ctx.command_encoder,
                                &self.render_ctx.context,
                            );
                            self.dirty_tiles.clear();
                        }

                        // Render Map viewport
                        let mut player_colors = [[0.5, 0.5, 0.5, 1.0]; 256];
                        player_colors[1] = [0.1, 0.6, 0.9, 1.0];

                        let globals = terrain_globals.expect("terrain_globals set in brush mode");
                        let colors_struct = sow_render::PlayerColors {
                            colors: player_colors,
                        };

                        mr.draw(
                            &mut self.render_ctx.command_encoder,
                            frame.texture_view(),
                            globals,
                            colors_struct,
                        );
                    }
                }

                // Draw EGUI overlay on top of map viewport
                let mut pass = self.render_ctx.command_encoder.render(
                    "editor_ui_pass",
                    gpu::RenderTargetSet {
                        colors: &[gpu::RenderTarget {
                            view: frame.texture_view(),
                            init_op: gpu::InitOp::Load,
                            finish_op: gpu::FinishOp::Store,
                        }],
                        depth_stencil: None,
                    },
                );
                gp.paint(
                    &mut pass,
                    &paint_jobs,
                    &screen_desc,
                    &self.render_ctx.context,
                );
                drop(pass);
                gp.sync(&self.render_ctx.context);

                self.render_ctx.command_encoder.present(frame);
                let sync_point = self
                    .render_ctx
                    .context
                    .submit(&mut self.render_ctx.command_encoder);
                gp.after_submit(&sync_point);
                self.prev_sync_point = Some(sync_point);
            }
        }

        transition
    }

    /// Wait for in-flight GPU work and destroy editor map textures (splash exit step 1).
    pub fn teardown_gpu(&mut self) {
        if let Some(sp) = self.prev_sync_point.take() {
            let _ = self.render_ctx.context.wait_for(&sp, !0);
        }
        if let Some(mut mr) = self.map_renderer.take() {
            mr.destroy(&self.render_ctx);
        }
        self.render_ctx.reset_command_encoder();
    }

    #[allow(clippy::type_complexity)]
    pub fn destroy_and_reclaim(
        self,
    ) -> (
        Option<Box<dyn Window>>,
        Option<gpu::Surface>,
        RenderContext,
        Option<GuiPainter>,
        ClientApp,
        Context,
    ) {
        let mut this = std::mem::ManuallyDrop::new(self);

        this.teardown_gpu();

        unsafe {
            let window = std::ptr::read(&this.window);
            let surface = std::ptr::read(&this.surface);
            let render_ctx = std::ptr::read(&this.render_ctx);
            let gui_painter = std::ptr::read(&this.gui_painter);
            let client_app = std::ptr::read(&this.client_app);
            let egui_ctx = std::ptr::read(&this.egui_ctx);
            std::ptr::drop_in_place(&mut this.terrain);
            std::ptr::drop_in_place(&mut this.dirty_tiles);
            std::ptr::drop_in_place(&mut this.raw_input);
            std::ptr::drop_in_place(&mut this.editor_ui);
            #[cfg(feature = "osm")]
            std::ptr::drop_in_place(&mut this.osm_picker);
            (
                window,
                surface,
                render_ctx,
                gui_painter,
                client_app,
                egui_ctx,
            )
        }
    }
}

impl Drop for MapEditorSession {
    fn drop(&mut self) {
        self.teardown_gpu();
        if let Some(mut gp) = self.gui_painter.take() {
            gp.destroy(&self.render_ctx.context);
        }
        if let Some(mut s) = self.surface.take() {
            self.render_ctx.context.destroy_surface(&mut s);
        }
        // The command encoder is destroyed by `RenderContext`'s own `Drop`
        // when the `render_ctx` field is dropped after this.
    }
}
