//! Themed, non-blocking "speaker dialog" — portrait on the left, title + body, themed buttons.
//!
//! Mobile-style bottom sheet used to funnel players into mechanics (tutorial today, single-player
//! campaign later). Non-blocking on purpose: it draws no scrim so the map stays interactive
//! underneath (the tutorial's expand/attack steps need live clicks).

use crate::ui::asset_loader::AssetLoader;
use crate::ui::theme;
use crate::widgets::{ThemeButton, ThemeButtonStyle};
use egui::{Align, Color32, Context, Layout, Vec2};
use sow_core::player::Leader;

#[derive(Clone)]
pub struct DialogButton {
    pub label: String,
    pub style: ThemeButtonStyle,
}

impl DialogButton {
    pub fn new(label: impl Into<String>, style: ThemeButtonStyle) -> Self {
        Self {
            label: label.into(),
            style,
        }
    }
}

/// The speaker's portrait. Mirrors how the world nameplates draw each player type, so the modal
/// reads consistently: leaders get their avatar, tribes a colored disc + spirit-animal emoji,
/// empires a plain colored disc.
#[derive(Clone)]
pub enum SpeakerVisual {
    /// Human / named leader → avatar texture.
    Avatar(Leader),
    /// Tribe (bot) → colored disc + spirit-animal emoji (caller resolves the emoji).
    Tribe { color: [f32; 3], emoji: String },
    /// Empire (nation) → a solid colored disc.
    Empire { color: [f32; 3] },
}

pub struct SpeakerDialog<'a> {
    /// Portrait shown on the left; `None` draws no portrait.
    pub visual: Option<SpeakerVisual>,
    /// Small eyebrow name above the title (e.g. "Commander").
    pub name: Option<&'a str>,
    pub title: &'a str,
    pub body: &'a str,
    pub buttons: Vec<DialogButton>,
}

/// Owned twin of [`SpeakerDialog`], handed to the HUD so a message can *take over* the bottom
/// panel (reusing its frame) instead of floating its own sheet — see
/// [`crate::ui::hud`]'s bottom-panel takeover. Owned because it crosses the frame boundary: the
/// caller stashes it in `HudState` and the panel renders it next frame.
#[derive(Clone)]
pub struct BottomDialog {
    /// Stable id for the in/out animation (one animation slot per logical dialog).
    pub id: String,
    pub visual: Option<SpeakerVisual>,
    pub name: Option<String>,
    pub title: String,
    pub body: String,
    pub buttons: Vec<DialogButton>,
    /// Tapping anywhere on the panel acts as button 0 (only for non-destructive proceeds).
    pub click_anywhere: bool,
    /// Auto-fire button 0 after this many seconds on screen ("timed, with tap-to-skip").
    /// `None` = stays until the player acts (use for destructive / branching choices).
    pub auto_dismiss_secs: Option<f32>,
}

/// A solid colored disc with a dark rim — the portrait for tribes/empires (no avatar art).
fn paint_speaker_disc(painter: &egui::Painter, rect: egui::Rect, rgb: [f32; 3]) {
    let c = Color32::from_rgb(
        (rgb[0] * 255.0).round() as u8,
        (rgb[1] * 255.0).round() as u8,
        (rgb[2] * 255.0).round() as u8,
    );
    let center = rect.center();
    let r = rect.width().min(rect.height()) * 0.5;
    painter.circle_filled(center, r, c);
    painter.circle_stroke(
        center,
        r,
        egui::Stroke::new((r * 0.1).max(1.5), Color32::from_black_alpha(170)),
    );
}

/// Bottom edge clearance so the sheet sits above the HUD bottom controls. Reuses the same
/// `hud_bottom_panel_rect` temp key the HUD publishes each frame.
fn bottom_clearance(ctx: &Context, compact: bool) -> f32 {
    let screen = ctx.content_rect();
    ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("hud_bottom_panel_rect")))
        .map(|r| (screen.max.y - r.min.y).max(0.0) + 12.0)
        .unwrap_or(if compact { 132.0 } else { 24.0 })
}

