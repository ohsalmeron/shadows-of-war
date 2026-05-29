use crate::app::SowApp;
use crate::{get_maps_url, MapDownloadEvent};
#[cfg(target_arch = "wasm32")]
use crate::{get_assets_cache_bust, get_assets_url};
use sow_ui::app::ClientPhase;

impl SowApp {
    pub fn update_assets(&mut self) {
        self.poll_thumbnail_fetches();

        #[cfg(target_arch = "wasm32")]
        self.poll_leader_portrait_fetches();

        // Poll map download channel
        while let Ok(res) = self.tasks.map_rx.try_recv() {
            match res {
                MapDownloadEvent::CatalogReady(entries) => {
                    self.ui.app.asset_loader.catalog_in_flight = false;
                    self.ui.app.asset_loader.map_catalog = Some(entries);
                }
                MapDownloadEvent::Progress(downloaded_map_name, progress) => {
                    if Some(downloaded_map_name.clone())
                        == self.ui.app.main_menu_state.downloading_map_name
                    {
                        self.ui.app.main_menu_state.map_download_progress = progress;
                        if let (Some(lid), Some(pid)) =
                            (self.sim.my_lobby_id, self.sim.my_player_id)
                        {
                            if let Some(c) = self.net.client.as_ref() {
                                c.send(
                                    bincode::serialize(
                                        &sow_core::protocol::ClientMessage::MapDownloadProgress {
                                            lobby_id: lid,
                                            player_id: pid,
                                            progress,
                                        },
                                    )
                                    .unwrap(),
                                );
                            }
                        }
                    }
                }
                MapDownloadEvent::ThumbnailReady(map_name, bytes) => {
                    match self.ui.app.asset_loader.ingest_thumbnail(
                        &self.ui.egui_ctx,
                        &map_name,
                        &bytes,
                    ) {
                        Ok(()) => {
                            log::debug!("Loaded map thumbnail: {}", map_name);
                        }
                        Err(e) => {
                            log::warn!("Failed to decode thumbnail for {}: {}", map_name, e);
                            self.ui
                                .app
                                .asset_loader
                                .note_thumbnail_failure(&map_name, e);
                        }
                    }
                }
                MapDownloadEvent::ThumbnailFailed(map_name, reason) => {
                    log::warn!("Map thumbnail fetch failed for {}: {}", map_name, reason);
                    self.ui
                        .app
                        .asset_loader
                        .note_thumbnail_failure(&map_name, reason);
                }
                MapDownloadEvent::MapReady(map_name, bytes) => {
                    self.ui.app.asset_loader.maps_in_flight.remove(&map_name);
                    self.ui
                        .app
                        .asset_loader
                        .maps
                        .insert(map_name.clone(), bytes.clone());

                    if Some(map_name.clone()) == self.ui.app.main_menu_state.downloading_map_name {
                        log::info!("Map download completed successfully.");
                        self.ui.app.main_menu_state.cached_map = Some(bytes);
                        self.ui.app.main_menu_state.is_downloading_map = false;
                        self.ui.app.main_menu_state.map_download_progress = 100;

                        if let (Some(lid), Some(pid)) =
                            (self.sim.my_lobby_id, self.sim.my_player_id)
                        {
                            if let Some(c) = self.net.client.as_ref() {
                                c.send(
                                    bincode::serialize(
                                        &sow_core::protocol::ClientMessage::MapDownloadProgress {
                                            lobby_id: lid,
                                            player_id: pid,
                                            progress: 100,
                                        },
                                    )
                                    .unwrap(),
                                );
                                // Also send Ready to Orchestrator to signal we are prepared for handoff
                                c.send(
                                    bincode::serialize(&sow_core::protocol::ClientMessage::Ready {
                                        lobby_id: lid,
                                        player_id: pid,
                                    })
                                    .unwrap(),
                                );
                            }
                        }
                    }
                }
                MapDownloadEvent::LeaderPortraitReady {
                    leader,
                    mobile,
                    bytes,
                } => {
                    self.ui.app.asset_loader.ingest_leader_portrait(
                        &self.ui.egui_ctx,
                        leader,
                        mobile,
                        &bytes,
                    );
                }
                MapDownloadEvent::Error(e) => {
                    log::error!("Map download aborted: {}", e);
                    self.ui.app.main_menu_state.is_downloading_map = false;
                    // Optionally return to main menu or show error
                    self.ui.app.phase = ClientPhase::MainMenu;
                    self.ui.app.main_menu_state.is_waiting = false;
                    self.ui.app.main_menu_state.pending_join_lobby_id = None;
                    self.ui.app.main_menu_state.joined_lobby_id = None;
                    self.tasks.engine_init_queued_msg = None;
                }
            }
        }
    }

