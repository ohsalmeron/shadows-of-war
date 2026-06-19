use super::state::*;
#[cfg(feature = "osm")]
use crate::heightmap::{terrain_stats_from_packed, WorldHeightmap};
#[cfg(feature = "osm")]
use crate::image_pipeline::generate_from_rgba;
#[cfg(feature = "osm")]
use crate::osm_tiles::{
    classify_osm_to_rgba_with_heightmap, fetch_region_blocking, lonlat_to_world_px,
    pick_fetch_zoom, tiles_covering_rect, world_px_to_lonlat, CachedTile, MAX_TILE_ZOOM, TILE_SIZE,
};

impl MapEditorSession {
    pub(crate) fn osm_center_world_px(&self) -> (f64, f64) {
        lonlat_to_world_px(
            self.osm_picker.center_lon,
            self.osm_picker.center_lat,
            self.osm_picker.zoom,
        )
    }

    #[cfg(feature = "osm")]
    pub(crate) fn screen_to_world_px(&self, sx: f32, sy: f32) -> (f64, f64) {
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
    pub(crate) fn world_px_to_screen(&self, wx: f64, wy: f64) -> egui::Pos2 {
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
    pub(crate) fn selection_world_square(&self) -> Option<(f64, f64, f64)> {
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
    pub(crate) fn update_osm_tiles(&mut self) {
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
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                    let handle =
                        self.egui_ctx
                            .load_texture(name, color_image, egui::TextureOptions::LINEAR);
                    self.osm_picker.textures.insert(key, handle);
                }
            }
        }
    }

    #[cfg(feature = "osm")]
    pub(crate) fn build_osm_view(&self) -> sow_ui::ui::map_editor::OsmPickerView {
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
    pub(crate) fn pan_osm(&mut self, dx: f32, dy: f32) {
        let z = self.osm_picker.zoom;
        let (cx, cy) = self.osm_center_world_px();
        let (lon, lat) = world_px_to_lonlat(cx - dx as f64, cy - dy as f64, z);
        self.osm_picker.center_lon = lon.clamp(-180.0, 180.0);
        self.osm_picker.center_lat = lat.clamp(-85.0, 85.0);
    }

    #[cfg(feature = "osm")]
    pub(crate) fn zoom_osm(&mut self, delta: f32) {
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

    pub(crate) fn enter_osm_view(&mut self) {
        self.release_brush_renderer();
        self.osm_picker = OsmPickerState::default();
        self.editor_ui.osm_drag_anchor = None;
        self.editor_ui.osm_selection_screen = None;
    }

    #[cfg(feature = "osm")]
    pub(crate) fn apply_osm_selection_from_screen(&mut self) {
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
    pub(crate) fn refresh_map_renderer_terrain(&mut self) {
        self.dirty_tiles.clear();
        if let Some(mut mr) = self.map_renderer.take() {
            mr.destroy(&self.render_ctx);
        }
        self.ensure_brush_renderer();
    }

    #[cfg(feature = "osm")]
    pub(crate) fn generate_from_osm(&mut self) {
        let lang = self.client_app.settings_state.language;
        let strings = &sow_i18n::get(lang).map_editor;

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
            &stitched, min_lon, min_lat, max_lon, max_lat, &heightmap,
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
}
