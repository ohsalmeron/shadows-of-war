use blade_egui::GuiPainter;
use blade_graphics as gpu;
use egui::Context;
use sow_render::{MapGlobals, MapRenderer, RenderContext};
use sow_ui::ClientApp;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
use web_time::Instant;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use super::state::*;

impl MapEditorSession {
    pub fn new(
        window: Box<dyn Window>,
        surface: gpu::Surface,
        render_ctx: RenderContext,
        gui_painter: GuiPainter,
        egui_ctx: Context,
        client_app: ClientApp,
    ) -> Self {
        sow_ui::ui::theme::apply_theme(&egui_ctx);
        let width = 400;
        let height = 300;
        let size = (width * height) as usize;

        // Start with a basic water background
        // bit 7: is_land, bit 6: is_shoreline, bit 5: is_ocean
        let terrain = vec![0b00100000; size]; // Default ocean tiles

        let format = surface.info().format;
        let map_renderer = MapRenderer::new(&render_ctx.context, width, height, format, &terrain);

        let sz = window.surface_size();
        let screen_w = sz.width as f32;
        let screen_h = sz.height as f32;
        let sf = window.scale_factor() as f32;
        let camera_zoom = 1.0f32;
        let camera_x = (screen_w / sf) * 0.5 - (width as f32 * 0.5) * camera_zoom;
        let camera_y = (screen_h / sf) * 0.5 - (height as f32 * 0.5) * camera_zoom;

        Self {
            window: Some(window),
            surface: Some(surface),
            render_ctx,
            map_renderer: Some(map_renderer),
            gui_painter: Some(gui_painter),
            prev_sync_point: None,
            needs_first_upload: true,
            needs_owner_upload: true,

            width,
            height,
            terrain,
            dirty_tiles: Vec::new(),

            editor_ui: sow_ui::ui::map_editor::MapEditorUiState::new(width, height),

            camera_x,
            camera_y,
            camera_zoom,
            dragging: false,
            primary_button_down: false,
            pending_pan: (0.0, 0.0),
            last_mouse_logical_x: 0.0,
            last_mouse_logical_y: 0.0,
            screen_w,
            screen_h,

            egui_ctx,
            raw_input: egui::RawInput::default(),
            client_app,
            last_frame_time: Instant::now(),
            start_time: Instant::now(),

            #[cfg(feature = "osm")]
            osm_picker: OsmPickerState::default(),

            undo_stack: Vec::new(),
            paint_stroke_snapshotted: false,
        }
    }

    pub fn window_id(&self) -> Option<WindowId> {
        self.window.as_ref().map(|w| w.id())
    }

    pub fn window_ref(&self) -> Option<&dyn Window> {
        self.window.as_deref()
    }

    pub fn handle_suspended(&mut self) {
        if let Some(sp) = self.prev_sync_point.take() {
            let _ = self.render_ctx.context.wait_for(&sp, !0);
        }
        if let Some(mut s) = self.surface.take() {
            if let Some(mut gp) = self.gui_painter.take() {
                gp.destroy(&self.render_ctx.context);
            }
            if let Some(mut mr) = self.map_renderer.take() {
                mr.destroy(&self.render_ctx);
            }
            self.render_ctx.reset_command_encoder();
            self.render_ctx.context.destroy_surface(&mut s);
        }
    }

    pub fn handle_resumed(&mut self) {
        self.check_surface();
    }

    pub(crate) fn scale_factor(&self) -> f64 {
        self.window.as_ref().map_or(1.0, |w| w.scale_factor())
    }

    pub(crate) fn logical_screen(&self) -> (f32, f32) {
        let sf = self.scale_factor() as f32;
        (self.screen_w / sf, self.screen_h / sf)
    }

    pub(crate) fn map_editor_viewport(&self) -> sow_ui::ui::map_editor::MapEditorViewport {
        let (lw, lh) = self.logical_screen();
        sow_ui::ui::map_editor::MapEditorViewport {
            camera_x: self.camera_x,
            camera_y: self.camera_y,
            zoom: self.camera_zoom,
            screen_w: lw,
            screen_h: lh,
            pointer_x: self.last_mouse_logical_x,
            pointer_y: self.last_mouse_logical_y,
        }
    }

