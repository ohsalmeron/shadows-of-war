use blade_egui::GuiPainter;
use blade_graphics as gpu;
use egui::Context;
use sow_render::{MapGlobals, MapRenderer, RenderContext};
use sow_ui::ClientApp;
use std::path::PathBuf;
use web_time::Instant;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

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

    // Brush configurations
    pub brush_size: i32,
    pub selected_paint: PaintType,
    pub brush_strength: f64,

    // Navigation state
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub dragging: bool,
    pub last_mouse_x: f64,
    pub last_mouse_y: f64,
    pub screen_w: f32,
    pub screen_h: f32,

    // UI state
    pub egui_ctx: Context,
    pub raw_input: egui::RawInput,
    pub client_app: ClientApp,
    pub last_frame_time: Instant,
    pub start_time: Instant,

    // Map metadata & spawns
    pub map_name: String,
    pub spawns: Vec<NationSpawn>,
    pub notification: Option<(String, Instant)>,

    // New Map dimensions
    pub new_map_w: u32,
    pub new_map_h: u32,
    pub show_new_dialog: bool,
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

#[derive(Clone)]
pub struct NationSpawn {
    pub x: u32,
    pub y: u32,
    pub name: String,
    pub flag: String,
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

        let camera_zoom = 1.0f32;
        let camera_x = 1280.0 * 0.5 - (width as f32 * 0.5) * camera_zoom;
        let camera_y = 720.0 * 0.5 - (height as f32 * 0.5) * camera_zoom;

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

            brush_size: 3,
            selected_paint: PaintType::Plains,
            brush_strength: 5.0,

            camera_x,
            camera_y,
            camera_zoom,
            dragging: false,
            last_mouse_x: 0.0,
            last_mouse_y: 0.0,
            screen_w: 1280.0,
            screen_h: 720.0,

            egui_ctx,
            raw_input: egui::RawInput::default(),
            client_app,
            last_frame_time: Instant::now(),
            start_time: Instant::now(),

            map_name: "custom_map".to_string(),
            spawns: Vec::new(),
            notification: None,

