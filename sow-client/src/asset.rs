use crate::app::SowApp;
use crate::MapDownloadEvent;
use sow_ui::app::ClientPhase;

impl SowApp {
    pub fn update_assets(&mut self) {
        for key in crate::map_cache::list_cached_keys() {
            if !self.ui.app.asset_loader.maps.contains_key(&key) {
                if let Some(bytes) = crate::map_cache::load(&key) {
                    self.ui.app.asset_loader.maps.insert(key, bytes);
                }
            }
        }

        self.poll_thumbnail_fetches();
        self.poll_leader_portrait_fetches();

        // Poll map download channel
        while let Ok(res) = self.tasks.map_rx.try_recv() {
            match res {
                MapDownloadEvent::CatalogReady(entries) => {
                    self.ui.app.asset_loader.catalog_in_flight = false;
                    self.ui
                        .app
                        .main_menu_state
                        .apply_map_catalog(&entries);
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
                    crate::map_cache::persist(&map_name, &bytes);
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
                    self.ui.app.asset_loader.enqueue_leader_portrait_bytes(
                        leader,
                        mobile,
                        bytes,
                    );
                }
                MapDownloadEvent::LeaderPortraitFailed {
                    leader,
                    mobile,
                    reason,
                } => {
                    log::warn!(
                        "Leader portrait fetch failed for {:?} mobile={}: {}",
                        leader,
                        mobile,
                        reason
                    );
                    self.ui
                        .app
                        .asset_loader
                        .note_leader_portrait_fetch_failed(leader, mobile);
                }
                MapDownloadEvent::Error(e) => {
                    log::error!("Map download aborted: {}", e);
                    self.ui.app.main_menu_state.is_downloading_map = false;
                    self.ui.app.phase = ClientPhase::MainMenu;
                    self.ui.app.main_menu_state.is_waiting = false;
                    self.ui.app.main_menu_state.pending_join_lobby_id = None;
                    self.ui.app.main_menu_state.joined_lobby_id = None;
                    self.tasks.engine_init_queued_msg = None;
                }
            }
        }

        if self.ui.app.phase == ClientPhase::MainMenu {
            let mobile = sow_ui::ui::theme::compact_viewport(&self.ui.egui_ctx);
            let selected = self.ui.app.main_menu_state.selected_leader;
            let focus = sow_ui::ui::asset_loader::LeaderPortraitKey {
                leader: selected,
                mobile,
            };
            self.ui
                .app
                .asset_loader
                .process_leader_decode_budget(&self.ui.egui_ctx, 1, focus);
        }
    }

    fn poll_thumbnail_fetches(&mut self) {
        let pending = self.ui.app.asset_loader.drain_thumbnail_fetch_pending();
        for map_name in pending {
            self.start_thumbnail_fetch(map_name);
        }
    }

    pub(crate) fn start_thumbnail_fetch(&mut self, map_name: String) {
        let url = self
            .asset_config
            .map_url(&map_name, "thumbnail.webp");
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

    fn poll_leader_portrait_fetches(&mut self) {
        use sow_ui::ui::asset_loader::{AssetLoader, LeaderPortraitKey, MAX_LEADER_FETCHES_IN_FLIGHT};

        let compact = sow_ui::ui::theme::compact_viewport(&self.ui.egui_ctx);
        let priority_leader = self.ui.app.main_menu_state.selected_leader;
        let priority = LeaderPortraitKey {
            leader: priority_leader,
            mobile: compact,
        };

        while self.ui.app.asset_loader.leaders_in_flight.len() < MAX_LEADER_FETCHES_IN_FLIGHT {
            let Some(key) = self
                .ui
                .app
                .asset_loader
                .take_next_leader_fetch_pending(priority)
            else {
                break;
            };

            let filename = AssetLoader::leader_portrait_filename(key);
            let url = self.asset_config.leader_portrait_url(&filename);
            let tx = self.tasks.map_tx.clone();
            let leader = key.leader;
            let mobile = key.mobile;
            let request = ehttp::Request::get(&url);
            ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
                match result {
                    Ok(res) if res.ok => {
                        let _ = tx.send(MapDownloadEvent::LeaderPortraitReady {
                            leader,
                            mobile,
                            bytes: res.bytes,
                        });
                    }
                    Ok(res) => {
                        let _ = tx.send(MapDownloadEvent::LeaderPortraitFailed {
                            leader,
                            mobile,
                            reason: format!("HTTP {}", res.status),
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(MapDownloadEvent::LeaderPortraitFailed {
                            leader,
                            mobile,
                            reason: e.to_string(),
                        });
                    }
                }
            });
        }
    }
}
