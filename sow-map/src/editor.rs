use blade_egui::GuiPainter;
use blade_graphics as gpu;
use egui::Context;
use sow_render::{MapGlobals, MapRenderer, RenderContext};
use sow_ui::ClientApp;
use std::io::Write;
use std::path::{Path, PathBuf};
use web_time::Instant;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

pub struct MapEditorSession {
    // Reclaimable graphics state
    pub window: Option<Box<dyn Window>>,
    pub surface: Option<gpu::Surface>,
    pub render_ctx: RenderContext,
    pub map_renderer: Option<MapRenderer>,
    pub gui_painter: Option<GuiPainter>,
    pub prev_sync_point: Option<gpu::SyncPoint>,
    pub needs_first_upload: bool,

    // Session states
    pub width: u32,
    pub height: u32,
    pub terrain: Vec<u8>, // Holds the raw packed bytes representing each tile
    pub dirty_tiles: Vec<usize>,

    // UI state (chrome lives in sow-ui)
    pub editor_ui: sow_ui::ui::map_editor::MapEditorUiState,

    // Navigation state
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub dragging: bool,
    pub primary_button_down: bool,
    pub pending_pan: (f32, f32),
    pub last_mouse_logical_x: f32,
    pub last_mouse_logical_y: f32,
    pub screen_w: f32,
    pub screen_h: f32,

