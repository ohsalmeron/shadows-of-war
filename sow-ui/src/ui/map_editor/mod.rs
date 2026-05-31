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
    pub width: u32,
    pub height: u32,
    pub map_name: String,
    pub selected_paint: EditorPaintKind,
    pub brush_size: i32,
    pub brush_strength: f64,
    pub spawns: Vec<SpawnRowUi>,
    pub show_new_dialog: bool,
    pub new_map_w: u32,
    pub new_map_h: u32,
    pub toast_message: Option<String>,
    pub toast_is_error: bool,
    /// Set each frame by `draw_map_editor` — click/drag painting only inside this rect.
    pub map_canvas_rect: Option<egui::Rect>,
    toast_last_message: Option<String>,
    toast_started: Option<Instant>,
}

impl Default for MapEditorUiState {
    fn default() -> Self {
        Self {
            width: 400,
            height: 300,
            map_name: "custom_map".to_string(),
            selected_paint: EditorPaintKind::Plains,
            brush_size: 8,
            brush_strength: 15.0,
            spawns: Vec::new(),
            show_new_dialog: false,
            new_map_w: 400,
            new_map_h: 300,
            toast_message: None,
            toast_is_error: false,
            map_canvas_rect: None,
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
}

pub fn draw_map_editor(
    ui: &mut Ui,
    ctx: &Context,
    state: &mut MapEditorUiState,
    viewport: MapEditorViewport,
    lang: sow_lang::Language,
) -> MapEditorAction {
    let strings = &sow_lang::get(lang).map_editor;
    let compact = crate::ui::theme::compact_viewport(ctx);
    let mut action = MapEditorAction::None;
    state.map_canvas_rect = None;

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
            ui.horizontal(|ui| {
                ui.heading(&strings.title);
                ui.add_space(20.0);

                let rail_fill = crate::ui::theme::menu_secondary_button();
                if ui
                    .add(
                        ThemeButton::new(&strings.btn_new)
                            .style(ThemeButtonStyle::Tertiary)
                            .custom_fill(rail_fill)
                            .text_size(14.0),
                    )
                    .clicked()
                {
                    action = MapEditorAction::ToggleNewDialog;
                }

                ui.add_space(10.0);
                if ui
                    .add(
                        ThemeButton::new(&strings.btn_export)
                            .style(ThemeButtonStyle::Primary)
                            .text_size(14.0),
                    )
                    .clicked()
                {
                    action = MapEditorAction::Export;
                }

                ui.add_space(20.0);
                ui.label(
                    RichText::new(
                        strings
                            .label_size
                            .replacen("{}", &state.width.to_string(), 1)
                            .replacen("{}", &state.height.to_string(), 1),
                    )
                    .color(crate::ui::theme::text_secondary()),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            ThemeButton::new(&strings.btn_exit)
                                .style(ThemeButtonStyle::Tertiary)
                                .custom_fill(rail_fill)
                                .text_size(14.0),
                        )
                        .clicked()
                    {
                        action = MapEditorAction::Exit;
                    }
                });
            });
        });

    egui::Panel::left("brush_panel")
        .default_size(240.0)
        .frame(side_frame)
        .show_inside(ui, |ui| {
            ui.heading(&strings.heading_brush);
            ui.separator();
            ui.add_space(10.0);

            ui.label(&strings.label_terrain);
            if paint_chip(ui, &strings.paint_plains, state.selected_paint == EditorPaintKind::Plains)
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
            if paint_chip(ui, &strings.paint_lake, state.selected_paint == EditorPaintKind::Water)
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
            ui.add(
                egui::Slider::new(&mut state.brush_strength, 1.0..=31.0).show_value(false),
            );

            ui.add_space(30.0);
            ui.heading(&strings.heading_instructions);
            ui.small(&strings.instructions_body);
        });

    egui::Panel::right("spawns_panel")
        .default_size(260.0)
        .frame(side_frame)
        .show_inside(ui, |ui| {
            ui.heading(&strings.heading_spawns);
            ui.separator();
            ui.add_space(10.0);

            if ui
                .add(
                    ThemeButton::new(&strings.btn_place_spawn)
                        .style(ThemeButtonStyle::Secondary)
                        .text_size(14.0),
                )
                .clicked()
            {
                action = MapEditorAction::PlaceSpawn;
            }

            ui.add_space(15.0);
            ui.label(&strings.label_placed_spawns);

            let mut to_remove = None;
            egui::ScrollArea::vertical()
                .max_height(200.0)
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

            ui.add_space(20.0);
            ui.label(&strings.label_metadata_name);
            ui.text_edit_singleline(&mut state.map_name);
        });

    // Pass pointer events through the map viewport; draw spawn markers and brush preview here.
    egui::CentralPanel::default()
        .frame(Frame::NONE)
        .show_inside(ui, |ui| {
            let map_rect = ui.max_rect();
            state.map_canvas_rect = Some(map_rect);
            ui.allocate_rect(map_rect, Sense::empty());
            draw_viewport_overlay(ui, viewport, state);
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
                    state.new_map_w = state.new_map_w.clamp(100, 2000);
                });
                ui.horizontal(|ui| {
                    ui.label(&strings.label_height);
                    ui.add(egui::DragValue::new(&mut state.new_map_h));
                    state.new_map_h = state.new_map_h.clamp(100, 2000);
                });
                ui.add_space(10.0);
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
            });
        if !open {
            state.show_new_dialog = false;
        }
    }

    draw_toast(ctx, state);

    action
}

fn paint_chip(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    let accent = crate::ui::theme::accent_solo_cyan();
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
        crate::ui::theme::outlined_text(
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

fn draw_viewport_overlay(ui: &mut Ui, viewport: MapEditorViewport, state: &MapEditorUiState) {
    if !ui.is_rect_visible(ui.max_rect()) {
        return;
    }
    let painter = ui.painter();
    let accent = crate::ui::theme::accent_solo_cyan();

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
        crate::ui::theme::outlined_text(
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
        painter.circle_filled(
            center,
            3.0_f32,
            accent.linear_multiply(0.9),
        );
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
    let progress = ctx.animate_bool_with_time(egui::Id::new("map_editor_toast"), is_active, 0.22);

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
        crate::ui::theme::accent_danger()
    } else {
        crate::ui::theme::accent_solo_cyan()
    };
    let bg_color = Color32::from_rgba_unmultiplied(15, 23, 42, (180.0 * alpha) as u8);
    let border_color = accent.linear_multiply(alpha);
    let text_color = Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8);

    egui::Area::new(egui::Id::new("map_editor_toast_area"))
        .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -50.0))
        .order(Order::Tooltip)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(bg_color)
                .stroke(Stroke::new(1.0_f32, border_color))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(16, 8))
                .show(ui, |ui| {
                    ui.label(RichText::new(message).color(text_color).size(13.0).strong());
                });
        });
}
