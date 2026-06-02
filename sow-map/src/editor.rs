use blade_egui::GuiPainter;
use blade_graphics as gpu;
use egui::Context;
use sow_render::{MapGlobals, MapRenderer, RenderContext};
use sow_ui::ClientApp;
#[cfg(feature = "osm")]
use std::collections::HashMap;
use std::io::Write;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
use web_time::Instant;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

#[cfg(feature = "osm")]
use crate::image_pipeline::generate_from_rgba;
#[cfg(feature = "osm")]
use crate::heightmap::{terrain_stats_from_packed, WorldHeightmap};
#[cfg(feature = "osm")]
use crate::osm_tiles::{
    classify_osm_to_rgba_with_heightmap, fetch_region_blocking, lonlat_to_world_px,
    pick_fetch_zoom, tiles_covering_rect, world_px_to_lonlat, CachedTile, OsmTileCache, TileKey,
    MAX_TILE_ZOOM, TILE_SIZE,
};

#[cfg(feature = "osm")]
struct OsmPickerState {
    center_lon: f64,
    center_lat: f64,
    zoom: u32,
    sel_anchor_world: Option<(f64, f64)>,
    sel_corner_world: Option<(f64, f64)>,
    cache: OsmTileCache,
    textures: HashMap<TileKey, egui::TextureHandle>,
}

#[cfg(feature = "osm")]
impl Default for OsmPickerState {
    fn default() -> Self {
        Self {
            center_lon: -95.0,
            center_lat: 40.0,
            zoom: 6,
            sel_anchor_world: None,
            sel_corner_world: None,
            cache: OsmTileCache::default(),
            textures: HashMap::new(),
        }
    }
}

struct MapExportArtifacts {
    slug: String,
    map_bytes: Vec<u8>,
    brotli_bytes: Vec<u8>,
    thumb_webp: Vec<u8>,
}

pub struct MapEditorSession {
    // Reclaimable graphics state
    pub window: Option<Box<dyn Window>>,
    pub surface: Option<gpu::Surface>,
    pub render_ctx: RenderContext,
    pub map_renderer: Option<MapRenderer>,
    pub gui_painter: Option<GuiPainter>,
    pub prev_sync_point: Option<gpu::SyncPoint>,
    pub needs_first_upload: bool,
    pub needs_owner_upload: bool,

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

    #[cfg(feature = "osm")]
    osm_picker: OsmPickerState,

    undo_stack: Vec<Vec<u8>>,
    paint_stroke_snapshotted: bool,
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

    fn pointer_on_map_canvas(&self) -> bool {
        let pos = egui::pos2(self.last_mouse_logical_x, self.last_mouse_logical_y);
        self.editor_ui
            .map_canvas_rect
            .is_some_and(|rect| rect.contains(pos))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn maps_root() -> PathBuf {
        std::env::var("SOW_MAPS_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("assets/maps"))
    }

    #[cfg(not(target_arch = "wasm32"))]
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

    /// Reload `catalog.bin` from disk into the shared client app (Single Player map list).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn reload_local_map_catalog(
        client_app: &mut sow_ui::ClientApp,
        egui_ctx: &egui::Context,
        select_map_key: Option<&str>,
    ) -> Result<(), String> {
        let maps_root = Self::maps_root();
        let bytes =
            std::fs::read(maps_root.join("catalog.bin")).map_err(|e| e.to_string())?;
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
                client_app
                    .asset_loader
                    .request_thumbnail(&normalized);
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

    fn push_undo_snapshot(&mut self) {
        const MAX_UNDO: usize = 20;
        self.undo_stack.push(self.terrain.clone());
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.remove(0);
        }
    }

    fn undo_last_stroke(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.terrain = prev;
            self.dirty_tiles.clear();
            if let Some(ref mut mr) = self.map_renderer {
                mr.terrain.clone_from(&self.terrain);
            }
            self.dirty_tiles.extend(0..self.terrain.len());
            self.editor_ui.is_dirty = !self.undo_stack.is_empty();
        }
    }

