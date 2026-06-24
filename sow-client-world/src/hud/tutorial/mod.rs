//! Campaign tutorial. Split into focused modules so the campaign can grow without any one
//! file blowing past the size guard:
//! - [`steps`]      — the ordered objective table + trigger evaluation (the *content*).
//! - [`objectives`] — the detachable Objectives panel + its row model (the *tracker*).
//! - [`pointer`]    — the screen-space "here" attention pointer.
//!
//! `mod.rs` is the orchestrator: it reads the live snapshot, advances the step pointer,
//! drives the speaker dialog, and feeds the panel + pointer. Adding a chapter is mostly
//! data in [`steps`]; adding a mechanic is one `Trigger` variant + its arm.

mod objectives;
mod pointer;
mod steps;

use crate::app::SowApp;
use objectives::{draw_objectives_panel, ObjRow, ObjState};
use pointer::draw_tutorial_pointer;
use sow_ui::widgets::{DialogButton, SpeakerDialog, SpeakerVisual, ThemeButtonStyle};
use steps::{objective_progress, ADVISOR, CHAPTER_1};

/// Kept for the `UiState.tutorial_step` field + `start_offline_match`; the campaign
/// interpreter below drives flow via `tutorial_step_idx`.
#[derive(Debug, Clone, PartialEq)]
pub enum TutorialStep {
    Welcome,
    Expansion,
    Combat,
    Complete,
}

