//! Full-screen leader hero backdrop — on-demand load, book-style page turns between leaders.

use crate::ui::animation::{
    leader_page_in_alpha, leader_page_out_alpha, leader_page_turn_t, LEADER_PAGE_FADE_IN_START,
    LEADER_PAGE_LOADING_MIN, LEADER_PAGE_TURN_DURATION,
};
use crate::ui::asset_loader::AssetLoader;
use crate::widgets::avatar_picker::{
    draw_leader_picker_overlay_gradient, leader_background_cover_uv,
};
use egui::{Color32, Rect, Ui};
use sow_data::Leader;
use web_time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackdropPhase {
    Steady,
    /// Outgoing page slides away; incoming page (or loading sheet) slides in.
    Turning,
    /// Turn finished but portrait bytes not ready — spinner on the new page.
    Loading,
}

pub struct LeaderBackdropTransition {
    pub displayed_leader: Leader,
    pub anim_leader: Leader,
    pub started_at: Instant,
    pub phase: BackdropPhase,
    /// `true` = next page enters from the right (forward in the roster).
    pub turn_forward: bool,
    pub loading_hold_from: Option<Instant>,
}

impl LeaderBackdropTransition {
    pub fn new(initial: Leader) -> Self {
        Self {
            displayed_leader: initial,
            anim_leader: initial,
            started_at: Instant::now(),
            phase: BackdropPhase::Steady,
            turn_forward: true,
            loading_hold_from: None,
        }
    }

    fn begin_transition(&mut self, leader: Leader) {
        self.turn_forward = leader_page_index(leader) >= leader_page_index(self.displayed_leader);
        self.anim_leader = leader;
        self.started_at = Instant::now();
        self.loading_hold_from = None;
        self.phase = BackdropPhase::Turning;
    }

    fn settle_to_steady(&mut self) {
        self.displayed_leader = self.anim_leader;
        self.phase = BackdropPhase::Steady;
        self.loading_hold_from = None;
    }
}

fn leader_page_index(leader: Leader) -> usize {
    Leader::ALL.iter().position(|&l| l == leader).unwrap_or(0)
}

fn paint_portrait_clipped(
    ui: &Ui,
    clip: Rect,
    rect: Rect,
    tex: &egui::TextureHandle,
    mobile: bool,
    opacity: f32,
) {
    if opacity <= 0.001 || !clip.intersects(rect) {
        return;
    }
    let uv = leader_background_cover_uv(rect.size(), tex.size_vec2(), mobile);
    let tint = Color32::from_white_alpha((255.0 * opacity.clamp(0.0, 1.0)) as u8);
    ui.painter()
        .with_clip_rect(clip)
        .image(tex.id(), rect, uv, tint);
}

fn draw_loading_page(ui: &mut Ui, page_rect: Rect, label: &str, opacity: f32) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= 0.001 {
        return;
    }
    ui.painter().rect_filled(
        page_rect,
        0.0,
        Color32::from_rgba_unmultiplied(12, 14, 22, (250.0 * opacity) as u8),
    );
    let center = page_rect.center();
    let spinner_rect =
        Rect::from_center_size(center - egui::vec2(0.0, 12.0), egui::vec2(30.0, 30.0));
    egui::Area::new(egui::Id::new("backdrop_loading_spinner"))
        .fixed_pos(spinner_rect.min)
        .interactable(false)
        .show(ui.ctx(), |ui| {
            ui.spinner();
        });
    ui.painter().with_clip_rect(page_rect).text(
        center + egui::vec2(0.0, 22.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(14.0),
        Color32::from_rgba_unmultiplied(220, 230, 245, (230.0 * opacity) as u8),
    );
}