    fn mark_dirty(&mut self) {
        self.editor_ui.is_dirty = true;
    }

    fn paint_at_cursor(&mut self) {
        if !self.paint_stroke_snapshotted {
            self.push_undo_snapshot();
            self.paint_stroke_snapshotted = true;
        }
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
        self.mark_dirty();
    }

    fn new_blank_map(&mut self, w: u32, h: u32) {
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

    fn gameplay_map_globals(&self, logical_w: f32, logical_h: f32, hover_hex: [f32; 2]) -> MapGlobals {
        MapGlobals {
            camera_pos: [self.camera_x, self.camera_y],
            zoom: self.camera_zoom,
            time: self.start_time.elapsed().as_secs_f32() % 1000.0,
            screen_size: [logical_w, logical_h],
            map_size: [self.width as f32, self.height as f32],
            border_thickness: 1.0,
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
            _pad1: 0.0,
            fallout_slots: [[0.0; 4]; 8],
            nobuild_slots: [[0.0; 4]; 32],
        }
    }

    #[cfg(feature = "osm")]
    fn osm_center_world_px(&self) -> (f64, f64) {
        lonlat_to_world_px(
            self.osm_picker.center_lon,
            self.osm_picker.center_lat,
            self.osm_picker.zoom,
        )
    }

    #[cfg(feature = "osm")]
    fn screen_to_world_px(&self, sx: f32, sy: f32) -> (f64, f64) {
        let Some(rect) = self.editor_ui.map_canvas_rect else {
            return self.osm_center_world_px();
        };
        let sx = sx.clamp(rect.min.x, rect.max.x);
        let sy = sy.clamp(rect.min.y, rect.max.y);
        let (cx, cy) = self.osm_center_world_px();
        let dx = (sx - rect.center().x) as f64;
        let dy = (sy - rect.center().y) as f64;
        (cx + dx, cy + dy)
    }

    #[cfg(feature = "osm")]
    fn world_px_to_screen(&self, wx: f64, wy: f64) -> egui::Pos2 {
        let (cx, cy) = self.osm_center_world_px();
        let rect = self
            .editor_ui
            .map_canvas_rect
            .unwrap_or_else(|| egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ONE));
        egui::pos2(
            rect.center().x + (wx - cx) as f32,
            rect.center().y + (wy - cy) as f32,
        )
    }

    #[cfg(feature = "osm")]
    fn selection_world_square(&self) -> Option<(f64, f64, f64)> {
        let (ax, ay) = self.osm_picker.sel_anchor_world?;
        let (cx, cy) = self.osm_picker.sel_corner_world?;
        let dx = cx - ax;
        let dy = cy - ay;
        let size = dx.abs().max(dy.abs());
        if size < 8.0 {
            return None;
        }
        let x0 = if dx >= 0.0 { ax } else { ax - size };
        let y0 = if dy >= 0.0 { ay } else { ay - size };
        Some((x0, y0, size))
    }