impl SowApp {
    pub(crate) fn render_tutorial_ui(&mut self, ctx: &egui::Context) {
        if !self.ui.tutorial_active {
            return;
        }

        // The advisor portrait persists from the main menu, but guarantee it offline too.
        self.ui.app.asset_loader.ensure_avatars_loaded(ctx);

        let steps = CHAPTER_1;

        // Snapshot facts about the local player.
        let my_id = self.sim.my_player_id.unwrap_or(1);
        let me = self
            .sim
            .current_snapshot
            .as_ref()
            .and_then(|s| s.players.iter().find(|p| p.id == my_id))
            .map(|p| (p.tile_count, p.has_spawned, p.centroid_x, p.centroid_y, p.kills));
        let my_tiles = me.map(|m| m.0).unwrap_or(0);
        let my_kills = me.map(|m| m.4).unwrap_or(0);
        let spawned = matches!(me, Some((_, true, ..)));

        // Capture the spawn tile count ONCE. Objectives track tiles gained since spawn, in
        // real time, regardless of which dialog step is showing — the dialog never gates them.
        if !self.ui.tutorial_baseline_set && spawned && my_tiles > 0 {
            self.ui.tutorial_baseline_tiles = my_tiles;
            self.ui.tutorial_baseline_set = true;
            log::info!("tutorial: spawn baseline = {} tiles", my_tiles);
        }
        let gained = if self.ui.tutorial_baseline_set {
            my_tiles.saturating_sub(self.ui.tutorial_baseline_tiles)
        } else {
            0
        };

        // Reactive emotes: each new conquest makes the nearby world react — allies cheer, Rome
        // rages, independents recoil. (`me` is a Copy tuple, so no borrow is held here.)
        if my_kills > self.ui.tutorial_last_kills {
            self.ui.tutorial_last_kills = my_kills;
            self.tutorial_react_to_conquest();
        }

        // Completing the current objective opens the NEXT modal — that's the only thing that
        // advances a step. The tutorial NEVER ends here; the last step just stays satisfied
        // until more objectives are appended. (Camera follows progress; nothing blocks play.)
        let mut idx = self.ui.tutorial_step_idx.min(steps.len() - 1);
        while idx + 1 < steps.len() {
            let (cur, tgt) = objective_progress(steps[idx].advance, gained, my_kills);
            if cur < tgt {
                break;
            }
            idx += 1;
        }
        let mut step_changed = false;
        if idx != self.ui.tutorial_step_idx {
            self.ui.tutorial_step_idx = idx;
            self.ui.tutorial_modal_dismissed = false; // open the new step's modal
            step_changed = true;
            log::info!(
                "tutorial: objective complete -> step {} '{}' (gained={})",
                idx,
                steps[idx].title,
                gained
            );
        }
        let step = &steps[idx];

        // Objectives panel: completed objectives + the current one. A completed objective
        // lingers HOLD seconds, then fades out and is removed (cleanup).
        const HOLD: f64 = 3.0;
        const FADE: f64 = 0.5;
        let now = ctx.input(|i| i.time);
        // Stamp each newly-completed step's finish time once.
        for j in 0..idx {
            self.ui.tutorial_obj_done_at.entry(j).or_insert(now);
        }
        let mut want_repaint = false;
        let mut rows: Vec<ObjRow> = Vec::new();
        for (i, s) in steps.iter().take(idx + 1).enumerate() {
            let (current, target) = objective_progress(s.advance, gained, my_kills);
            if i < idx {
                // Completed: hold, then fade out, then drop.
                let done_at = self.ui.tutorial_obj_done_at.get(&i).copied().unwrap_or(now);
                let elapsed = now - done_at;
                if elapsed > HOLD + FADE {
                    continue; // cleaned up
                }
                want_repaint = true;
                let fade = if elapsed < HOLD {
                    1.0
                } else {
                    (1.0 - (elapsed - HOLD) / FADE) as f32
                };
                rows.push(ObjRow {
                    label: s.title,
                    current,
                    target,
                    state: ObjState::Done,
                    fade,
                });
            } else {
                rows.push(ObjRow {
                    label: s.title,
                    current,
                    target,
                    state: if current >= target {
                        ObjState::Done
                    } else {
                        ObjState::Active
                    },
                    fade: 1.0,
                });
            }
        }
        if want_repaint {
            ctx.request_repaint(); // drive the cleanup fade
        }
        let toggle = draw_objectives_panel(ctx, &rows, self.ui.tutorial_objectives_open);
        if toggle {
            self.ui.tutorial_objectives_open = !self.ui.tutorial_objectives_open;
        }

        // Pointer ("here") always guides to the player's territory — even with the modal
        // closed — so direction is clear while they complete the objective.
        if let Some((_, true, ..)) = me {
            // Smoothed nameplate anchor (same point the avatar is drawn at) → concentric ring.
            let (wx, wy) = self
                .ui
                .label_positions
                .get(&my_id)
                .copied()
                .or_else(|| me.map(|(_, _, cx, cy, _)| (cx, cy)))
                .unwrap_or((0.0, 0.0));
            let sf = ctx.pixels_per_point();
            let sx = (wx * self.input.camera_zoom + self.input.camera_x) / sf;
            let sy = (wy * self.input.camera_zoom + self.input.camera_y) / sf;
            draw_tutorial_pointer(ctx, egui::pos2(sx, sy));
        }

        // First-contact intros: when our territory first reaches a tribe with a written line,
        // queue it. It takes over the modal until dismissed, then play continues.
        if self.ui.tutorial_pending_intro.is_none() {
            self.ui.tutorial_pending_intro = self.tutorial_detect_first_contact();
        }

        // A pending intro takes over the modal; otherwise the objective modal shows. Both reuse
        // the same speaker dialog. ANY click anywhere closes whichever is up.
        if let Some(name) = self.ui.tutorial_pending_intro.clone() {
            // The NPC presents itself, with its real portrait: tribe → spirit-animal emoji on a
            // colored disc, empire → colored disc, all from the live snapshot.
            let line = crate::campaign::dialog::first_contact(&name);
            let visual = self
                .sim
                .current_snapshot
                .as_ref()
                .and_then(|s| s.players.iter().find(|p| p.name == name))
                .map(|p| match p.player_type {
                    sow_core::player::PlayerType::Bot => SpeakerVisual::Tribe {
                        color: p.color,
                        emoji: sow_core::player::tribe_animal(p.id, &p.name).to_string(),
                    },
                    sow_core::player::PlayerType::Nation => SpeakerVisual::Empire { color: p.color },
                    sow_core::player::PlayerType::Human => SpeakerVisual::Avatar(ADVISOR),
                });
            let dialog = SpeakerDialog {
                visual,
                name: Some(&line.speaker),
                title: &line.title,
                body: &line.body,
                buttons: vec![DialogButton::new("Got it", ThemeButtonStyle::Primary)],
            };
            let clicked = sow_ui::widgets::draw_speaker_dialog(
                ctx,
                "tutorial_intro",
                &dialog,
                &self.ui.app.asset_loader,
                true,
            );
            if clicked.is_some() || ctx.input(|i| i.pointer.any_click()) {
                self.ui.tutorial_pending_intro = None;
            }
        } else if !self.ui.tutorial_modal_dismissed {
            // The objective modal. ANY click closes it (tapping the map to grow dismisses too).
            // Skip the very frame the step changed to avoid a flash.
            let dialog = SpeakerDialog {
                visual: Some(SpeakerVisual::Avatar(ADVISOR)),
                name: Some("Boudica"),
                title: step.title,
                body: step.body,
                buttons: vec![DialogButton::new("Got it", ThemeButtonStyle::Primary)],
            };
            let clicked = sow_ui::widgets::draw_speaker_dialog(
                ctx,
                "tutorial_dialog",
                &dialog,
                &self.ui.app.asset_loader,
                true, // whole panel closes it
            );
            let clicked_anywhere = ctx.input(|i| i.pointer.any_click());
            if clicked.is_some() || (clicked_anywhere && !step_changed) {
                self.ui.tutorial_modal_dismissed = true;
                log::info!("tutorial: modal closed on step {} '{}'", idx, step.title);
            }
        }

        // The opening dialog freezes the world (nobody moves) so the player can read it; the
        // moment they dismiss it, play resumes. Only the very first step pauses.
        self.sim.paused = idx == 0 && !self.ui.tutorial_modal_dismissed;
    }

