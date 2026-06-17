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

pub fn draw_map_editor(
    ui: &mut Ui,
    ctx: &Context,
    state: &mut MapEditorUiState,
    viewport: MapEditorViewport,
    osm_view: Option<&OsmPickerView>,
    lang: sow_i18n::Language,
) -> MapEditorAction {
    let strings = &sow_i18n::get(lang).map_editor;
    let compact = crate::ui::theme::compact_viewport(ctx);
    let busy = state.is_busy();
    let mut action = MapEditorAction::None;
    state.map_canvas_rect = None;
    if state.mode != EditorMode::OsmPicker {
        state.osm_drag_anchor = None;
        state.osm_selection_screen = None;
    }

    let top_frame = crate::ui::theme::map_editor_glass_frame(
        crate::ui::theme::MapEditorGlassPanel::Top,
        compact,
    );
    let side_frame = crate::ui::theme::map_editor_glass_frame(
        crate::ui::theme::MapEditorGlassPanel::Side,
        compact,
    );

    egui::Panel::top("editor_menu")
        .frame(top_frame)
        .show_inside(ui, |ui| {
            let rail_fill = crate::ui::theme::palette::button_inactive();

            // Row 1: title + exit
            ui.horizontal(|ui| {
                ui.heading(RichText::new(&strings.title).size(18.0));
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
                        .color(crate::ui::theme::palette::text_muted()),
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
                            generate_btn.custom_fill(crate::ui::theme::palette::button_inactive());
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
                    .color(crate::ui::theme::palette::text_muted()),
                );
            });

            if busy {
                ui.add_space(6.0);
                if let Some(ref msg) = state.busy_message {
                    ui.label(
                        RichText::new(msg)
                            .size(13.0)
                            .color(crate::ui::theme::palette::text_muted()),
                    );
                }
                ui.add(
                    egui::ProgressBar::new(0.0)
                        .animate(true)
                        .fill(crate::ui::theme::palette::neon_cyan()),
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
                        Some(crate::ui::theme::palette::button_inactive()),
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
                            .color(crate::ui::theme::palette::text_muted()),
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
            .frame(crate::ui::theme::standard_panel_frame(compact))
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
        &strings.confirm_exit_title,
        &strings.confirm_exit_body,
        &strings.confirm_yes,
        &strings.confirm_no,
        &mut state.show_exit_confirm,
        compact,
        || MapEditorAction::Exit,
        &mut action,
    );

    let slug = sow_core::maps::map_key(&state.map_name);
    let export_body = strings.confirm_export_body.replace("{}", &slug);
    draw_confirm_dialog(
        ctx,
        &strings.confirm_export_title,
        &export_body,
        &strings.confirm_yes,
        &strings.confirm_no,
        &mut state.show_export_confirm,
        compact,
        || MapEditorAction::Export,
        &mut action,
    );

    draw_toast(ctx, state);

    action
}

fn paint_chip(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    let accent = crate::ui::theme::palette::neon_cyan();
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
    let hovered = response.hovered();
    let visuals = crate::ui::theme::interact_card(selected, true, hovered, accent);

    if ui.is_rect_visible(rect) {
        ui.painter().rect(
            rect,
            egui::CornerRadius::same(6),
            visuals.bg,
            visuals.stroke,
            egui::StrokeKind::Inside,
        );
        crate::ui::theme::paint_premium_glow_text(
            ui.painter(),
            rect.left_center() + Vec2::new(10.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.0),
            if selected {
                accent
            } else {
                crate::ui::theme::palette::text_normal()
            },
            Color32::BLACK,
        );
    }

    response
}

fn tile_center_screen(viewport: MapEditorViewport, tx: f32, ty: f32) -> egui::Pos2 {
    egui::Pos2::new(
        tx * viewport.zoom + viewport.camera_x,
        ty * viewport.zoom + viewport.camera_y,
    )
}

fn draw_osm_picker_canvas(ui: &mut Ui, view: &OsmPickerView, state: &mut MapEditorUiState) {
    let rect = ui.max_rect();
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(30, 30, 30));
    for tile in &view.tiles {
        if rect.intersects(tile.rect) {
            painter.image(
                tile.texture,
                tile.rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }

    if response.drag_started_by(egui::PointerButton::Primary) {
        if let Some(pos) = response.interact_pointer_pos() {
            state.osm_drag_anchor = Some(pos);
        }
    }
    if response.dragged_by(egui::PointerButton::Primary) {
        if let (Some(start), Some(current)) =
            (state.osm_drag_anchor, response.interact_pointer_pos())
        {
            state.osm_selection_screen = Some(egui::Rect::from_two_pos(start, current));
        }
    }
    if response.drag_stopped() {
        state.osm_drag_anchor = None;
    }

    let sel = state.osm_selection_screen.or(view.selection_screen_rect);
    if let Some(sel) = sel {
        painter.rect_stroke(
            sel,
            0.0,
            Stroke::new(2.0_f32, crate::ui::theme::palette::neon_cyan()),
            egui::StrokeKind::Outside,
        );
        painter.rect_filled(sel, 0.0, Color32::from_rgba_unmultiplied(6, 182, 212, 40));
    }
}

fn draw_viewport_overlay(ui: &mut Ui, viewport: MapEditorViewport, state: &MapEditorUiState) {
    if !ui.is_rect_visible(ui.max_rect()) {
        return;
    }
    let painter = ui.painter();
    let accent = crate::ui::theme::palette::neon_cyan();

    for spawn in &state.spawns {
        let center = tile_center_screen(viewport, spawn.x as f32 + 0.5, spawn.y as f32 + 0.5);
        if !ui.clip_rect().contains(center) {
            continue;
        }
        let radius = (viewport.zoom * 5.0).clamp(4.0, 24.0);
        painter.circle_filled(
            center,
            radius,
            Color32::from_rgba_unmultiplied(6, 182, 212, 60),
        );
        painter.circle_stroke(center, radius, Stroke::new(2.0_f32, accent));
        painter.text(
            center,
            Align2::CENTER_CENTER,
            &spawn.flag,
            egui::FontId::proportional((radius * 1.2).clamp(12.0, 22.0)),
            Color32::WHITE,
        );
        crate::ui::theme::paint_premium_glow_text(
            painter,
            center + Vec2::new(0.0, radius + 6.0),
            Align2::CENTER_TOP,
            &spawn.name,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
            Color32::BLACK,
        );
    }

    let map_rect = ui.max_rect();
    if map_rect.contains(egui::pos2(viewport.pointer_x, viewport.pointer_y)) {
        let world_x = (viewport.pointer_x - viewport.camera_x) / viewport.zoom;
        let world_y = (viewport.pointer_y - viewport.camera_y) / viewport.zoom;
        let cx = world_x.round() + 0.5;
        let cy = world_y.round() + 0.5;
        let center = tile_center_screen(viewport, cx, cy);
        let brush_r = state.brush_size as f32 * viewport.zoom;
        painter.circle_stroke(
            center,
            brush_r,
            Stroke::new(1.5_f32, accent.linear_multiply(0.85)),
        );
        painter.circle_filled(center, 3.0_f32, accent.linear_multiply(0.9));
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_confirm_dialog(
    ctx: &Context,
    title: &str,
    body: &str,
    yes_label: &str,
    no_label: &str,
    open: &mut bool,
    compact: bool,
    on_yes: impl FnOnce() -> MapEditorAction,
    action: &mut MapEditorAction,
) {
    if !*open {
        return;
    }
    let mut still_open = true;
    egui::Window::new(title)
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .frame(crate::ui::theme::standard_panel_frame(compact))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(RichText::new(body).size(14.0));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .add(
                        ThemeButton::new(yes_label)
                            .style(ThemeButtonStyle::Primary)
                            .text_size(14.0),
                    )
                    .clicked()
                {
                    *action = on_yes();
                    *open = false;
                }
                ui.add_space(8.0);
                if ui
                    .add(
                        ThemeButton::new(no_label)
                            .style(ThemeButtonStyle::Tertiary)
                            .text_size(14.0),
                    )
                    .clicked()
                {
                    *open = false;
                }
            });
        });
    if !still_open {
        *open = false;
    }
}

fn draw_toast(ctx: &Context, state: &mut MapEditorUiState) {
    const DISPLAY_SECS: f32 = 2.5;

    if let Some(start) = state.toast_started {
        if state.toast_message.is_some() && start.elapsed().as_secs_f32() >= DISPLAY_SECS {
            state.toast_message = None;
            state.toast_started = None;
        }
    }

    let is_active = state.toast_message.is_some();
    let anim = crate::ui::theme::anim_duration_from_ctx(ctx);
    let progress = ctx.animate_bool_with_time(egui::Id::new("map_editor_toast"), is_active, anim);

    if progress <= 0.01 && !is_active {
        state.toast_last_message = None;
        return;
    }

    if progress > 0.0 && progress < 1.0 {
        ctx.request_repaint();
    }

    let message = match state.toast_last_message.clone() {
        Some(msg) => msg,
        None => return,
    };

    let alpha = progress;
    let accent = if state.toast_is_error {
        crate::ui::theme::palette::danger()
    } else {
        crate::ui::theme::palette::neon_cyan()
    };
    let bg_color = Color32::from_rgba_unmultiplied(15, 23, 42, (180.0 * alpha) as u8);
    let border_color = accent.linear_multiply(alpha);
    let text_color = Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8);

    egui::Area::new(egui::Id::new("map_editor_toast_area"))
        .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -50.0))
        .order(Order::Tooltip)
        .show(ctx, |ui| {
            let frame_response = egui::Frame::new()
                .fill(bg_color)
                .stroke(Stroke::new(1.0_f32, border_color))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(16, 8))
                .show(ui, |ui| {
                    ui.label(RichText::new(message).color(text_color).size(13.0).strong());
                });
            if frame_response.response.clicked() {
                state.toast_message = None;
                state.toast_started = None;
            }
        });
}