    #[cfg(feature = "osm")]
    fn update_osm_tiles(&mut self) {
        let Some(rect) = self.editor_ui.map_canvas_rect else {
            return;
        };
        self.osm_picker.cache.drain_messages();

        let (cx, cy) = self.osm_center_world_px();
        let z = self.osm_picker.zoom;
        let half_w = (rect.width() * 0.5) as f64 + TILE_SIZE as f64;
        let half_h = (rect.height() * 0.5) as f64 + TILE_SIZE as f64;
        let keys = tiles_covering_rect(cx - half_w, cy - half_h, cx + half_w, cy + half_h, z);
        for key in &keys {
            self.osm_picker.cache.request(*key);
        }

        for key in keys {
            if let Some(CachedTile::Ready(img)) = self.osm_picker.cache.get(key).cloned() {
                if !self.osm_picker.textures.contains_key(&key) {
                    let name = format!("osm_{}_{}_{}", key.z, key.x, key.y);
                    let size = [img.width() as usize, img.height() as usize];
                    let color_image =
                        egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                    let handle = self.egui_ctx.load_texture(
                        name,
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.osm_picker.textures.insert(key, handle);
                }
            }
        }
    }

    #[cfg(feature = "osm")]
    fn build_osm_view(&self) -> sow_ui::ui::map_editor::OsmPickerView {
        let mut view = sow_ui::ui::map_editor::OsmPickerView {
            center_lon: self.osm_picker.center_lon,
            center_lat: self.osm_picker.center_lat,
            zoom: self.osm_picker.zoom,
            ..Default::default()
        };

        let Some(rect) = self.editor_ui.map_canvas_rect else {
            return view;
        };

        let (cx, cy) = self.osm_center_world_px();
        let z = self.osm_picker.zoom;
        let half_w = rect.width() as f64 * 0.5 + TILE_SIZE as f64;
        let half_h = rect.height() as f64 * 0.5 + TILE_SIZE as f64;
        for key in tiles_covering_rect(cx - half_w, cy - half_h, cx + half_w, cy + half_h, z) {
            let Some(handle) = self.osm_picker.textures.get(&key) else {
                continue;
            };
            let wx = key.x as f64 * TILE_SIZE as f64;
            let wy = key.y as f64 * TILE_SIZE as f64;
            let min = self.world_px_to_screen(wx, wy);
            let max = self.world_px_to_screen(wx + TILE_SIZE as f64, wy + TILE_SIZE as f64);
            view.tiles.push(sow_ui::ui::map_editor::OsmPickerTileDraw {
                rect: egui::Rect::from_min_max(min, max),
                texture: handle.id(),
            });
        }

        if let Some(sel) = self.editor_ui.osm_selection_screen {
            view.selection_screen_rect = Some(sel);
        } else if let Some((x0, y0, size)) = self.selection_world_square() {
            let min = self.world_px_to_screen(x0, y0);
            let max = self.world_px_to_screen(x0 + size, y0 + size);
            view.selection_screen_rect = Some(egui::Rect::from_min_max(min, max));
        }

        if let Some((x0, y0, size)) = self.selection_world_square() {
            let (lon0, lat0) = world_px_to_lonlat(x0, y0, z);
            let (lon1, lat1) = world_px_to_lonlat(x0 + size, y0 + size, z);
            let min_lon = lon0.min(lon1);
            let max_lon = lon0.max(lon1);
            let min_lat = lat0.min(lat1);
            let max_lat = lat0.max(lat1);
            view.selection_bbox = Some((min_lon, min_lat, max_lon, max_lat));
            let deg_span = (max_lon - min_lon).abs().max((max_lat - min_lat).abs());
            let fetch_z = pick_fetch_zoom(self.editor_ui.osm.target_size, deg_span);
            let (wx0, wy0) = lonlat_to_world_px(min_lon, max_lat, fetch_z);
            let (wx1, wy1) = lonlat_to_world_px(max_lon, min_lat, fetch_z);
            let keys = tiles_covering_rect(wx0, wy0, wx1, wy1, fetch_z);
            view.overpass_tile_estimate = Some(keys.len());
        }

        view
    }

    #[cfg(feature = "osm")]
    fn pan_osm(&mut self, dx: f32, dy: f32) {
        let z = self.osm_picker.zoom;
        let (cx, cy) = self.osm_center_world_px();
        let (lon, lat) = world_px_to_lonlat(cx - dx as f64, cy - dy as f64, z);
        self.osm_picker.center_lon = lon.clamp(-180.0, 180.0);
        self.osm_picker.center_lat = lat.clamp(-85.0, 85.0);
    }

    #[cfg(feature = "osm")]
    fn zoom_osm(&mut self, delta: f32) {
        let old = self.osm_picker.zoom;
        if delta > 0.0 {
            self.osm_picker.zoom = (self.osm_picker.zoom + 1).min(MAX_TILE_ZOOM);
        } else if delta < 0.0 {
            self.osm_picker.zoom = self.osm_picker.zoom.saturating_sub(1).max(2);
        }
        if self.osm_picker.zoom != old {
            self.osm_picker.textures.clear();
        }
    }

    #[cfg(feature = "osm")]
    fn release_brush_renderer(&mut self) {
        if let Some(sp) = self.prev_sync_point.take() {
            let _ = self.render_ctx.context.wait_for(&sp, !0);
        }
        if let Some(mut mr) = self.map_renderer.take() {
            mr.destroy(&self.render_ctx);
        }
        self.render_ctx.reset_command_encoder();
        self.needs_first_upload = true;
        self.needs_owner_upload = true;
    }

    fn ensure_brush_renderer(&mut self) {
        if self.map_renderer.is_some() {
            return;
        }
        if let Some(ref s) = self.surface {
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
    }

    #[cfg(feature = "osm")]
    fn enter_osm_view(&mut self) {
        self.release_brush_renderer();
        self.osm_picker = OsmPickerState::default();
        self.editor_ui.osm_drag_anchor = None;
        self.editor_ui.osm_selection_screen = None;
    }

    #[cfg(feature = "osm")]
    fn apply_osm_selection_from_screen(&mut self) {
        let Some(sel) = self.editor_ui.osm_selection_screen else {
            return;
        };
        let min = sel.min;
        let max = sel.max;
        let (wx0, wy0) = self.screen_to_world_px(min.x, min.y);
        let (wx1, wy1) = self.screen_to_world_px(max.x, max.y);
        self.osm_picker.sel_anchor_world = Some((wx0.min(wx1), wy0.min(wy1)));
        self.osm_picker.sel_corner_world = Some((wx0.max(wx1), wy0.max(wy1)));
    }

    #[cfg(feature = "osm")]
    fn refresh_map_renderer_terrain(&mut self) {
        self.dirty_tiles.clear();
        if let Some(mut mr) = self.map_renderer.take() {
            mr.destroy(&self.render_ctx);
        }
        self.ensure_brush_renderer();
    }

    #[cfg(feature = "osm")]
    fn generate_from_osm(&mut self) {
        let lang = self.client_app.settings_state.language;
        let strings = &sow_lang::get(lang).map_editor;

        let Some((x0, y0, size)) = self.selection_world_square() else {
            self.notify_error(&strings.msg_osm_no_selection);
            return;
        };

        self.editor_ui.osm.generating = true;
        self.editor_ui.busy_message = Some(strings.msg_osm_generating.clone());
        self.notify_info(&strings.msg_osm_generating);
        self.egui_ctx.request_repaint();

        let z = self.osm_picker.zoom;
        let (lon0, lat0) = world_px_to_lonlat(x0, y0, z);
        let (lon1, lat1) = world_px_to_lonlat(x0 + size, y0 + size, z);
        let deg_span = (lon1 - lon0).abs().max((lat1 - lat0).abs());
        let target = self.editor_ui.osm.target_size;
        let fetch_z = pick_fetch_zoom(target, deg_span);
        let (wx0, wy0) = lonlat_to_world_px(lon0.min(lon1), lat0.max(lat1), fetch_z);
        let (wx1, wy1) = lonlat_to_world_px(lon0.max(lon1), lat0.min(lat1), fetch_z);
        let world_size = (wx1 - wx0).max(wy1 - wy0);

        let stitched = match fetch_region_blocking(
            &mut self.osm_picker.cache,
            fetch_z,
            wx0,
            wy0,
            world_size,
        ) {
            Ok(img) => img,
            Err(e) => {
                self.editor_ui.clear_busy();
                self.notify_error(strings.msg_osm_failed.replace("{}", &e));
                return;
            }
        };
        self.egui_ctx.request_repaint();

        self.editor_ui.busy_message = Some(strings.msg_osm_classifying.clone());
        self.egui_ctx.request_repaint();

        let heightmap = match WorldHeightmap::load() {
            Ok(hm) => hm,
            Err(e) => {
                self.editor_ui.clear_busy();
                self.notify_error(strings.msg_osm_failed.replace("{}", &e));
                return;
            }
        };

        let min_lon = lon0.min(lon1);
        let max_lon = lon0.max(lon1);
        let min_lat = lat0.min(lat1);
        let max_lat = lat0.max(lat1);

        let dst = target - (target % 4);
        let encoded = classify_osm_to_rgba_with_heightmap(
            &stitched,
            min_lon,
            min_lat,
            max_lon,
            max_lat,
            &heightmap,
        );
        let water_px = encoded.pixels().filter(|p| p.0[2] == 106).count();
        let elevated_land = encoded
            .pixels()
            .filter(|p| p.0[2] > 140 && p.0[2] <= 200)
            .count();
        log::info!(
            "OSM classify: {}x{} — {} water / {} land pixels ({} with elevation > plains)",
            encoded.width(),
            encoded.height(),
            water_px,
            encoded.pixels().len() - water_px,
            elevated_land
        );

        match generate_from_rgba(&encoded, Some((dst, dst))) {
            Ok(result) => {
                terrain_stats_from_packed(&result.map_data).log_summary();

                self.width = result.width;
                self.height = result.height;
                self.terrain = result.map_data;
                self.editor_ui.width = self.width;
                self.editor_ui.height = self.height;
                self.refresh_map_renderer_terrain();

                self.camera_zoom = 1.0;
                let (lw, lh) = self.logical_screen();
                self.camera_x = lw * 0.5 - (self.width as f32 * 0.5) * self.camera_zoom;
                self.camera_y = lh * 0.5 - (self.height as f32 * 0.5) * self.camera_zoom;

                self.editor_ui.mode = sow_ui::ui::map_editor::EditorMode::Brush;
                self.editor_ui.clear_busy();
                self.osm_picker.sel_anchor_world = None;
                self.osm_picker.sel_corner_world = None;
                self.notify_info(&strings.msg_osm_generated);
            }
            Err(e) => {
                self.editor_ui.clear_busy();
                self.notify_error(strings.msg_osm_failed.replace("{}", &e));
            }
        }
    }

    fn compile_map_package(&self) -> Result<MapExportArtifacts, String> {
        if !sow_core::maps::map_within_pixel_budget(self.width, self.height) {
            return Err(format!(
                "Map is {}x{} ({} pixels); max is {}.",
                self.width,
                self.height,
                self.width as u64 * self.height as u64,
                sow_core::maps::MAX_MAP_PIXELS
            ));
        }

        let slug = sow_core::maps::map_key(&self.editor_ui.map_name);
        if slug.is_empty() {
            return Err("Map name must contain at least one letter or number.".into());
        }

        let mut pixels = vec![[0u8; 4]; (self.width * self.height) as usize];
        for (i, &byte) in self.terrain.iter().enumerate() {
            let is_land = (byte & 0b10000000) != 0;
            let mag = byte & 0b00011111;
            let mut blue = 106u8;
            if is_land {
                blue = (mag as u16 + 140).min(200) as u8;
            }
            pixels[i] = [0, 0, blue, 255];
        }

        let thumb_pixels = pixels.clone();
        let args = crate::generator::GeneratorArgs {
            width: self.width,
            height: self.height,
            pixels,
            remove_small: true,
        };
        let result = crate::generator::generate_map(args)?;

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

        let mut brotli_bytes = Vec::new();
        {
            let mut writer = brotli::CompressorWriter::new(&mut brotli_bytes, 4096, 11, 22);
            writer
                .write_all(&map_bytes)
                .map_err(|e| e.to_string())?;
            writer.flush().map_err(|e| e.to_string())?;
        }

        use image::{DynamicImage, RgbaImage};
        let rgba = RgbaImage::from_raw(
            self.width,
            self.height,
            thumb_pixels
                .iter()
                .flat_map(|p| p.iter().copied())
                .collect(),
        )
        .ok_or_else(|| "thumbnail pixel buffer size mismatch".to_string())?;
        let thumb_webp =
            crate::thumbnail::encode_square_thumbnail_webp(&DynamicImage::ImageRgba8(rgba))?;

        Ok(MapExportArtifacts {
            slug,
            map_bytes,
            brotli_bytes,
            thumb_webp,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn write_map_package_native(&mut self, artifacts: MapExportArtifacts, strings: &sow_lang::MapEditorStrings) {
        let maps_root = Self::maps_root();
        let out_dir = maps_root.join(&artifacts.slug);
        if let Err(e) = std::fs::create_dir_all(&out_dir) {
            self.notify_error(format!("Failed to create path: {e}"));
            return;
        }

        let wrote_map = std::fs::write(out_dir.join("map.bin"), &artifacts.map_bytes).is_ok();
        let wrote_br =
            std::fs::write(out_dir.join("map.bin.br"), &artifacts.brotli_bytes).is_ok();
        let wrote_thumb = std::fs::write(out_dir.join("thumbnail.webp"), &artifacts.thumb_webp).is_ok();

        if wrote_map && wrote_br && wrote_thumb {
            match Self::refresh_maps_catalog(&maps_root) {
                Ok(()) => {
                    if let Err(e) = Self::reload_local_map_catalog(
                        &mut self.client_app,
                        &self.egui_ctx,
                        Some(&artifacts.slug),
                    ) {
                        self.notify_info(format!(
                            "{} (catalog reload: {e})",
                            strings.msg_saved
                        ));
                    } else {
                        self.notify_info(&strings.msg_saved_sp);
                    }
                }
                Err(e) => {
                    self.notify_info(format!("{} (catalog: {e})", strings.msg_saved));
                }
            }
        } else {
            self.notify_error(&strings.msg_write_failed);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn download_map_package_wasm(&mut self, artifacts: MapExportArtifacts, strings: &sow_lang::MapEditorStrings) {
        let prefix = &artifacts.slug;
        crate::wasm_export::trigger_download(
            &format!("{prefix}/map.bin"),
            &artifacts.map_bytes,
        );
        crate::wasm_export::trigger_download(
            &format!("{prefix}/map.bin.br"),
            &artifacts.brotli_bytes,
        );
        crate::wasm_export::trigger_download(
            &format!("{prefix}/thumbnail.webp"),
            &artifacts.thumb_webp,
        );
        self.notify_info(&strings.msg_saved_download);
    }

    fn export_map_package(&mut self) {
        log::info!("Compiling map package from editor...");
        let lang = self.client_app.settings_state.language;
        let strings = &sow_lang::get(lang).map_editor;

        self.editor_ui.exporting = true;
        self.editor_ui.busy_message = Some(strings.msg_compiling.clone());
        self.notify_info(&strings.msg_compiling);

        match self.compile_map_package() {
            Ok(artifacts) => {
                #[cfg(not(target_arch = "wasm32"))]
                self.write_map_package_native(artifacts, strings);
                #[cfg(target_arch = "wasm32")]
                self.download_map_package_wasm(artifacts, strings);
            }
            Err(e) => {
                self.notify_error(format!("Compilation error: {e}"));
            }
        }
        self.editor_ui.clear_busy();
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
        sow_ui::ui::theme::publish_reduced_motion(
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
                self.editor_ui.spawns.push(sow_ui::ui::map_editor::SpawnRowUi {
                    x: cx,
                    y: cy,
                    name: format!("Nation {}", idx),
                    flag: "🏳".to_string(),
                });
                self.notify_info(&sow_lang::get(lang).map_editor.msg_spawn_placed);
                self.mark_dirty();
            }
            sow_ui::ui::map_editor::MapEditorAction::RemoveSpawn(idx) => {
                self.push_undo_snapshot();
                self.editor_ui.spawns.remove(idx);
                self.notify_info(&sow_lang::get(lang).map_editor.msg_spawn_removed);
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
                gp.after_submit(&sync_point, &self.render_ctx.context);
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
            std::mem::forget(this);
            (window, surface, render_ctx, gui_painter, client_app, egui_ctx)
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