    pub egui_ctx: Context,
    pub raw_input: egui::RawInput,
    pub client_app: ClientApp,
    pub last_frame_time: Instant,
    pub start_time: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PaintType {
    Water,
    Ocean,
    Shoreline,
    Plains,
    Highlands,
    Mountains,
}

fn paint_type_from_kind(kind: sow_ui::ui::map_editor::EditorPaintKind) -> PaintType {
    match kind {
        sow_ui::ui::map_editor::EditorPaintKind::Water => PaintType::Water,
        sow_ui::ui::map_editor::EditorPaintKind::Ocean => PaintType::Ocean,
        sow_ui::ui::map_editor::EditorPaintKind::Shoreline => PaintType::Shoreline,
        sow_ui::ui::map_editor::EditorPaintKind::Plains => PaintType::Plains,
        sow_ui::ui::map_editor::EditorPaintKind::Highlands => PaintType::Highlands,
        sow_ui::ui::map_editor::EditorPaintKind::Mountains => PaintType::Mountains,
    }
}

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
            self.render_ctx.context.destroy_surface(&mut s);
        }
    }

    pub fn handle_resumed(&mut self) {
        self.check_surface();
    }

    fn scale_factor(&self) -> f64 {
        self.window.as_ref().map_or(1.0, |w| w.scale_factor())
    }

    fn logical_screen(&self) -> (f32, f32) {
        let sf = self.scale_factor() as f32;
        (self.screen_w / sf, self.screen_h / sf)
    }

    fn map_editor_viewport(&self) -> sow_ui::ui::map_editor::MapEditorViewport {
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

    fn pointer_over_ui(&self) -> bool {
        self.egui_ctx.egui_wants_pointer_input()
    }

    fn maps_root() -> PathBuf {
        std::env::var("SOW_MAPS_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("assets/maps"))
    }

    fn refresh_maps_catalog(maps_root: &Path) -> Result<(), String> {
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

                    self.map_renderer = Some(MapRenderer::new(
                        &self.render_ctx.context,
                        self.width,
                        self.height,
                        s.info().format,
                        &self.terrain,
                    ));
                    self.needs_first_upload = true;
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
                                color_space: gpu::ColorSpace::Srgb,
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
                    self.raw_input.events.push(egui::Event::PointerMoved(egui::Pos2::new(
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
                if self.pointer_over_ui() {
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
                    let sf = self.scale_factor() as f32;
                    let scroll = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * 30.0,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / sf,
                    };
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

    fn paint_at_cursor(&mut self) {
        let mx = self.last_mouse_logical_x;
        let my = self.last_mouse_logical_y;

        let world_x = (mx - self.camera_x) / self.camera_zoom;
        let world_y = (my - self.camera_y) / self.camera_zoom;

        let cx = world_x.round() as i32;
        let cy = world_y.round() as i32;
        let r = self.editor_ui.brush_size;

        for dx in -r..=r {
            for dy in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    let tx = cx + dx;
                    let ty = cy + dy;

                    if tx >= 0 && tx < self.width as i32 && ty >= 0 && ty < self.height as i32 {
                        let idx = (ty * self.width as i32 + tx) as usize;
                        let mut byte = 0u8;

                        match paint_type_from_kind(self.editor_ui.selected_paint) {
                            PaintType::Water => {
                                byte |= 0b00000000; // Water, not land, not shore, not ocean
                                byte |= (self.editor_ui.brush_strength as u8).min(31);
                            }
                            PaintType::Ocean => {
                                byte |= 0b00100000; // Ocean
                                byte |= (self.editor_ui.brush_strength as u8).min(31);
                            }
                            PaintType::Shoreline => {
                                byte |= 0b01000000; // Shoreline
                            }
                            PaintType::Plains => {
                                byte |= 0b10000000; // Land
                                byte |= (self.editor_ui.brush_strength.min(9.0) as u8) & 0b00011111;
                            }
                            PaintType::Highlands => {
                                byte |= 0b10000000; // Land
                                byte |= (self.editor_ui.brush_strength.clamp(10.0, 19.0) as u8)
                                    & 0b00011111;
                            }
                            PaintType::Mountains => {
                                byte |= 0b10000000; // Land
                                byte |= (self.editor_ui.brush_strength.clamp(20.0, 31.0) as u8)
                                    & 0b00011111;
                            }
                        }

                        self.terrain[idx] = byte;
                        self.dirty_tiles.push(idx);
                    }
                }
            }
        }
    }

    fn new_blank_map(&mut self, w: u32, h: u32) {
        self.width = w - (w % 2);
        self.height = h - (h % 2);
        let size = (self.width * self.height) as usize;
        self.terrain = vec![0b00100000; size]; // All ocean tiles
        self.editor_ui.spawns.clear();
        self.editor_ui.width = self.width;
        self.editor_ui.height = self.height;
        self.dirty_tiles.clear();
        self.needs_first_upload = true;

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
        let msg = sow_lang::get(self.client_app.settings_state.language)
            .map_editor
            .msg_blank_created
            .clone();
        self.editor_ui
            .show_toast(msg, false);
    }

    fn notify_error(&mut self, text: impl Into<String>) {
        self.editor_ui.show_toast(text, true);
    }

    fn notify_info(&mut self, text: impl Into<String>) {
        self.editor_ui.show_toast(text, false);
    }

    fn export_map_package(&mut self) {
        log::info!("Starting native Rust map compilation...");
        let lang = self.client_app.settings_state.language;
        let strings = &sow_lang::get(lang).map_editor;
        self.notify_info(&strings.msg_compiling);

        // Translate current map layout canvas bytes back to RGBA pixels for our high-fidelity generator
        let mut pixels = vec![[0u8; 4]; (self.width * self.height) as usize];
        for (i, &byte) in self.terrain.iter().enumerate() {
            let is_land = (byte & 0b10000000) != 0;
            let mag = byte & 0b00011111;

            let mut blue = 106u8; // default water blue
            if is_land {
                blue = (mag as u16 + 140).min(200) as u8;
            }

            pixels[i] = [0, 0, blue, 255];
        }

        let args = crate::generator::GeneratorArgs {
            width: self.width,
            height: self.height,
            pixels,
            remove_small: true,
        };

        match crate::generator::generate_map(args) {
            Ok(result) => {
                let maps_root = Self::maps_root();
                let out_dir = maps_root.join(&self.editor_ui.map_name);
                if let Err(e) = std::fs::create_dir_all(&out_dir) {
                    self.notify_error(format!("Failed to create path: {}", e));
                    return;
                }

                let spawns: Vec<sow_core::map_file::MapSpawn> = self
                    .editor_ui
                    .spawns
                    .iter()
                    .map(|s| sow_core::map_file::MapSpawn {
                        name: s.name.clone(),
                        flag: s.flag.clone(),
                        x: s.x,
                        y: s.y,
                    })
                    .collect();
                let map_file = sow_core::map_file::MapFile {
                    display_name: self.editor_ui.map_name.clone(),
                    width: result.width,
                    height: result.height,
                    num_land_tiles: result.num_land_tiles,
                    spawns,
                    terrain: result.map_data,
                };
                let map_bytes = sow_core::map_file::encode(&map_file);

                let mut brotli_out = Vec::new();
                let brotli_ok = (|| {
                    let mut writer = brotli::CompressorWriter::new(&mut brotli_out, 4096, 11, 22);
                    writer.write_all(&map_bytes)?;
                    writer.flush()?;
                    Ok::<(), std::io::Error>(())
                })()
                .is_ok();

                let wrote_map = std::fs::write(out_dir.join("map.bin"), &map_bytes).is_ok();
                let wrote_br = brotli_ok
                    && std::fs::write(out_dir.join("map.bin.br"), &brotli_out).is_ok();
                let wrote_thumb =
                    std::fs::write(out_dir.join("thumbnail.webp"), &result.thumbnail_data).is_ok();

                if wrote_map && wrote_br && wrote_thumb {
                    match Self::refresh_maps_catalog(&maps_root) {
                        Ok(()) => self.notify_info(&strings.msg_saved),
                        Err(e) => {
                            self.notify_info(format!("{} (catalog: {})", strings.msg_saved, e));
                        }
                    }
                } else {
                    self.notify_error(&strings.msg_write_failed);
                }
            }
            Err(e) => {
                self.notify_error(format!("Compilation error: {}", e));
            }
        }
    }

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
            sow_core::register_game_assets(&self.egui_ctx);
        });

        self.editor_ui.width = self.width;
        self.editor_ui.height = self.height;

        let mut ui_action = sow_ui::ui::map_editor::MapEditorAction::None;
        let viewport = self.map_editor_viewport();
        let egui_ctx = self.egui_ctx.clone();
        let egui_output = egui_ctx.run_ui(self.raw_input.clone(), |ui| {
            ui_action = sow_ui::ui::map_editor::draw_map_editor(
                ui,
                &egui_ctx,
                &mut self.editor_ui,
                viewport,
                lang,
            );
        });

        if self.dragging {
            self.camera_x += self.pending_pan.0;
            self.camera_y += self.pending_pan.1;
            self.pending_pan = (0.0, 0.0);
        }

        if self.primary_button_down && !self.pointer_over_ui() {
            self.paint_at_cursor();
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
                self.editor_ui.spawns.push(sow_ui::ui::map_editor::SpawnRowUi {
                    x: cx,
                    y: cy,
                    name: format!("Nation {}", idx),
                    flag: "🏳".to_string(),
                });
                self.notify_info(&sow_lang::get(lang).map_editor.msg_spawn_placed);
            }
            sow_ui::ui::map_editor::MapEditorAction::RemoveSpawn(idx) => {
                self.editor_ui.spawns.remove(idx);
                self.notify_info(&sow_lang::get(lang).map_editor.msg_spawn_removed);
            }
            sow_ui::ui::map_editor::MapEditorAction::None => {}
        }

        self.raw_input.events.clear();

        let (logical_w, logical_h) = self.logical_screen();
        let hover_hex = if !self.pointer_over_ui() {
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

                // Upload map updates to GPU
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

                    // Push dirty tile indexes to MapRenderer GPU buffer
                    if !self.dirty_tiles.is_empty() {
                        // Sync programmatic changes to MapRenderer raw backing buffer
                        for &idx in &self.dirty_tiles {
                            if idx < self.terrain.len() {
                                mr.terrain[idx] = self.terrain[idx];
                            }
                        }
                        let dirty_dt: Vec<sow_core::protocol::DirtyTile> = self
                            .dirty_tiles
                            .iter()
                            .map(|&idx| sow_core::protocol::DirtyTile {
                                index: idx as u32,
                                new_owner: 0,
                                upgrade_level: 0,
                            })
                            .collect();
                        mr.update(
                            &mut self.render_ctx.command_encoder,
                            &self.render_ctx.context,
                            &dirty_dt,
                        );
                        self.dirty_tiles.clear();
                    }

                    // Render Map viewport
                    let mut player_colors = [[0.5, 0.5, 0.5, 1.0]; 256];
                    player_colors[1] = [0.1, 0.6, 0.9, 1.0];

                    let globals = MapGlobals {
                        camera_pos: [self.camera_x, self.camera_y],
                        zoom: self.camera_zoom,
                        time: self.start_time.elapsed().as_secs_f32(),
                        screen_size: [logical_w, logical_h],
                        map_size: [self.width as f32, self.height as f32],
                        border_thickness: 1.0,
                        border_darkness: 0.0,
                        shore_thickness: 1.0,
                        shore_darkness: 1.0,
                        threat_slots: [[0.0; 4]; 8],
                        effect_shockwave: 0.0,
                        effect_breathe: 0.0,
                        effect_energy_flow: 0.0,
                        my_player_id: 0.0,
                        hover_hex,
                        hover_building_kind: 0.0,
                        _pad1: 0.0,
                        fallout_slots: [[0.0; 4]; 8],
                        nobuild_slots: [[0.0; 4]; 32],
                    };
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

    #[allow(clippy::type_complexity)]
    pub fn destroy_and_reclaim(
        mut self,
    ) -> (
        Option<Box<dyn Window>>,
        Option<gpu::Surface>,
        RenderContext,
        Option<GuiPainter>,
        ClientApp,
    ) {
        if let Some(sp) = self.prev_sync_point.take() {
            let _ = self.render_ctx.context.wait_for(&sp, !0);
        }
        if let Some(mut mr) = self.map_renderer.take() {
            mr.destroy(&self.render_ctx);
        }
        let gui_painter = self.gui_painter.take();
        let window = self.window.take();
        let surface = self.surface.take();

        // Safety: read fields directly bypassing move restrictions, matching gameplay session reclamation!
        let render_ctx = unsafe { std::ptr::read(&self.render_ctx) };
        let client_app = unsafe { std::ptr::read(&self.client_app) };

        unsafe {
            std::ptr::drop_in_place(&mut self.terrain);
            std::ptr::drop_in_place(&mut self.dirty_tiles);
            std::ptr::drop_in_place(&mut self.egui_ctx);
            std::ptr::drop_in_place(&mut self.raw_input);
            std::ptr::drop_in_place(&mut self.editor_ui);
            std::mem::forget(self);
        }

        (window, surface, render_ctx, gui_painter, client_app)
    }
}

impl Drop for MapEditorSession {
    fn drop(&mut self) {
        if let Some(sp) = self.prev_sync_point.take() {
            let _ = self.render_ctx.context.wait_for(&sp, !0);
        }
        if let Some(mut mr) = self.map_renderer.take() {
            mr.destroy(&self.render_ctx);
        }
        if let Some(mut gp) = self.gui_painter.take() {
            gp.destroy(&self.render_ctx.context);
        }
        if let Some(mut s) = self.surface.take() {
            self.render_ctx.context.destroy_surface(&mut s);
        }
    }
}
