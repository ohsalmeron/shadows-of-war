use crate::widgets::{ThemeButton, ThemeButtonStyle};
use egui::{Align2, Color32, Context, Frame, Order, RichText, Sense, Stroke, Ui, Vec2};
use web_time::Instant;

/// Camera and pointer state for map-canvas overlays (spawn markers, brush preview).
#[derive(Clone, Copy, Debug)]
pub struct MapEditorViewport {
    pub camera_x: f32,
    pub camera_y: f32,
    pub zoom: f32,
    /// Logical screen size (matches egui coordinates).
    pub screen_w: f32,
    pub screen_h: f32,
    pub pointer_x: f32,
    pub pointer_y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EditorMode {
    #[default]
    Brush,
    OsmPicker,
}

#[derive(Clone, Debug, Default)]
pub struct OsmPickerUiState {
    pub target_size: u32,
    pub generating: bool,
}

#[derive(Clone, Debug)]
pub struct OsmPickerTileDraw {
    pub rect: egui::Rect,
    pub texture: egui::TextureId,
}

#[derive(Clone, Debug, Default)]
pub struct OsmPickerView {
    pub center_lon: f64,
    pub center_lat: f64,
    pub zoom: u32,
    pub tiles: Vec<OsmPickerTileDraw>,
    pub selection_screen_rect: Option<egui::Rect>,
    /// Lon/lat bounds of current selection (for side panel).
    pub selection_bbox: Option<(f64, f64, f64, f64)>,
    pub overpass_tile_estimate: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorPaintKind {
    Water,
    Ocean,
    Shoreline,
    Plains,
    Highlands,
    Mountains,
}

#[derive(Clone, Debug)]
pub struct SpawnRowUi {
    pub name: String,
    pub flag: String,
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Debug)]
pub struct MapEditorUiState {
    pub mode: EditorMode,
    pub osm: OsmPickerUiState,
    pub width: u32,
    pub height: u32,
    pub map_name: String,
    pub selected_paint: EditorPaintKind,
    pub brush_size: i32,
    pub brush_strength: f64,
    pub spawns: Vec<SpawnRowUi>,
    pub show_new_dialog: bool,
    pub show_exit_confirm: bool,
    pub show_export_confirm: bool,
    pub is_dirty: bool,
    pub show_npcs_panel: bool,
    pub npcs_panel_saved: bool,
    pub new_map_w: u32,
    pub new_map_h: u32,
    pub toast_message: Option<String>,
    pub toast_is_error: bool,
    pub exporting: bool,
    pub busy_message: Option<String>,
    /// Set each frame by `draw_map_editor` — click/drag painting only inside this rect.
    pub map_canvas_rect: Option<egui::Rect>,
    /// Left-drag on OSM map (egui coordinates).
    pub osm_drag_anchor: Option<egui::Pos2>,
    pub osm_selection_screen: Option<egui::Rect>,
    toast_last_message: Option<String>,
    toast_started: Option<Instant>,
}

impl Default for MapEditorUiState {
    fn default() -> Self {
        Self {
            mode: EditorMode::Brush,
            osm: OsmPickerUiState {
                target_size: 1000,
                generating: false,
            },
            width: 400,
            height: 300,
            map_name: "custom_map".to_string(),
            selected_paint: EditorPaintKind::Plains,
            brush_size: 8,
            brush_strength: 15.0,
            spawns: Vec::new(),
            show_new_dialog: false,
            show_exit_confirm: false,
            show_export_confirm: false,
            is_dirty: false,
            show_npcs_panel: false,
            npcs_panel_saved: true,
            new_map_w: 400,
            new_map_h: 300,
            toast_message: None,
            toast_is_error: false,
            exporting: false,
            busy_message: None,
            map_canvas_rect: None,
            osm_drag_anchor: None,
            osm_selection_screen: None,
            toast_last_message: None,
            toast_started: None,
        }
    }
}

impl MapEditorUiState {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            brush_size: 3,
            brush_strength: 5.0,
            selected_paint: EditorPaintKind::Plains,
            ..Default::default()
        }
    }

    pub fn show_toast(&mut self, message: impl Into<String>, is_error: bool) {
        let msg = message.into();
        self.toast_last_message = Some(msg.clone());
        self.toast_message = Some(msg);
        self.toast_is_error = is_error;
        self.toast_started = Some(Instant::now());
    }

    pub fn is_busy(&self) -> bool {
        self.osm.generating || self.exporting
    }

    pub fn clear_busy(&mut self) {
        self.osm.generating = false;
        self.exporting = false;
        self.busy_message = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapEditorAction {
    None,
    Exit,
    Export,
    ToggleNewDialog,
    CreateBlankMap,
    PlaceSpawn,
    RemoveSpawn(usize),
    EnterOsmPicker,
    ExitOsmPicker,
    GenerateFromOsm,
    Undo,
}

const TOOLBAR_BTN_H: f32 = 36.0;
const TOOLBAR_BTN_MIN_W: f32 = 120.0;
const TOOLBAR_TEXT: f32 = 15.0;

fn toolbar_button(
    ui: &mut Ui,
    label: &str,
    style: ThemeButtonStyle,
    custom_fill: Option<Color32>,
) -> egui::Response {
    let mut btn = ThemeButton::new(label)
        .style(style)
        .min_size(Vec2::new(TOOLBAR_BTN_MIN_W, TOOLBAR_BTN_H))
        .text_size(TOOLBAR_TEXT);
    if let Some(fill) = custom_fill {
        btn = btn.custom_fill(fill);
    }
    ui.add(btn)
}

