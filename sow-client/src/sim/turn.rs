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
        self.maybe_record_match_progress(snap.winner, snap.winning_team, my_team);
        self.sim.current_snapshot = Some(snap);
        self.time.interp.stamp_applied(web_time::Instant::now());
    }
}
