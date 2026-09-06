use super::state::SowApp;

impl SowApp {
    pub(crate) fn reset_progress_session(&mut self) {
        self.progress_match_recorded = false;
        self.progress_stats_submitted = false;
        self.progress_result_submitted = false;
        self.progress_session_defeats = crate::player_progress::SessionDefeats::default();
        self.ui.reward_cache = None;
    }

    pub(crate) fn maybe_submit_online_stats(&mut self, snap: &sow_core::protocol::SimSnapshot) {
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
        if game_over {
            if self.progress_result_submitted {
                return;
            }
        } else if self.progress_stats_submitted {
            return;
        }
        if !game_over && !eliminated {
            return;
        }

        let leader = self
            .ui
            .app
            .main_menu_state
            .selected_leader
            .name()
            .to_string();
        let msg = if game_over {
            self.progress_result_submitted = true;
            sow_core::protocol::ClientMessage::SubmitMatchReport {
                kills: me.kills,
                deaths: me.deaths,
                assists: me.assists,
                players_defeated: self.progress_session_defeats.players,
                empires_defeated: self.progress_session_defeats.empires,
                tribes_defeated: self.progress_session_defeats.tribes,
                leader,
                winner_player_id: snap.winner,
                winning_team: snap.winning_team,
                tick: snap.tick,
            }
        } else {
            self.progress_stats_submitted = true;
            sow_core::protocol::ClientMessage::SubmitStatsWithLeader {
                kills: me.kills,
                deaths: me.deaths,
                assists: me.assists,
                players_defeated: self.progress_session_defeats.players,
                empires_defeated: self.progress_session_defeats.empires,
                tribes_defeated: self.progress_session_defeats.tribes,
                leader,
            }
        };
        if let Ok(json) = bincode::serialize(&msg)
            && let Some(c) = self.net.client.as_ref()
        {
            c.send(json);
            log::info!(
                "Submitted online stats: K/D/A {}/{}/{}",
                me.kills,
                me.deaths,
                me.assists
            );
        }
    }

    pub(crate) fn maybe_record_match_progress(
        &mut self,
        snap: &sow_core::protocol::SimSnapshot,
        winner: Option<u16>,
        winning_team: Option<sow_core::protocol::Team>,
        my_team: Option<sow_core::protocol::Team>,
    ) {
        if self.progress_match_recorded {
            return;
        }
        let my_id = self.sim.my_player_id.unwrap_or(0);
        if my_id == 0 {
            return;
        }

        let Some(me) = snap.players.iter().find(|p| p.id == my_id) else {
            return;
        };
        let eliminated = !me.alive && me.has_spawned;
        let Some(winner_id) = winner.or_else(|| eliminated.then_some(0)) else {
            return;
        };
        self.progress_match_recorded = true;

        let won = if let Some(team) = winning_team {
            my_team == Some(team)
        } else {
            winner_id == my_id
        };
        let defeats = self.progress_session_defeats;
        let (kills, deaths, assists) = (me.kills, me.deaths, me.assists);
        crate::analytics::track_with(
            "match_ended_client",
            serde_json::json!({
                "won": won,
                "offline": self.net.is_offline,
                "tutorial": self.sim.config.tutorial,
                "kills": kills,
            }),
        );
        self.ui.reward_cache = Some(sow_data::rewards::calculate(
            sow_data::rewards::RewardInput {
                won,
                players_defeated: defeats.players,
                empires_defeated: defeats.empires,
                tribes_defeated: defeats.tribes,
                kills,
                assists,
                ..Default::default()
            },
        ));

        // Online ranked matches: relay + sow-database own the outcome; client only reads profile later.
        if self.progress_account_id.is_some() && !self.net.is_offline {
            log::info!(
                "Online match ended (winner={winner_id}); stats will sync from sow-database on menu return"
            );
            return;
        }

        self.progress.preferred_leader = Some(self.ui.app.main_menu_state.selected_leader);
        self.progress
            .record_match_with_kda(won, defeats, kills, deaths, assists);
        self.save_local_progress();
        log::info!(
            "Recorded local match progress: won={won}, defeats={defeats:?}, level={}",
            self.progress.level
        );
    }
}
