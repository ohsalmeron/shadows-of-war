use crate::app::SowApp;
use sow_core::protocol::Turn;

impl SowApp {
    pub(crate) fn handle_sim_turn(&mut self, turn: Turn) {
        let (mut snap, events) = {
            let Some(e) = self.sim.engine.as_mut() else {
                return;
            };
            e.apply_intents(&turn.intents);
            e.tick();
            let snap = e.build_snapshot();
            let events: Vec<_> = e.state.events.drain(..).collect();
            (snap, events)
        };

        let my_id = self.sim.my_player_id.unwrap_or(0);
        let now_instant = web_time::Instant::now();
        let turn_defeats = self.process_tick_events(events, &snap, my_id, now_instant);

        self.progress_session_defeats.players = self
            .progress_session_defeats
            .players
            .saturating_add(turn_defeats.players);
        self.progress_session_defeats.empires = self
            .progress_session_defeats
            .empires
            .saturating_add(turn_defeats.empires);
        self.progress_session_defeats.tribes = self
            .progress_session_defeats
            .tribes
            .saturating_add(turn_defeats.tribes);

        self.apply_snapshot_fx(&mut snap, my_id);
        self.process_nuke_alerts(&snap);

        let my_team = snap
            .players
            .iter()
            .find(|p| p.id == my_id)
            .and_then(|p| p.team);
        self.maybe_submit_online_stats(&snap);
        self.maybe_record_match_progress(&snap, snap.winner, snap.winning_team, my_team);

        // Viewport Alerts: Victory / Defeat
        if let Some(winner) = snap.winner {
            if winner == my_id {
                self.ui
                    .trigger_viewport_alert(crate::app::ViewportAlertKind::Victory);
            } else {
                self.ui
                    .trigger_viewport_alert(crate::app::ViewportAlertKind::Defeat);
            }
        } else if let Some(w_team) = snap.winning_team {
            if Some(w_team) == my_team {
                self.ui
                    .trigger_viewport_alert(crate::app::ViewportAlertKind::Victory);
            } else {
                self.ui
                    .trigger_viewport_alert(crate::app::ViewportAlertKind::Defeat);
            }
        }

        self.sim.current_snapshot = Some(snap);

        // Recompute Fog of War visibility
        if let Some(ref snap_ref) = self.sim.current_snapshot {
            let owners = self
                .gfx
                .map_renderer
                .as_ref()
                .map(|mr| mr.owners.as_slice())
                .unwrap_or(&[]);
            let terrain = self
                .gfx
                .map_renderer
                .as_ref()
                .map(|mr| mr.terrain.as_slice())
                .unwrap_or(&[]);
            self.sim
                .fog_explored
                .blocks
                .resize((self.sim.map_w * self.sim.map_h + 63) as usize / 64, 0);
            self.sim
                .fog_visible
                .blocks
                .resize((self.sim.map_w * self.sim.map_h + 63) as usize / 64, 0);
            let dev = sow_ui_kit::theme::dev_config::DevConfig::get();
            crate::sim::visibility::compute_visibility(
                self.sim.map_w,
                self.sim.map_h,
                my_id,
                owners,
                snap_ref,
                terrain,
                &mut self.sim.fog_explored,
                &mut self.sim.fog_visible,
                dev.fog_of_war,
            );
            self.sim.force_fog_upload = true;
        }

        self.time.interp.stamp_applied(web_time::Instant::now());
    }
}
