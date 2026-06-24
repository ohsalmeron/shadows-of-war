//! Themed, non-blocking "speaker dialog" — portrait on the left, title + body, themed buttons.
//!
//! Mobile-style bottom sheet used to funnel players into mechanics (tutorial today, single-player
//! campaign later). Non-blocking on purpose: it draws no scrim so the map stays interactive
//! underneath (the tutorial's expand/attack steps need live clicks).

use crate::ui::asset_loader::AssetLoader;
use crate::ui::theme;
use crate::widgets::{ThemeButton, ThemeButtonStyle};
use egui::{Align, Color32, Context, Layout, RichText, Vec2};
use sow_core::player::Leader;

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
    let portrait = if compact { 44.0 } else { 52.0 };

    let mut clicked: Option<usize> = None;

    egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground) // above world nameplates
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -clearance))
        .show(ctx, |ui| {
            let frame_resp = theme::standard_panel_frame(compact).show(ui, |ui| {
                ui.set_width(width);
                ui.horizontal_top(|ui| {
                    match &dialog.visual {
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
                            let (rect, _) = ui
                                .allocate_exact_size(Vec2::splat(portrait), egui::Sense::hover());
                            paint_speaker_disc(ui.painter(), rect, *color);
                            let er = egui::Rect::from_center_size(
                                rect.center(),
                                Vec2::splat(portrait * 0.62),
                            );
                            crate::widgets::try_paint_emoji(ui.painter(), emoji, er, Color32::WHITE);
                            ui.add_space(10.0);
                        }
                        Some(SpeakerVisual::Empire { color }) => {
                            let (rect, _) = ui
                                .allocate_exact_size(Vec2::splat(portrait), egui::Sense::hover());
                            paint_speaker_disc(ui.painter(), rect, *color);
                            ui.add_space(10.0);
                        }
                        None => {}
                    }

                    ui.vertical(|ui| {
                        if let Some(name) = dialog.name {
                            ui.label(
                                RichText::new(name.to_uppercase())
                                    .size(11.0)
                                    .strong()
                                    .color(theme::palette::neon_cyan()),
                            );
                        }
                        ui.label(
                            RichText::new(dialog.title)
                                .size(if compact { 16.0 } else { 18.0 })
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(dialog.body)
                                .size(if compact { 13.0 } else { 14.0 })
                                .color(theme::palette::text_muted()),
                        );

                        ui.add_space(8.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            for (i, btn) in dialog.buttons.iter().enumerate() {
                                let resp = ui.add(
                                    ThemeButton::new(&btn.label)
                                        .style(btn.style)
                                        .text_size(if compact { 13.0 } else { 14.0 })
                                        .min_size(egui::vec2(
                                            88.0,
                                            if compact { 32.0 } else { 34.0 },
                                        )),
                                );
                                if resp.clicked() {
                                    clicked = Some(i);
                                }
                            }
                        });
                    });
                });
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