            new_map_w: 400,
            new_map_h: 300,
            show_new_dialog: false,
        }
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
                    self.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::new(self.screen_w, self.screen_h),
                    ));
                }
            }
            WindowEvent::PointerMoved {
                position, primary, ..
            } => {
                if primary {
                    let sf = self.window.as_ref().map_or(1.0, |w| w.scale_factor());
                    let logical_x = position.x / sf;
                    let logical_y = position.y / sf;
                    let dx = position.x - self.last_mouse_x;
                    let dy = position.y - self.last_mouse_y;
                    self.last_mouse_x = position.x;
                    self.last_mouse_y = position.y;
                    if self.dragging && !self.egui_ctx.egui_wants_pointer_input() {
                        self.camera_x += dx as f32;
                        self.camera_y += dy as f32;
                    }
                    self.raw_input
                        .events
                        .push(egui::Event::PointerMoved(egui::Pos2::new(
                            logical_x as f32,
                            logical_y as f32,
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
                let pressed = state == winit::event::ElementState::Pressed;
                if primary {
                    self.last_mouse_x = position.x;
                    self.last_mouse_y = position.y;
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
                } else if is_left && pressed && !self.egui_ctx.egui_wants_pointer_input() {
                    self.paint_at_cursor();
                }

                if primary {
                    let sf = self.window.as_ref().map_or(1.0, |w| w.scale_factor());
                    let logical_x = position.x / sf;
                    let logical_y = position.y / sf;
                    self.raw_input.events.push(egui::Event::PointerButton {
                        pos: egui::Pos2::new(logical_x as f32, logical_y as f32),
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
                        modifiers: Default::default(),
                    });
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.egui_ctx.egui_wants_pointer_input() {
                    let (unit, vec_delta) = match delta {
                        winit::event::MouseScrollDelta::LineDelta(x, y) => {
                            (egui::MouseWheelUnit::Line, egui::vec2(x, y))
                        }
                        winit::event::MouseScrollDelta::PixelDelta(pos) => {
                            let sf = self
                                .window
                                .as_ref()
                                .map_or(1.0, |w| w.scale_factor() as f32);
                            (
                                egui::MouseWheelUnit::Point,
                                egui::vec2(pos.x as f32 / sf, pos.y as f32 / sf),
                            )
                        }
                    };

                    self.raw_input.events.push(egui::Event::MouseWheel {
                        unit,
                        delta: vec_delta,
                        phase: egui::TouchPhase::Move,
                        modifiers: self.raw_input.modifiers,
                    });
                } else {
                    let scroll = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => y * 30.0,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                    };
                    let zoom_speed = 0.002f32;
                    let old_zoom = self.camera_zoom;
                    self.camera_zoom =
                        (self.camera_zoom * (1.0 + scroll * zoom_speed)).clamp(0.2, 10.0);

                    // Adjust camera position to zoom toward cursor
                    let mx = self.last_mouse_x as f32;
                    let my = self.last_mouse_y as f32;
                    self.camera_x = mx - (mx - self.camera_x) * (self.camera_zoom / old_zoom);
                    self.camera_y = my - (my - self.camera_y) * (self.camera_zoom / old_zoom);
                }
            }
            _ => {}
        }
    }

    fn paint_at_cursor(&mut self) {
        let mx = self.last_mouse_x as f32;
        let my = self.last_mouse_y as f32;

        let world_x = (mx - self.camera_x) / self.camera_zoom;
        let world_y = (my - self.camera_y) / self.camera_zoom;

        let cx = world_x.round() as i32;
        let cy = world_y.round() as i32;
        let r = self.brush_size;

        for dx in -r..=r {
            for dy in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    let tx = cx + dx;
                    let ty = cy + dy;

                    if tx >= 0 && tx < self.width as i32 && ty >= 0 && ty < self.height as i32 {
                        let idx = (ty * self.width as i32 + tx) as usize;
                        let mut byte = 0u8;

                        match self.selected_paint {
                            PaintType::Water => {
                                byte |= 0b00000000; // Water, not land, not shore, not ocean
                                byte |= (self.brush_strength as u8).min(31);
                            }
                            PaintType::Ocean => {
                                byte |= 0b00100000; // Ocean
                                byte |= (self.brush_strength as u8).min(31);
                            }
                            PaintType::Shoreline => {
                                byte |= 0b01000000; // Shoreline
                            }
                            PaintType::Plains => {
                                byte |= 0b10000000; // Land
                                byte |= (self.brush_strength.min(9.0) as u8) & 0b00011111;
                            }
                            PaintType::Highlands => {
                                byte |= 0b10000000; // Land
                                byte |= (self.brush_strength.clamp(10.0, 19.0) as u8) & 0b00011111;
                            }
                            PaintType::Mountains => {
                                byte |= 0b10000000; // Land
                                byte |= (self.brush_strength.clamp(20.0, 31.0) as u8) & 0b00011111;
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
        self.spawns.clear();
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
        self.camera_x = self.screen_w * 0.5 - (self.width as f32 * 0.5) * self.camera_zoom;
        self.camera_y = self.screen_h * 0.5 - (self.height as f32 * 0.5) * self.camera_zoom;
        let msg = sow_lang::get(self.client_app.settings_state.language)
            .map_editor
            .msg_blank_created
            .clone();
        self.notify(&msg);
    }

    fn notify(&mut self, text: &str) {
        self.notification = Some((text.to_string(), Instant::now()));
    }

    fn export_map_package(&mut self) {
        log::info!("Starting native Rust map compilation...");
        let lang = self.client_app.settings_state.language;
        let strings = &sow_lang::get(lang).map_editor;
        self.notify(&strings.msg_compiling);

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
                // Save output package directly to active directory assets for seamless integration
                let out_dir = PathBuf::from("assets/maps").join(&self.map_name);
                if let Err(e) = std::fs::create_dir_all(&out_dir) {
                    self.notify(&format!("Failed to create path: {}", e));
                    return;
                }

                if std::fs::write(out_dir.join("map.bin"), &result.map_data).is_ok()
                    && std::fs::write(out_dir.join("mini_map.bin"), &result.mini_map_data).is_ok()
                    && std::fs::write(out_dir.join("thumbnail.webp"), &result.thumbnail_data)
                        .is_ok()
                {
                    // Construct and serialize manifest metadata
                    let manifest = serde_json::json!({
                        "name": self.map_name,
                        "nations": self.spawns.iter().map(|s| {
                            serde_json::json!({
                                "name": s.name,
                                "flag": s.flag,
                                "coordinates": [s.x, s.y]
                            })
                        }).collect::<Vec<_>>()
                    });

                    if let Ok(manifest_bytes) = serde_json::to_vec_pretty(&manifest) {
                        let _ = std::fs::write(out_dir.join("manifest.json"), manifest_bytes);
                    }

                    self.notify(&strings.msg_saved);
                } else {
                    self.notify(&strings.msg_write_failed);
                }
            }
            Err(e) => {
                self.notify(&format!("Compilation error: {}", e));
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
        let strings = &sow_lang::get(lang).map_editor;

        static REGISTER_ONCE: std::sync::Once = std::sync::Once::new();
        REGISTER_ONCE.call_once(|| {
            sow_core::register_game_assets!(self.egui_ctx, "../../sow-client/assets/");
        });

        let egui_ctx = self.egui_ctx.clone();
        let egui_output = egui_ctx.run_ui(self.raw_input.clone(), |ui| {
            let transparent_bg = sow_ui::ui::theme::panel_bg_transparent();

            let top_frame = egui::Frame::new()
                .fill(transparent_bg)
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    sow_ui::ui::theme::palette::field_border(),
                ))
                .corner_radius(12.0)
                .inner_margin(egui::Margin::symmetric(20, 14))
                .shadow(egui::Shadow {
                    blur: 16,
                    spread: 0,
                    color: egui::Color32::from_rgba_unmultiplied(6, 182, 212, 15),
                    offset: [0, 6],
                });

            let side_frame = egui::Frame::new()
                .fill(transparent_bg)
                .stroke(egui::Stroke::new(
                    1.0_f32,
                    sow_ui::ui::theme::palette::field_border(),
                ))
                .corner_radius(12.0)
                .inner_margin(egui::Margin::symmetric(16, 20))
                .shadow(egui::Shadow {
                    blur: 24,
                    spread: 0,
                    color: egui::Color32::from_rgba_unmultiplied(6, 182, 212, 20),
                    offset: [0, 8],
                });

            // Draw premium transparent top panel
            egui::Panel::top("editor_menu")
                .frame(top_frame)
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(&strings.title);
                        ui.add_space(20.0);

                        if ui.button(&strings.btn_new).clicked() {
                            self.show_new_dialog = !self.show_new_dialog;
                        }

                        ui.add_space(10.0);
                        if ui.button(&strings.btn_export).clicked() {
                            self.export_map_package();
                        }

                        ui.add_space(20.0);
                        ui.label(
                            strings
                                .label_size
                                .replacen("{}", &self.width.to_string(), 1)
                                .replacen("{}", &self.height.to_string(), 1),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(&strings.btn_exit).clicked() {
                                transition = Some(sow_ui::UiAction::LeaveLobby);
                            }
                        });
                    });
                });

            // Draw left painting brush controller
            egui::Panel::left("brush_panel")
                .default_size(240.0)
                .frame(side_frame)
                .show_inside(ui, |ui| {
                    ui.heading(&strings.heading_brush);
                    ui.separator();
                    ui.add_space(10.0);

                    ui.label(&strings.label_terrain);
                    ui.radio_value(
                        &mut self.selected_paint,
                        PaintType::Plains,
                        &strings.paint_plains,
                    );
                    ui.radio_value(
                        &mut self.selected_paint,
                        PaintType::Highlands,
                        &strings.paint_highlands,
                    );
                    ui.radio_value(
                        &mut self.selected_paint,
                        PaintType::Mountains,
                        &strings.paint_mountains,
                    );
                    ui.radio_value(
                        &mut self.selected_paint,
                        PaintType::Water,
                        &strings.paint_lake,
                    );
                    ui.radio_value(
                        &mut self.selected_paint,
                        PaintType::Ocean,
                        &strings.paint_ocean,
                    );
                    ui.radio_value(
                        &mut self.selected_paint,
                        PaintType::Shoreline,
                        &strings.paint_shoreline,
                    );

                    ui.add_space(15.0);
                    ui.label(
                        strings
                            .label_brush_size
                            .replace("{}", &self.brush_size.to_string()),
                    );
                    ui.add(egui::Slider::new(&mut self.brush_size, 1..=20).show_value(false));

                    ui.add_space(15.0);
                    ui.label(
                        strings
                            .label_strength
                            .replace("{:.1}", &format!("{:.1}", self.brush_strength)),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.brush_strength, 1.0..=31.0).show_value(false),
                    );

                    // Quick help panel
                    ui.add_space(30.0);
                    ui.heading(&strings.heading_instructions);
                    ui.small(&strings.instructions_body);
                });

            // Draw right spawn controller panel
            egui::Panel::right("spawns_panel")
                .default_size(260.0)
                .frame(side_frame)
                .show_inside(ui, |ui| {
                    ui.heading(&strings.heading_spawns);
                    ui.separator();
                    ui.add_space(10.0);

                    // Add spawn interface
                    ui.horizontal(|ui| {
                        if ui.button(&strings.btn_place_spawn).clicked() {
                            let cx = self.width / 2;
                            let cy = self.height / 2;
                            let idx = self.spawns.len() + 1;
                            self.spawns.push(NationSpawn {
                                x: cx,
                                y: cy,
                                name: format!("Nation {}", idx),
                                flag: "🏳".to_string(),
                            });
                            self.notify(&strings.msg_spawn_placed);
                        }
                    });

                    ui.add_space(15.0);
                    ui.label(&strings.label_placed_spawns);

                    let mut to_remove = None;
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for (i, spawn) in self.spawns.iter_mut().enumerate() {
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.text_edit_singleline(&mut spawn.name)
                                            .on_hover_text(&strings.hover_nation_name);
                                        ui.text_edit_singleline(&mut spawn.flag)
                                            .on_hover_text(&strings.hover_flag);
                                        if ui.button("🗑").clicked() {
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
                        self.spawns.remove(idx);
                        self.notify(&strings.msg_spawn_removed);
                    }

                    ui.add_space(20.0);
                    ui.label(&strings.label_metadata_name);
                    ui.text_edit_singleline(&mut self.map_name);
                });

            // Draw New Map modal dialog
            let mut show_new_dialog = self.show_new_dialog;
            let mut create_clicked = false;
            if show_new_dialog {
                egui::Window::new(&strings.win_new_title)
                    .open(&mut show_new_dialog)
                    .resizable(false)
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ui.ctx(), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(&strings.label_width);
                            ui.add(egui::DragValue::new(&mut self.new_map_w));
                            self.new_map_w = self.new_map_w.clamp(100, 2000);
                        });
                        ui.horizontal(|ui| {
                            ui.label(&strings.label_height);
                            ui.add(egui::DragValue::new(&mut self.new_map_h));
                            self.new_map_h = self.new_map_h.clamp(100, 2000);
                        });
                        ui.add_space(10.0);
                        if ui.button(&strings.btn_create_map).clicked() {
                            create_clicked = true;
                        }
                    });
            }
            if create_clicked {
                self.new_blank_map(self.new_map_w, self.new_map_h);
                self.show_new_dialog = false;
            } else {
                self.show_new_dialog = show_new_dialog;
            }

            // Draw transient global notices
            if let Some((ref text, ref start)) = self.notification {
                if start.elapsed().as_secs_f32() < 4.0 {
                    egui::Window::new("Notification")
                        .title_bar(false)
                        .collapsible(false)
                        .resizable(false)
                        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -50.0))
                        .show(ui.ctx(), |ui| {
                            ui.label(text);
                        });
                } else {
                    self.notification = None;
                }
            }
        });

        self.raw_input.events.clear();

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
                        screen_size: [self.screen_w, self.screen_h],
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
                        hover_hex: [0.0, 0.0],
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
            std::ptr::drop_in_place(&mut self.map_name);
            std::ptr::drop_in_place(&mut self.spawns);
            std::ptr::drop_in_place(&mut self.notification);
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
