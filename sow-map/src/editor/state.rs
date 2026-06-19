use blade_egui::GuiPainter;
use blade_graphics as gpu;
use egui::Context;
use sow_render::{MapRenderer, RenderContext};
use sow_ui::ClientApp;
#[cfg(feature = "osm")]
use std::collections::HashMap;
use web_time::Instant;
use winit::window::Window;

#[cfg(feature = "osm")]
use crate::osm_tiles::{OsmTileCache, TileKey};
#[cfg(feature = "osm")]
pub(crate) struct OsmPickerState {
    pub(crate) center_lon: f64,
    pub(crate) center_lat: f64,
    pub(crate) zoom: u32,
    pub(crate) sel_anchor_world: Option<(f64, f64)>,
    pub(crate) sel_corner_world: Option<(f64, f64)>,
    pub(crate) cache: OsmTileCache,
    pub(crate) textures: HashMap<TileKey, egui::TextureHandle>,
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

pub(crate) struct MapExportArtifacts {
    pub(crate) slug: String,
    pub(crate) map_bytes: Vec<u8>,
    pub(crate) brotli_bytes: Vec<u8>,
    pub(crate) thumb_webp: Vec<u8>,
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
    pub(crate) osm_picker: OsmPickerState,

    pub(crate) undo_stack: Vec<Vec<u8>>,
    pub(crate) paint_stroke_snapshotted: bool,
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

pub(crate) fn paint_type_from_kind(kind: sow_ui::ui::map_editor::EditorPaintKind) -> PaintType {
    match kind {
        sow_ui::ui::map_editor::EditorPaintKind::Water => PaintType::Water,
        sow_ui::ui::map_editor::EditorPaintKind::Ocean => PaintType::Ocean,
        sow_ui::ui::map_editor::EditorPaintKind::Shoreline => PaintType::Shoreline,
        sow_ui::ui::map_editor::EditorPaintKind::Plains => PaintType::Plains,
        sow_ui::ui::map_editor::EditorPaintKind::Highlands => PaintType::Highlands,
        sow_ui::ui::map_editor::EditorPaintKind::Mountains => PaintType::Mountains,
    }
}
