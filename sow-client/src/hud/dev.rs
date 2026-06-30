use web_time::Instant;

use crate::app::SowApp;
use sow_ui_kit::ClientPhase;

impl SowApp {
    pub(crate) fn calculate_fps_and_ping(&mut self) {
        self.time.frame_count += 1;
        if self.time.last_fps_time.elapsed().as_secs_f64() >= 1.0 {
            self.time.current_fps = self.time.frame_count;
            self.time.frame_count = 0;
            self.time.last_fps_time = Instant::now();
        }

        if self.net.last_ping_time.elapsed().as_secs_f64() >= 1.0 {
            if let Some(c) = self.net.client.as_ref() {
                let ping_msg = sow_core::protocol::ClientMessage::Ping {
                    client_time: self.time.start_time.elapsed().as_secs_f64(),
                };
                if let Ok(json) = bincode::serialize(&ping_msg) {
                    c.send(json);
                }
            }
            self.net.last_ping_time = Instant::now();
        }
    }

    /// Sync snapshot attacks/fleets/players into hud_state when the sim tick advances.
    pub(crate) fn sync_hud_combat_state(&mut self) {
        let my_pid = self.sim.my_player_id.unwrap_or(0);
        self.ui.app.hud_state.my_player_id = my_pid;
        self.ui.app.hud_state.map_w = self.sim.map_w;

        if let Some(snap) = &self.sim.current_snapshot {
            if self.ui.hud_combat_sync_tick != snap.tick {
                self.ui.hud_combat_sync_tick = snap.tick;
                self.ui.app.hud_state.attacks = snap.attacks.clone();
                self.ui.app.hud_state.fleets = snap.fleets.clone();
                self.ui.app.hud_state.players = snap.players.clone();
            }
        } else if self.ui.hud_combat_sync_tick != 0 {
            self.ui.hud_combat_sync_tick = 0;
            self.ui.app.hud_state.attacks.clear();
            self.ui.app.hud_state.fleets.clear();
            self.ui.app.hud_state.players.clear();
        }
    }