/// Spine shadow between pages during the turn (book gutter).
fn paint_page_spine(ui: &Ui, screen_rect: Rect, turn_t: f32, forward: bool) {
    let w = screen_rect.width();
    let spine_x = if forward {
        screen_rect.min.x + w * turn_t
    } else {
        screen_rect.max.x - w * turn_t
    };
    let spine_w = 14.0_f32;
    let spine_rect = Rect::from_min_max(
        egui::pos2(spine_x - spine_w * 0.5, screen_rect.min.y),
        egui::pos2(spine_x + spine_w * 0.5, screen_rect.max.y),
    );
    ui.painter().rect_filled(
        spine_rect,
        0.0,
        Color32::from_black_alpha((55.0 + 40.0 * (1.0 - turn_t)) as u8),
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_page_turn(
    ui: &mut Ui,
    screen_rect: Rect,
    mobile: bool,
    asset_loader: &AssetLoader,
    transition: &LeaderBackdropTransition,
    turn_t: f32,
    incoming_tex: Option<&egui::TextureHandle>,
    loading_label: &str,
) {
    let w = screen_rect.width();
    let forward = transition.turn_forward;
    let out_sign = if forward { -1.0 } else { 1.0 };
    let in_sign = if forward { 1.0 } else { -1.0 };

    let out_offset = out_sign * turn_t * w;
    let in_offset = in_sign * (1.0 - turn_t) * w;

    let out_rect = screen_rect.translate(egui::vec2(out_offset, 0.0));
    let in_rect = screen_rect.translate(egui::vec2(in_offset, 0.0));

    paint_page_spine(ui, screen_rect, turn_t, forward);

    let out_alpha = leader_page_out_alpha(turn_t);
    let in_alpha = leader_page_in_alpha(turn_t);

    if let Some(tex) = asset_loader.leader_portrait_texture(transition.displayed_leader, mobile) {
        paint_portrait_clipped(ui, screen_rect, out_rect, tex, mobile, out_alpha);
    }

    if let Some(tex) = incoming_tex {
        paint_portrait_clipped(ui, screen_rect, in_rect, tex, mobile, in_alpha);
    } else if in_alpha > 0.001 {
        ui.painter().with_clip_rect(screen_rect).rect_filled(
            in_rect,
            0.0,
            Color32::from_rgba_unmultiplied(12, 14, 22, (250.0 * in_alpha) as u8),
        );
        if turn_t >= LEADER_PAGE_FADE_IN_START {
            draw_loading_page(ui, in_rect, loading_label, in_alpha);
        }
    }
}

fn advance_phase(transition: &mut LeaderBackdropTransition, target_ready: bool, now: Instant) {
    let elapsed = now.duration_since(transition.started_at).as_secs_f32();

    match transition.phase {
        BackdropPhase::Steady => {
            if !target_ready {
                transition.phase = BackdropPhase::Loading;
                transition.loading_hold_from = Some(now);
            }
        }
        BackdropPhase::Turning => {
            if elapsed >= LEADER_PAGE_TURN_DURATION {
                if target_ready {
                    transition.settle_to_steady();
                } else {
                    transition.phase = BackdropPhase::Loading;
                    transition.loading_hold_from = Some(now);
                }
            }
        }
        BackdropPhase::Loading => {
            if target_ready {
                let hold = transition.loading_hold_from.unwrap_or(now);
                if now.duration_since(hold).as_secs_f32() >= LEADER_PAGE_LOADING_MIN {
                    transition.settle_to_steady();
                }
            }
        }
    }
}

/// Draw hero backdrop; returns true when `selected` changed this frame.
#[allow(clippy::too_many_arguments)]
pub fn draw_leader_hero_backdrop(
    ui: &mut Ui,
    screen_rect: Rect,
    selected: Leader,
    mobile: bool,
    asset_loader: &mut AssetLoader,
    transition: &mut LeaderBackdropTransition,
    loading_label: &str,
    draw_picker_gradient: bool,
) -> bool {
    asset_loader.set_leader_portrait_focus(selected, mobile);

    let reduced_motion = crate::ui::theme::anim_duration_from_ctx(ui.ctx()) <= 0.02;
    let now = Instant::now();

    let selection_changed = selected != transition.anim_leader;

    if selection_changed {
        transition.begin_transition(selected);
    } else if transition.phase == BackdropPhase::Steady
        && selected == transition.anim_leader
        && transition.displayed_leader != selected
    {
        transition.displayed_leader = selected;
    }

    let incoming_tex = asset_loader.leader_portrait_texture(transition.anim_leader, mobile);
    let incoming_ready = incoming_tex.is_some();
    advance_phase(transition, incoming_ready, now);

    ui.painter().rect_filled(
        screen_rect,
        0.0,
        Color32::from_rgba_unmultiplied(8, 8, 12, 255),
    );

    if reduced_motion {
        if let Some(tex) = incoming_tex {
            paint_portrait_clipped(ui, screen_rect, screen_rect, tex, mobile, 1.0);
            transition.settle_to_steady();
        } else {
            draw_loading_page(ui, screen_rect, loading_label, 1.0);
        }
        if draw_picker_gradient {
            draw_leader_picker_overlay_gradient(ui.painter(), screen_rect, mobile);
        }
        if transition.phase != BackdropPhase::Steady {
            ui.ctx().request_repaint();
        }
        return selection_changed;
    }

    match transition.phase {
        BackdropPhase::Turning => {
            let elapsed = now.duration_since(transition.started_at).as_secs_f32();
            let turn_t = leader_page_turn_t(elapsed);
            draw_page_turn(
                ui,
                screen_rect,
                mobile,
                asset_loader,
                transition,
                turn_t,
                incoming_tex,
                loading_label,
            );
        }
        BackdropPhase::Loading | BackdropPhase::Steady => {
            if let Some(tex) = incoming_tex {
                paint_portrait_clipped(ui, screen_rect, screen_rect, tex, mobile, 1.0);
            } else {
                draw_loading_page(ui, screen_rect, loading_label, 1.0);
            }
        }
    }

    if draw_picker_gradient {
        draw_leader_picker_overlay_gradient(ui.painter(), screen_rect, mobile);
    }

    if transition.phase != BackdropPhase::Steady {
        ui.ctx().request_repaint();
    }

    selection_changed
}
