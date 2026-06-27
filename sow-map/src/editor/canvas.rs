use sow_render::MapRenderer;

use super::state::*;

impl MapEditorSession {
    pub(crate) fn push_undo_snapshot(&mut self) {
        const MAX_UNDO: usize = 20;
        self.undo_stack.push(self.terrain.clone());
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.remove(0);
        }
    }

    pub(crate) fn undo_last_stroke(&mut self) {
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

    pub(crate) fn mark_dirty(&mut self) {
        self.editor_ui.is_dirty = true;
    }

    pub(crate) fn paint_at_cursor(&mut self) {
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
    pub(crate) fn release_brush_renderer(&mut self) {
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

    pub(crate) fn ensure_brush_renderer(&mut self) {
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
}