    pub(crate) fn pointer_on_map_canvas(&self) -> bool {
        let pos = egui::pos2(self.last_mouse_logical_x, self.last_mouse_logical_y);
        self.editor_ui
            .map_canvas_rect
            .is_some_and(|rect| rect.contains(pos))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn maps_root() -> PathBuf {
        std::env::var("SOW_MAPS_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(sow_core::maps::SERVER_MAPS_ROOT))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn refresh_maps_catalog(maps_root: &Path) -> Result<(), String> {
        let mut items = Vec::new();
        let read_dir = std::fs::read_dir(maps_root).map_err(|e| e.to_string())?;
        for entry in read_dir.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let key = entry.file_name().to_string_lossy().to_string();
            if key.starts_with('.') {
                continue;
            }
            let map_path = entry.path().join("map.bin");
            if !map_path.exists() {
                continue;
            }
            let bytes = std::fs::read(&map_path).map_err(|e| e.to_string())?;
            let header = sow_core::map_file::parse_header(&bytes).map_err(|e| e.to_string())?;
            let slug = sow_core::maps::map_key(&key);
            items.push((slug, header));
        }
        let catalog = sow_core::map_file::catalog_from_headers(items);
        let catalog_bytes = sow_core::map_file::encode_catalog(&catalog);
        std::fs::write(maps_root.join("catalog.bin"), catalog_bytes).map_err(|e| e.to_string())
    }

    /// Reload `catalog.bin` from disk into the shared client app (SOLO map list).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn reload_local_map_catalog(
        client_app: &mut sow_ui::ClientApp,
        egui_ctx: &egui::Context,
        select_map_key: Option<&str>,
    ) -> Result<(), String> {
        let maps_root = Self::maps_root();
        let bytes = std::fs::read(maps_root.join("catalog.bin")).map_err(|e| e.to_string())?;
        let catalog = sow_core::map_file::parse_catalog(&bytes).map_err(|e| e.to_string())?;
        let entries = catalog.entries;
        client_app.asset_loader.map_catalog = Some(entries.clone());
        client_app.main_menu_state.apply_map_catalog(&entries);

        if let Some(key) = select_map_key {
            let normalized = sow_core::maps::map_key(key);
            if normalized.is_empty() {
                return Err("Map name produces an empty folder key".into());
            }
            client_app.main_menu_state.single_player_config.map_name = normalized.clone();
            sow_core::maps::apply_catalog_dimensions(
                &entries,
                &mut client_app.main_menu_state.single_player_config.map_name,
                &mut client_app.main_menu_state.single_player_config.map_width,
                &mut client_app.main_menu_state.single_player_config.map_height,
            );
            if let Some(bytes) = sow_core::maps::read_thumbnail_webp_from_repo(&normalized) {
                let _ = client_app
                    .asset_loader
                    .ingest_thumbnail(egui_ctx, &normalized, &bytes);
            } else {
                client_app.asset_loader.request_thumbnail(&normalized);
            }
        }
        Ok(())
    }