    /// Returns a tribe whose first-contact intro should fire now — i.e. our territory has just
    /// reached an un-met faction that has a written intro. "Contact" = we own a tile in a small
    /// box around the tribe's centroid (cheap; bounded to the few factions with intros).
    fn tutorial_detect_first_contact(&mut self) -> Option<String> {
        let my_id = self.sim.my_player_id.unwrap_or(1);
        // Candidates from the snapshot (owned, so no borrow is held past this block).
        let candidates: Vec<(u16, String, f32, f32)> = {
            let snap = self.sim.current_snapshot.as_ref()?;
            snap.players
                .iter()
                .filter(|p| {
                    // Every named NPC greets on first border contact (fires once each).
                    p.id != my_id
                        && p.alive
                        && p.tile_count > 0
                        && !self.ui.tutorial_met_tribes.contains(&p.id)
                })
                .map(|p| (p.id, p.name.clone(), p.centroid_x, p.centroid_y))
                .collect()
        };
        if candidates.is_empty() {
            return None;
        }
        // First we own a tile within R of a tribe's centroid = contact.
        const R: i32 = 7;
        let hit = {
            let engine = self.sim.engine.as_ref()?;
            let map = &engine.state.map;
            candidates.into_iter().find(|&(_, _, cx, cy)| {
                let (tx, ty) = (cx as i32, cy as i32);
                (-R..=R).any(|dy| {
                    (-R..=R).any(|dx| {
                        map.is_valid_coord(tx + dx, ty + dy)
                            && map.owner_id((tx + dx) as u32, (ty + dy) as u32) == my_id
                    })
                })
            })
        };
        let (id, name, ..) = hit?;
        self.ui.tutorial_met_tribes.insert(id);
        log::info!("tutorial: first contact with '{}'", name);
        Some(name)
    }

    /// On a fresh conquest, make the *nearby* world react, differentiated by allegiance:
    /// teammates cheer, Rome's bloc rages, independents recoil. Offline-only (we own the engine);
    /// the emotes flow into the next snapshot and render with the existing float/spring animation,
    /// auto-expiring via the engine's emoji timer. Reusable for any future scripted reaction.
    fn tutorial_react_to_conquest(&mut self) {
        use sow_core::protocol::Team;
        let my_id = self.sim.my_player_id.unwrap_or(1);
        let Some(engine) = self.sim.engine.as_mut() else {
            return;
        };
        let my_team = engine.state.player(my_id).and_then(|p| p.team);
        let Some((mcx, mcy)) = engine
            .state
            .player(my_id)
            .filter(|p| p.tile_count > 0)
            .map(|p| {
                (
                    (p.sum_x / p.tile_count as u64) as f32,
                    (p.sum_y / p.tile_count as u64) as f32,
                )
            })
        else {
            return;
        };
        let ticks = (3.0 * 1000.0 / engine.state.config.tick_rate_ms).round() as u32;
        const REACT_RADIUS_SQ: f32 = 170.0 * 170.0;
        for p in engine.state.players.iter_mut() {
            if p.id == my_id || !p.alive || p.tile_count == 0 {
                continue;
            }
            let cx = (p.sum_x / p.tile_count as u64) as f32;
            let cy = (p.sum_y / p.tile_count as u64) as f32;
            if (cx - mcx).powi(2) + (cy - mcy).powi(2) > REACT_RADIUS_SQ {
                continue;
            }
            // Allegiance → reaction. All emoji are atlas-confirmed (sow-ui emote palette).
            let emoji = if p.team.is_some() && p.team == my_team {
                "🎉" // a teammate cheers the conquest
            } else if p.team == Some(Team::Blue) {
                "😡" // Rome's bloc rages
            } else {
                "😮" // an independent clan recoils
            };
            p.active_emoji = Some(emoji.to_string());
            p.emoji_timer = ticks;
            p.emoji_pinned = false;
        }
    }
}