    fn poll_thumbnail_fetches(&mut self) {
        let pending = self.ui.app.asset_loader.drain_thumbnail_fetch_pending();
        for map_name in pending {
            self.start_thumbnail_fetch(map_name);
        }
    }

    pub(crate) fn start_thumbnail_fetch(&mut self, map_name: String) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(bytes) = sow_core::maps::read_thumbnail_webp_from_repo(&map_name) {
                match self.ui.app.asset_loader.ingest_thumbnail(
                    &self.ui.egui_ctx,
                    &map_name,
                    &bytes,
                ) {
                    Ok(()) => {
                        log::debug!("Loaded map thumbnail from repo: {}", map_name);
                        return;
                    }
                    Err(e) => {
                        log::warn!(
                            "Repo thumbnail decode failed for {}: {}",
                            map_name,
                            e
                        );
                    }
                }
            }
        }

        let url = format!(
            "{}/{}/thumbnail.webp",
            get_maps_url().trim_end_matches('/'),
            map_name
        );
        let tx = self.tasks.map_tx.clone();
        let map_name_for_closure = map_name.clone();
        let request = ehttp::Request::get(&url);
        log::debug!("Fetching map thumbnail: {}", url);
        ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
            match result {
                Ok(res) if res.ok => {
                    let _ = tx.send(MapDownloadEvent::ThumbnailReady(
                        map_name_for_closure,
                        res.bytes,
                    ));
                }
                Ok(res) => {
                    let _ = tx.send(MapDownloadEvent::ThumbnailFailed(
                        map_name_for_closure,
                        format!("HTTP {}", res.status),
                    ));
                }
                Err(e) => {
                    let _ = tx.send(MapDownloadEvent::ThumbnailFailed(
                        map_name_for_closure,
                        e.to_string(),
                    ));
                }
            }
        });
    }

    #[cfg(target_arch = "wasm32")]
    fn poll_leader_portrait_fetches(&mut self) {
        let pending = self.ui.app.asset_loader.drain_leader_fetch_pending();
        if pending.is_empty() {
            return;
        }
        let assets_base = get_assets_url();
        let cache_bust = get_assets_cache_bust();
        for key in pending {
            let filename =
                sow_ui::ui::asset_loader::AssetLoader::leader_portrait_filename(key);
            let url = if cache_bust.is_empty() {
                format!(
                    "{}/ui/leaders/{}",
                    assets_base.trim_end_matches('/'),
                    filename
                )
            } else {
                format!(
                    "{}/ui/leaders/{}?v={}",
                    assets_base.trim_end_matches('/'),
                    filename,
                    cache_bust
                )
            };
            let tx = self.tasks.map_tx.clone();
            let leader = key.leader;
            let mobile = key.mobile;
            let request = ehttp::Request::get(&url);
            ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
                if let Ok(res) = result {
                    if res.ok {
                        let _ = tx.send(MapDownloadEvent::LeaderPortraitReady {
                            leader,
                            mobile,
                            bytes: res.bytes,
                        });
                    } else {
                        log::warn!(
                            "Leader portrait fetch failed for {:?} mobile={}: HTTP {}",
                            leader,
                            mobile,
                            res.status
                        );
                    }
                }
            });
        }
    }
}