    /// THE in-game dispatch ("Attacks") panel — your outgoing attacks, naval invasions, and the
    /// incoming attacks against you, as a single-column **log**.
    ///
    /// Structure / poka-yoke for future edits:
    /// * One dispatch = one single-line row. Newest on top, oldest at the bottom (sorted by
    ///   dispatch `id` descending — `id` is monotonic per spawn). Resolved dispatches simply drop
    ///   out of the snapshot, so the log self-trims from the bottom.
    /// * Direction is shown by an **arrow emoji + colour** (green out / red in / cyan navy), not by
    ///   "IN"/"OUT" text. Troop counts are abbreviated (K/M).
    /// * Rendered through the **GPU text/emoji pipeline** (`tr.push_string` / `tr.push_emoji`).
    ///   egui only draws the themed backdrop and the real **cancel button** (it needs hit-testing).
    ///   GPU text draws *under* egui, so the backdrop is kept translucent.
    /// * NOTE: `sow_ui::ui::hud::tabs::battle_log` is a *different*, currently-disabled bottom-panel
    ///   tab (gated by `ENABLE_BOTTOM_HUD_LOG_TABS`). This method is the only attacks UI in use —
    ///   don't duplicate dispatch rendering elsewhere.
    pub(crate) fn render_attacks_panel(&mut self, ctx: &egui::Context, local_cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>) {
        use sow_core::protocol::GameplayIntent;

        // Abbreviate troop counts: 903 → "903", 12_400 → "12.4K", 3_000_000 → "3M".
        fn fmt_count(n: f64) -> String {
            let n = n.max(0.0);
            let trim = |v: f64, suffix: &str| {
                format!("{}{suffix}", format!("{v:.1}").trim_end_matches(".0").to_string())
            };
            if n >= 1.0e6 {
                trim(n / 1.0e6, "M")
            } else if n >= 1.0e3 {
                trim(n / 1.0e3, "K")
            } else {
                format!("{n:.0}")
            }
        }

        if self.ui.app.phase != ClientPhase::Playing {
            return;
        }
        if self.ui.app.hud_state.bottom_dialog.is_some() {
            return;
        }
        let my_pid = self.sim.my_player_id.unwrap_or(0);

        // ── Collect owned rows first ───────────────────────────────────────────────────────────
        // Owned (not borrowing `self.sim`) so the GPU text renderer (`&mut self.gfx`) is free
        // to borrow inside the egui closure below.
        #[derive(Clone, Copy, PartialEq)]
        enum Dir {
            Out,
            In,
            Navy,
        }
        struct Row {
            id: u64,
            dir: Dir,
            troops: f64,
            name: String,
            retreating: bool,
            cancel: Option<GameplayIntent>,
            focus_x: f32,
            focus_y: f32,
        }
        let mut rows: Vec<Row> = Vec::new();
        if my_pid > 0 {
            if let Some(snap) = &self.sim.current_snapshot {
                let map_w = self.sim.map_w.max(1) as u32;
                for a in snap.attacks.iter().filter(|a| a.owner_id == my_pid) {
                    let name = snap
                        .players
                        .iter()
                        .find(|p| p.id == a.target_owner)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "Expanding".to_string());
                    rows.push(Row {
                        id: a.id,
                        dir: Dir::Out,
                        troops: a.troops,
                        name,
                        retreating: a.retreating,
                        cancel: (!a.retreating).then_some(GameplayIntent::CancelAttack { attack_id: a.id }),
                        focus_x: a.front_cx,
                        focus_y: a.front_cy,
                    });
                }
                for f in snap.fleets.iter().filter(|f| f.owner_id == my_pid) {
                    let focus_x = (f.current_tile % map_w) as f32 + 0.5;
                    let focus_y = (f.current_tile / map_w) as f32 + 0.5;
                    rows.push(Row {
                        id: f.id,
                        dir: Dir::Navy,
                        troops: f.troops,
                        name: "Naval Invasion".to_string(),
                        retreating: f.retreating,
                        cancel: (!f.retreating).then_some(GameplayIntent::RecallFleet { fleet_id: f.id }),
                        focus_x,
                        focus_y,
                    });
                }
                for a in snap.attacks.iter().filter(|a| a.target_owner == my_pid) {
                    let name = snap
                        .players
                        .iter()
                        .find(|p| p.id == a.owner_id)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "Unknown".to_string());
                    rows.push(Row {
                        id: a.id,
                        dir: Dir::In,
                        troops: a.troops,
                        name,
                        retreating: a.retreating,
                        cancel: None,
                        focus_x: a.front_cx,
                        focus_y: a.front_cy,
                    });
                }
            }
        }
        if rows.is_empty() {
            return;
        }
        // Log order: newest (highest id) on top, oldest at the bottom.
        rows.sort_by(|a, b| b.id.cmp(&a.id));

        // ── Geometry ───────────────────────────────────────────────────────────────────────────
        let screen_rect = ctx.content_rect();
        let compact = sow_ui_kit::theme::compact_viewport(ctx);
        let bottom_rect = ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("hud_bottom_panel_rect")));
        let clearance = bottom_rect
            .map(|r| (screen_rect.max.y - r.min.y).max(0.0) + 12.0)
            .unwrap_or(if compact { 132.0 } else { 24.0 });

        const H_MARGIN: f32 = 14.0;
        let width = if compact {
            let btn_w = if cfg!(target_os = "android") { 46.0 } else { 30.0 };
            let rail_extent = 12.0 + (btn_w + 8.0);
            let gap = 8.0;
            (screen_rect.width() - 2.0 * (rail_extent + gap) - 2.0 * H_MARGIN).max(160.0)
        } else {
            bottom_rect
                .map(|r| r.width() - 2.0 * H_MARGIN)
                .unwrap_or(520.0 - 2.0 * H_MARGIN)
        };

        // 2-column grid of single-line cells; more than MAX_ROWS rows of pairs scrolls.
        let gap = 8.0_f32;
        let cols = if compact { 1 } else { 2 };
        let col_w = (width - gap * (cols - 1) as f32) / cols as f32;
        let cell_h = if compact { 28.0 } else { 30.0 };
        let row_gap = 6.0_f32;
        const MAX_ROWS: usize = 3;
        let n_rows = rows.len().div_ceil(cols);
        let visible = n_rows.min(MAX_ROWS);
        let view_h = visible as f32 * cell_h + visible.saturating_sub(1) as f32 * row_gap;

        let green = [0.34_f32, 0.92, 0.48, 1.0]; // outgoing text
        let red = [1.0_f32, 0.42, 0.42, 1.0]; // incoming text
        let cyan = [0.32_f32, 0.85, 1.0, 1.0]; // navy text

        egui::Area::new("attacks_panel".into())
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -clearance))
            .show(ctx, |ui| {
                let prepaint_idx = ui.painter().add(egui::Shape::Noop);
                let frame_res = egui::Frame::NONE
                    .inner_margin(egui::Margin::symmetric(H_MARGIN as i8, 8))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.spacing_mut().item_spacing.y = row_gap;
                        egui::ScrollArea::vertical()
                            .max_height(view_h)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                for pair in rows.chunks(cols) {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 0.0;
                                        for (i, row) in pair.iter().enumerate() {
                                            if i > 0 {
                                                ui.add_space(gap);
                                            }
                                            let (color, up) = match row.dir {
                                                Dir::Out => (green, true),
                                                Dir::In => (red, false),
                                                Dir::Navy => (cyan, true),
                                            };
                                            let (rect, response) = ui.allocate_exact_size(
                                                egui::vec2(col_w, cell_h),
                                                egui::Sense::click(),
                                            );

                                            // Draw themed rounded card background for this cell
                                            let bg_color = sow_ui_kit::theme::palette::field_bg();
                                            let border_color = sow_ui_kit::theme::palette::field_border();
                                            let cell_radius = sow_ui_kit::theme::radius::sm();
                                            ui.painter().rect(
                                                rect,
                                                cell_radius,
                                                bg_color,
                                                egui::Stroke::new(1.0, border_color),
                                                egui::StrokeKind::Inside,
                                            );

                                            // 1. Direction marker — an egui triangle
                                            let ax = rect.left() + 14.0;
                                            let ay = rect.center().y;
                                            let s = 5.0;
                                            let pts = if up {
                                                vec![egui::pos2(ax, ay - s), egui::pos2(ax - s, ay + s), egui::pos2(ax + s, ay + s)]
                                            } else {
                                                vec![egui::pos2(ax, ay + s), egui::pos2(ax - s, ay - s), egui::pos2(ax + s, ay - s)]
                                            };
                                            let tcol = egui::Color32::from_rgb(
                                                (color[0] * 255.0) as u8,
                                                (color[1] * 255.0) as u8,
                                                (color[2] * 255.0) as u8,
                                            );
                                            ui.painter().add(egui::Shape::convex_polygon(
                                                pts,
                                                tcol,
                                                egui::Stroke::new(1.0, egui::Color32::from_black_alpha(160)),
                                            ));

                                            // 2. Troops & name text (egui native - renders on top)
                                            let text_color = egui::Color32::from_rgb(
                                                (color[0] * 255.0) as u8,
                                                (color[1] * 255.0) as u8,
                                                (color[2] * 255.0) as u8,
                                            );
                                            let name: String = row.name.chars().take(12).collect();
                                            let mut line = format!("{}  {}", fmt_count(row.troops), name);
                                            if row.retreating {
                                                line.push_str("  (ret)");
                                            }
                                            let font_id = egui::FontId::proportional(13.0);
                                            sow_ui_kit::widgets::paint_emoji_text_at(
                                                ui.painter(),
                                                egui::pos2(rect.left() + 26.0, rect.center().y - 1.0),
                                                egui::Align2::LEFT_CENTER,
                                                &line,
                                                font_id,
                                                text_color,
                                                false,
                                            );

                                            let mut cancel_clicked = false;
                                            // 3. Cancel button
                                            if let Some(intent) = &row.cancel {
                                                let btn_size = cell_h - 10.0;
                                                let btn_rect = egui::Rect::from_center_size(
                                                    egui::pos2(rect.right() - btn_size / 2.0 - 8.0, rect.center().y),
                                                    egui::vec2(btn_size, btn_size),
                                                );

                                                let btn_id = ui.make_persistent_id(row.id);
                                                let btn_resp = ui.interact(btn_rect, btn_id, egui::Sense::click());
                                                let hot = btn_resp.hovered();
                                                let active = btn_resp.is_pointer_button_down_on();

                                                let bg = if active {
                                                    egui::Color32::from_rgba_unmultiplied(120, 24, 28, 160)
                                                } else if hot {
                                                    egui::Color32::from_rgba_unmultiplied(90, 18, 22, 120)
                                                } else {
                                                    egui::Color32::from_rgba_unmultiplied(60, 12, 14, 80)
                                                };
                                                let border = egui::Color32::from_rgb(180, 50, 55);

                                                ui.painter().rect(
                                                    btn_rect,
                                                    sow_ui_kit::theme::radius::SM,
                                                    bg,
                                                    egui::Stroke::new(1.0, border),
                                                    egui::StrokeKind::Inside,
                                                );

                                                let emoji_size = btn_size * 0.65;
                                                let emoji_rect = egui::Rect::from_center_size(btn_rect.center(), egui::vec2(emoji_size, emoji_size));
                                                sow_ui_kit::widgets::try_paint_emoji(
                                                    ui.painter(),
                                                    "❌",
                                                    emoji_rect,
                                                    egui::Color32::WHITE,
                                                );

                                                if hot {
                                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                                }
                                                if btn_resp.clicked() {
                                                    local_cancel_intents.push(intent.clone());
                                                    cancel_clicked = true;
                                                }
                                            }

                                            if response.clicked() && !cancel_clicked {
                                                if row.focus_x != 0.0 || row.focus_y != 0.0 {
                                                    self.input.camera_focus_target = Some((row.focus_x, row.focus_y));
                                                    self.input.target_zoom = 10.0;
                                                }
                                            }
                                        }
                                    });
                                }
                            });
                    });

                // Themed backdrop: uses proper HUD panel gradient/glow logic
                let bg = frame_res.response.rect;
                let radius = if compact {
                    egui::CornerRadius::ZERO
                } else {
                    sow_ui_kit::theme::radius::lg()
                };
                sow_ui_kit::theme::paint_hud_panel_gradient(
                    ui,
                    prepaint_idx,
                    bg,
                    sow_ui_kit::theme::palette::field_border(),
                    radius,
                );
            });
    }

    pub(crate) fn render_stats_overlay(&mut self, _ctx: &egui::Context) {
        if let Some(ref mut tr) = self.gfx.text_renderer {
            let mut stats = String::new();
            if let Some(ping) = self.net.current_ping_ms {
                stats.push_str(&format!("{ping}ms · {} fps", self.time.current_fps));
            } else {
                stats.push_str(&format!("{} fps", self.time.current_fps));
            }
            stats.push_str(&format!(" · {:.2}x", self.input.camera_zoom));

            // Render stats overlay in the bottom right corner using the GPU TextRenderer
            let right_inset = 12.0;
            let bottom_inset = 12.0;
            let font_size = 11.0;
            let x = self.input.screen_w - right_inset;
            let y = self.input.screen_h - bottom_inset;

            tr.push_string(
                &stats,
                [x, y],
                font_size,
                [1.0, 1.0, 1.0, 1.0], // color white
                [0.0, 0.0, 0.0, 1.0], // outline color black
                sow_render::TmpFontSettings::default(),
                1.0, // align right (align_x = 1.0)
                1.0, // char_spacing
                1.0, // emoji_scale
            );
        }
    }
}
