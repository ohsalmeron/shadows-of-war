use crate::app::SowApp;
use crate::MapDownloadEvent;
use sow_ui::ui::main_menu::MainMenuState;

/// Private lobbies are excluded from the global LobbiesBroadcast; seed local state on join.
pub(crate) fn seed_joined_lobby_entry(
    state: &mut MainMenuState,
    ack: &sow_core::protocol::ServerJoinAckMessage,
) {
    let me = sow_core::protocol::LobbyPlayerSyncState {
        name: state.player_name.clone(),
        is_ready: false,
        download_progress: 0,
        leader: state.selected_leader,
    };
    if let Some(lobby) = state.lobbies.iter_mut().find(|l| l.id == ack.lobby_id) {
        lobby.map_name = ack.map_name.clone();
        if !lobby.players.iter().any(|p| p.name == me.name) {
            lobby.players.push(me);
        }
        lobby.num_players = lobby.players.len() as u32;
    } else {
        // Self-seeded entry for a joined lobby not yet in the broadcast.
        // Always Custom — Matchmaking lobbies are always present in the broadcast before join.
        state.lobbies.push(sow_core::protocol::LobbyInfo {
            id: ack.lobby_id,
            kind: sow_core::protocol::LobbyKind::Custom,
            num_players: 1,
            max_players: 8,
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
    if let (Some(id), Some(snap)) = (joined_id, joined_snapshot) {
        if !state.lobbies.iter().any(|l| l.id == id) {
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
            if let Ok(res) = result {
                if res.ok {
                    if let Ok(catalog) = sow_core::map_file::parse_catalog(&res.bytes) {
                        let _ = tx.send(MapDownloadEvent::CatalogReady(catalog.entries));
                        return;
                    }
                }
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
        if let Ok(json) = bincode::serialize(&join_msg) {
            if let Some(c) = self.net.client.as_ref() {
                c.send(json);
            }
        }
        self.ui.app.main_menu_state.is_waiting = true;
    }
}