    pub fn check_surface(&mut self) {
        if self.surface.is_none() {
            if let Some(win) = self.window.as_ref() {
                let sz = win.surface_size();
                if let Ok(s) =
                    self.render_ctx
                        .create_surface(win, sz.width.max(1), sz.height.max(1))
                {
                    self.screen_w = sz.width as f32;
                    self.screen_h = sz.height as f32;
                    self.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::new(self.screen_w, self.screen_h),
                    ));

                    if let Some(sp) = self.prev_sync_point.take() {
                        let _ = self.render_ctx.context.wait_for(&sp, !0);
                    }

                    if let Some(mut old_mr) = self.map_renderer.take() {
                        old_mr.destroy(&self.render_ctx);
                    }
                    if let Some(mut old_gp) = self.gui_painter.take() {
                        old_gp.destroy(&self.render_ctx.context);
                    }

                    if self.editor_ui.mode == sow_ui::ui::map_editor::EditorMode::Brush {
                        self.map_renderer = Some(MapRenderer::new(
                            &self.render_ctx.context,
                            self.width,
                            self.height,
                            s.info().format,
                            &self.terrain,
                        ));
                        self.needs_first_upload = true;
                        self.needs_owner_upload = true;
                    }
                    self.gui_painter = Some(GuiPainter::new(s.info(), &self.render_ctx.context));
                    self.surface = Some(s);
                    log::info!("Successfully recreated editor surface.");
                }
            }
        }
    }

    pub fn handle_window_event(&mut self, _event_loop: &dyn ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::SurfaceResized(physical_size) => {
                if physical_size.width > 0 && physical_size.height > 0 {
                    if let Some(sp) = self.prev_sync_point.take() {
                        let _ = self.render_ctx.context.wait_for(&sp, !0);
                    }
                    if let Some(ref mut s) = self.surface {
                        self.render_ctx.context.reconfigure_surface(
                            s,
                            gpu::SurfaceConfig {
                                size: gpu::Extent {
                                    width: physical_size.width,
                                    height: physical_size.height,
                                    depth: 1,
                                },
                                usage: gpu::TextureUsage::TARGET,
                                display_sync: gpu::DisplaySync::Tear,
                                color_space: gpu::ColorSpace::Linear,
                                ..gpu::SurfaceConfig::default()
                            },
                        );
                    }
                    self.screen_w = physical_size.width as f32;
                    self.screen_h = physical_size.height as f32;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if pressed {
                    if let winit::keyboard::Key::Character(text) = &event.logical_key {
                        self.raw_input
                            .events
                            .push(egui::Event::Text(text.to_string()));
                    } else if let winit::keyboard::Key::Named(named) = &event.logical_key {
                        if *named == winit::keyboard::NamedKey::Backspace {
                            self.raw_input.events.push(egui::Event::Key {
                                key: egui::Key::Backspace,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: self.raw_input.modifiers,
                            });
                        }
                    }
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.raw_input.modifiers.alt = modifiers.state().alt_key();
                self.raw_input.modifiers.ctrl = modifiers.state().control_key();
                self.raw_input.modifiers.shift = modifiers.state().shift_key();
                self.raw_input.modifiers.mac_cmd = modifiers.state().meta_key();
                self.raw_input.modifiers.command =
                    self.raw_input.modifiers.ctrl || self.raw_input.modifiers.mac_cmd;
            }
            WindowEvent::PointerMoved {
                position, primary, ..
            } => {
                if primary {
                    let sf = self.scale_factor();
                    let logical_x = (position.x / sf) as f32;
                    let logical_y = (position.y / sf) as f32;
                    let dx = logical_x - self.last_mouse_logical_x;
                    let dy = logical_y - self.last_mouse_logical_y;
                    self.last_mouse_logical_x = logical_x;
                    self.last_mouse_logical_y = logical_y;
                    if self.dragging {
                        self.pending_pan.0 += dx;
                        self.pending_pan.1 += dy;
                    }
                    self.raw_input
                        .events
                        .push(egui::Event::PointerMoved(egui::Pos2::new(
                            logical_x, logical_y,
                        )));
                }
            }
            WindowEvent::PointerButton {
                state,
                button,
                position,
                primary,
                ..
            } => {
                let pressed = state == ElementState::Pressed;
                let sf = self.scale_factor();
                let logical_x = (position.x / sf) as f32;
                let logical_y = (position.y / sf) as f32;
                if primary {
                    self.last_mouse_logical_x = logical_x;
                    self.last_mouse_logical_y = logical_y;
                }

                let is_left = match button {
                    winit::event::ButtonSource::Mouse(b) => b == winit::event::MouseButton::Left,
                    _ => primary,
                };
                let is_right = match button {
                    winit::event::ButtonSource::Mouse(b) => b == winit::event::MouseButton::Right,
                    _ => false,
                };

                if is_right {
                    self.dragging = pressed;
                } else if is_left {
                    self.primary_button_down = pressed;
                    if !pressed {
                        self.paint_stroke_snapshotted = false;
                    }
                }

                if primary {
                    self.raw_input.events.push(egui::Event::PointerButton {
                        pos: egui::Pos2::new(logical_x, logical_y),
                        button: match button {
                            winit::event::ButtonSource::Mouse(winit::event::MouseButton::Right) => {
                                egui::PointerButton::Secondary
                            }
                            winit::event::ButtonSource::Mouse(
                                winit::event::MouseButton::Middle,
                            ) => egui::PointerButton::Middle,
                            _ => egui::PointerButton::Primary,
                        },
                        pressed,
                        modifiers: self.raw_input.modifiers,
                    });
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y * 30.0,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        pos.y as f32 / self.scale_factor() as f32
                    }
                };

                if self.pointer_on_map_canvas() {
                    #[cfg(feature = "osm")]
                    if self.editor_ui.mode == sow_ui::ui::map_editor::EditorMode::OsmPicker {
                        self.zoom_osm(scroll);
                    } else {
                        let zoom_speed = 0.002f32;
                        let old_zoom = self.camera_zoom;
                        self.camera_zoom =
                            (self.camera_zoom * (1.0 + scroll * zoom_speed)).clamp(0.2, 10.0);
                        let mx = self.last_mouse_logical_x;
                        let my = self.last_mouse_logical_y;
                        self.camera_x = mx - (mx - self.camera_x) * (self.camera_zoom / old_zoom);
                        self.camera_y = my - (my - self.camera_y) * (self.camera_zoom / old_zoom);
                    }
                } else if self.egui_ctx.egui_wants_pointer_input() {
                    let sf = self.scale_factor() as f32;
                    let (unit, vec_delta) = match delta {
                        winit::event::MouseScrollDelta::LineDelta(x, y) => {
                            (egui::MouseWheelUnit::Line, egui::vec2(x, y))
                        }
                        winit::event::MouseScrollDelta::PixelDelta(pos) => (
                            egui::MouseWheelUnit::Point,
                            egui::vec2(pos.x as f32 / sf, pos.y as f32 / sf),
                        ),
                    };
                    self.raw_input.events.push(egui::Event::MouseWheel {
                        unit,
                        delta: vec_delta,
                        phase: egui::TouchPhase::Move,
                        modifiers: self.raw_input.modifiers,
                    });
                } else {
                    let zoom_speed = 0.002f32;
                    let old_zoom = self.camera_zoom;
                    self.camera_zoom =
                        (self.camera_zoom * (1.0 + scroll * zoom_speed)).clamp(0.2, 10.0);
                    let mx = self.last_mouse_logical_x;
                    let my = self.last_mouse_logical_y;
                    self.camera_x = mx - (mx - self.camera_x) * (self.camera_zoom / old_zoom);
                    self.camera_y = my - (my - self.camera_y) * (self.camera_zoom / old_zoom);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn new_blank_map(&mut self, w: u32, h: u32) {
        self.width = w - (w % 2);
        self.height = h - (h % 2);
        let size = (self.width * self.height) as usize;
        self.terrain = vec![0b00100000; size]; // All ocean tiles
        self.editor_ui.spawns.clear();
        self.editor_ui.is_dirty = false;
        self.undo_stack.clear();
        self.editor_ui.width = self.width;
        self.editor_ui.height = self.height;
        self.dirty_tiles.clear();
        self.needs_first_upload = true;
        self.needs_owner_upload = true;

        if let Some(ref mut mr) = self.map_renderer {
            mr.destroy(&self.render_ctx);
        }

        if let Some(ref s) = self.surface {
            self.map_renderer = Some(MapRenderer::new(
                &self.render_ctx.context,
                self.width,
                self.height,
                s.info().format,
                &self.terrain,
            ));
        }

        self.camera_zoom = 1.0;
        let (lw, lh) = self.logical_screen();
        self.camera_x = lw * 0.5 - (self.width as f32 * 0.5) * self.camera_zoom;
        self.camera_y = lh * 0.5 - (self.height as f32 * 0.5) * self.camera_zoom;
        let msg = sow_i18n::get(self.client_app.settings_state.language)
            .map_editor
            .msg_blank_created
            .clone();
        self.editor_ui.show_toast(msg, false);
    }

    pub(crate) fn notify_error(&mut self, text: impl Into<String>) {
        self.editor_ui.show_toast(text, true);
    }

    pub(crate) fn notify_info(&mut self, text: impl Into<String>) {
        self.editor_ui.show_toast(text, false);
    }
}
