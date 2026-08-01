use super::state::SowApp;

impl SowApp {
    pub(crate) fn reset_progress_session(&mut self) {
        self.progress_match_recorded = false;
        self.progress_stats_submitted = false;
        self.progress_session_defeats = crate::player_progress::SessionDefeats::default();
    }

    pub(crate) fn maybe_submit_online_stats(&mut self, snap: &sow_core::protocol::SimSnapshot) {
        if self.progress_stats_submitted {
            return;
        }
        if self.progress_account_id.is_none() || self.net.is_offline {
            return;
        }
        let my_id = self.sim.my_player_id.unwrap_or(0);
        if my_id == 0 {
            return;
        }
        let Some(me) = snap.players.iter().find(|p| p.id == my_id) else {
            return;
        };
        let game_over = snap.winner.is_some();
        let eliminated = !me.alive && me.has_spawned;
        if !game_over && !eliminated {
            return;
        }
        self.progress_stats_submitted = true;

        let msg = sow_core::protocol::ClientMessage::SubmitStats {
            kills: me.kills,
            deaths: me.deaths,
            assists: me.assists,
            players_defeated: self.progress_session_defeats.players,
            empires_defeated: self.progress_session_defeats.empires,
            tribes_defeated: self.progress_session_defeats.tribes,
        };
        if let Ok(json) = bincode::serialize(&msg) {
            if let Some(c) = self.net.client.as_ref() {
                c.send(json);
                log::info!(
                    "Submitted online stats: K/D/A {}/{}/{}",
                    me.kills,
                    me.deaths,
                    me.assists
                );
            }
        }
    }

    pub(crate) fn maybe_record_match_progress(
        &mut self,
        winner: Option<u16>,
        winning_team: Option<sow_core::protocol::Team>,
        my_team: Option<sow_core::protocol::Team>,
    ) {
        if self.progress_match_recorded {
            return;
        }
        let Some(winner_id) = winner else {
            return;
        };
        let my_id = self.sim.my_player_id.unwrap_or(0);
        if my_id == 0 {
            return;
        }
        self.progress_match_recorded = true;

        // Online ranked matches: relay + sow-database own the outcome; client only reads profile later.
        if self.progress_account_id.is_some() && !self.net.is_offline {
            log::info!(
                "Online match ended (winner={winner_id}); stats will sync from sow-database on menu return"
            );
            return;
        }

        let won = if let Some(team) = winning_team {
            my_team == Some(team)
        } else {
            winner_id == my_id
        };
        let defeats = self.progress_session_defeats;
        let (kills, deaths, assists) = self
            .sim
            .current_snapshot
            .as_ref()
            .and_then(|s| s.players.iter().find(|p| p.id == my_id))
            .map(|p| (p.kills, p.deaths, p.assists))
            .unwrap_or((0, 0, 0));
        self.progress.preferred_leader = Some(self.ui.app.main_menu_state.selected_leader);
        self.progress
            .record_match_with_kda(won, defeats, kills, deaths, assists);
        self.save_local_progress();
        crate::store_portals::submit_leaderboard_score(self.progress.xp);
        log::info!(
            "Recorded local match progress: won={won}, defeats={defeats:?}, level={}",
            self.progress.level
        );
    }
}