/// Draw the dialog. Returns the index of the button clicked this frame, if any.
///
/// `click_anywhere`: when true (use it only when the sole button is a non-destructive
/// proceed like Next/Finish), clicking anywhere on the panel acts as button 0.
pub fn draw_speaker_dialog(
    ctx: &Context,
    id: &str,
    dialog: &SpeakerDialog,
    asset_loader: &AssetLoader,
    click_anywhere: bool,
) -> Option<usize> {
    let compact = theme::compact_viewport(ctx);
    let screen = ctx.content_rect();
    let clearance = bottom_clearance(ctx, compact);

    // Fixed width, height shrinks to content. Uses Area + Frame (not Window) because
    // egui Windows don't auto-shrink reliably here — every Window in this codebase sets
    // an explicit fixed_size. Area sizes exactly to its content.
    let width = (screen.width() - 24.0).min(if compact { 460.0 } else { 480.0 });

    let mut clicked: Option<usize> = None;

    egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground) // above world nameplates
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -clearance))
        .show(ctx, |ui| {
            let frame_resp = theme::standard_panel_frame(compact).show(ui, |ui| {
                ui.set_width(width);
                if let Some(i) = paint_dialog_contents(
                    ui,
                    dialog.visual.as_ref(),
                    dialog.name,
                    dialog.title,
                    dialog.body,
                    &dialog.buttons,
                    asset_loader,
                    compact,
                ) {
                    clicked = Some(i);
                }
            });

            // Whole-panel click acts as the primary button (only for non-destructive steps).
            if click_anywhere && clicked.is_none() {
                let r = ui.interact(
                    frame_resp.response.rect,
                    egui::Id::new((id, "panel_click")),
                    egui::Sense::click(),
                );
                if r.clicked() {
                    clicked = Some(0);
                }
                if r.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
        });

    clicked
}

/// The dialog's inner layout — portrait, eyebrow/title/body, themed buttons — with no frame,
/// area, or scrim of its own. Shared so the speaker sheet (floating) and the bottom-panel
/// takeover (docked) render byte-identical content. Returns the clicked button index, if any.
#[allow(clippy::too_many_arguments)]
pub fn paint_dialog_contents(
    ui: &mut egui::Ui,
    visual: Option<&SpeakerVisual>,
    name: Option<&str>,
    title: &str,
    body: &str,
    buttons: &[DialogButton],
    asset_loader: &AssetLoader,
    compact: bool,
) -> Option<usize> {
    let portrait = if compact { 44.0 } else { 52.0 };
    let mut clicked: Option<usize> = None;

    ui.horizontal_top(|ui| {
        match visual {
            Some(SpeakerVisual::Avatar(leader)) => {
                if let Some(tex) = asset_loader
                    .avatars
                    .get(leader)
                    .or(asset_loader.avatar_fallback.as_ref())
                {
                    ui.add(
                        egui::Image::new(tex)
                            .fit_to_exact_size(Vec2::splat(portrait))
                            .corner_radius(egui::CornerRadius::same(8)),
                    );
                    ui.add_space(10.0);
                }
            }
            Some(SpeakerVisual::Tribe { color, emoji }) => {
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::splat(portrait), egui::Sense::hover());
                paint_speaker_disc(ui.painter(), rect, *color);
                let er = egui::Rect::from_center_size(rect.center(), Vec2::splat(portrait * 0.62));
                crate::widgets::try_paint_emoji(ui.painter(), emoji, er, Color32::WHITE);
                ui.add_space(10.0);
            }
            Some(SpeakerVisual::Empire { color }) => {
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::splat(portrait), egui::Sense::hover());
                paint_speaker_disc(ui.painter(), rect, *color);
                ui.add_space(10.0);
            }
            None => {}
        }

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let wrap_w = ui.available_width().max(1.0);

            // Eyebrow, title, body all go through the emoji atlas text pipeline (the same one the
            // HUD uses) rather than egui labels — text runs render as glyphs, any emoji as atlas
            // images. The body is greedily word-wrapped to the column (the pipeline is single-line).
            if let Some(name) = name {
                crate::widgets::outlined_emoji_label(
                    ui,
                    &name.to_uppercase(),
                    egui::FontId::proportional(11.0),
                    theme::palette::neon_cyan(),
                );
            }
            crate::widgets::outlined_emoji_label(
                ui,
                title,
                egui::FontId::proportional(if compact { 16.0 } else { 18.0 }),
                Color32::WHITE,
            );
            ui.add_space(2.0);
            let body_font = egui::FontId::proportional(if compact { 13.0 } else { 14.0 });
            for line in wrap_emoji_lines(ui.painter(), body, &body_font, wrap_w) {
                crate::widgets::emoji_label(
                    ui,
                    &line,
                    body_font.clone(),
                    theme::palette::text_muted(),
                );
            }

            ui.add_space(8.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for (i, btn) in buttons.iter().enumerate() {
                    let resp = ui.add(
                        ThemeButton::new(&btn.label)
                            .style(btn.style)
                            .text_size(if compact { 13.0 } else { 14.0 })
                            .min_size(egui::vec2(88.0, if compact { 32.0 } else { 34.0 })),
                    );
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                }
            });
        });
    });

    clicked
}

/// Greedy word-wrap for the emoji-atlas text pipeline (which lays out a single line at a time):
/// returns the body split into lines that each fit `max_w`, measured with the same atlas-aware
/// metric used to paint them. A lone over-long word is kept on its own line rather than dropped.
fn wrap_emoji_lines(
    painter: &egui::Painter,
    text: &str,
    font: &egui::FontId,
    max_w: f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let trial = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if cur.is_empty() || crate::widgets::measure_emoji_text(painter, &trial, font).x <= max_w {
            cur = trial;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}
