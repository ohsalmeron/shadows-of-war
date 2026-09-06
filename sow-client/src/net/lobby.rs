use crate::MapDownloadEvent;
use crate::app::SowApp;
use sow_ui::ui::main_menu::MainMenuState;

/// Private lobbies are excluded from the global LobbiesBroadcast; seed local state on join.
pub(crate) fn seed_joined_lobby_entry(
    state: &mut MainMenuState,
    ack: &sow_core::protocol::ServerJoinAckMessage,
) {
    // Prefer the server-authoritative snapshot (real mode, slots, bots, host, teams) so
    // private lobbies — which are never broadcast — show truthful info, not placeholders.
    if let Some(info) = &ack.lobby_info {
        if let Some(existing) = state.lobbies.iter_mut().find(|l| l.id == ack.lobby_id) {
            *existing = info.clone();
        } else {
            state.lobbies.push(info.clone());
        }
        return;
    }

    // Legacy fallback (older server without lobby_info): seed just our own entry.
    let me = sow_core::protocol::LobbyPlayerSyncState {
        name: state.player_name.clone(),
        is_ready: false,
        download_progress: 0,
        leader: state.selected_leader,
        player_id: ack.player_id,
        team: None,
    };
    if let Some(lobby) = state.lobbies.iter_mut().find(|l| l.id == ack.lobby_id) {
        lobby.map_name = ack.map_name.clone();
        if !lobby.players.iter().any(|p| p.name == me.name) {
            lobby.players.push(me);
        }
        lobby.num_players = lobby.players.len() as u32;
    } else {
        let kind = if ack.is_private {
            sow_core::protocol::LobbyKind::Custom
        } else {
            sow_core::protocol::LobbyKind::Matchmaking
        };
        state.lobbies.push(sow_core::protocol::LobbyInfo {
            id: ack.lobby_id,
            kind,
            num_players: 1,
            max_players: 0, // unknown until the server broadcast arrives
            is_counting_down: false,
            timer_secs: 0.0,
            map_name: ack.map_name.clone(),
            game_mode: "FFA".to_string(),
            players: vec![me],
            has_password: false,
            host_name: String::new(),
            bot_count: 0,
            nation_count: 0,
            bot_difficulty: Default::default(),
        });
    }
}

pub(crate) fn apply_lobbies_broadcast(
    state: &mut MainMenuState,
    broadcast: &sow_core::protocol::ServerLobbiesBroadcastMessage,
) {
    let joined_id = state.joined_lobby_id;
    let joined_snapshot =
        joined_id.and_then(|id| state.lobbies.iter().find(|l| l.id == id).cloned());
    state.lobbies = broadcast.lobbies.clone();
    if let Some(id) = joined_id {
        if let Some(broadcast_lobby) = state.lobbies.iter_mut().find(|l| l.id == id) {
            // If the joined lobby is in the broadcast, preserve live timer if countdown is active
            if let Some(ref snap) = joined_snapshot
                && snap.is_counting_down
                && snap.timer_secs < broadcast_lobby.timer_secs
            {
                broadcast_lobby.timer_secs = snap.timer_secs;
                broadcast_lobby.is_counting_down = snap.is_counting_down;
            }
        } else if let Some(snap) = joined_snapshot {
            // If the joined lobby is in Loading/ReadyForRelay phase and omitted from broadcast,
            // push the existing live snapshot so the UI retains live timer_secs and players.
            state.lobbies.push(snap);
        }
    }
}

impl SowApp {
    pub(crate) fn fetch_map_catalog_if_needed(&mut self) {
        if self.ui.app.asset_loader.map_catalog.is_some()
            || self.ui.app.asset_loader.catalog_in_flight
        {
            return;
        }
        self.ui.app.asset_loader.catalog_in_flight = true;
        let url = format!(
            "{}/catalog.bin",
            self.asset_config.maps_base.trim_end_matches('/')
        );
        let tx = self.tasks.map_tx.clone();
        let request = ehttp::Request::get(&url);
        ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
            if let Ok(res) = result
                && res.ok
                && let Ok(catalog) = sow_core::map_file::parse_catalog(&res.bytes)
            {
                let _ = tx.send(MapDownloadEvent::CatalogReady(catalog.entries));
                return;
            }
            log::warn!("Failed to fetch map catalog.bin");
            let cached = crate::map_cache::catalog_from_cache();
            if cached.is_empty() {
                let _ = tx.send(MapDownloadEvent::CatalogReady(Vec::new()));
            } else {
                log::info!(
                    "Using {} map(s) from offline cache for catalog",
                    cached.len()
                );
                let _ = tx.send(MapDownloadEvent::CatalogReady(cached));
            }
        });
    }

    pub(crate) fn send_join_if_connected(
        &mut self,
        target_lobby_id: Option<u64>,
        host_private: bool,
    ) {
        let join_msg = self.make_join_message(target_lobby_id, host_private, None, None);
        if let Ok(json) = bincode::serialize(&join_msg)
            && let Some(c) = self.net.client.as_ref()
        {
            c.send(json);
        }
        self.ui.app.main_menu_state.is_waiting = true;
    }
}
