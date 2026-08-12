use crate::MapDownloadEvent;
use crate::app::SowApp;
use sow_ui_kit::ClientPhase;

impl SowApp {
    pub fn update_assets(&mut self) {
        self.poll_thumbnail_fetches();
        self.poll_leader_portrait_fetches();
        self.poll_boot_ui_fetches();
        self.poll_avatar_fetches();
        self.poll_database_events();

        // Poll map download channel
        while let Ok(res) = self.tasks.map_rx.try_recv() {
            match res {
                MapDownloadEvent::CatalogReady(entries) => {
                    self.ui.app.asset_loader.catalog_in_flight = false;
                    self.ui
                        .app
                        .main_menu_state
                        .apply_map_catalog_custom(&entries);
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
                                    bincode::serialize(&self.make_ready_message(lid, pid)).unwrap(),
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
                    self.ui
                        .app
                        .asset_loader
                        .enqueue_leader_portrait_bytes(leader, mobile, bytes);
                }
                MapDownloadEvent::BootUiReady { kind, bytes } => {
                    match self.ui.app.asset_loader.ingest_boot_ui_webp_bytes(
                        &self.ui.egui_ctx,
                        kind,
                        &bytes,
                    ) {
                        Ok(()) => log::debug!("Loaded boot UI asset {:?}", kind),
                        Err(e) => log::warn!("Failed to ingest boot UI {:?}: {}", kind, e),
                    }
                }
                MapDownloadEvent::BootUiFailed { kind, reason } => {
                    log::warn!("Boot UI fetch failed for {:?}: {}", kind, reason);
                    self.ui.app.asset_loader.note_boot_ui_fetch_failed(kind);
                }
                MapDownloadEvent::AvatarReady { leader, bytes } => {
                    let key = match leader {
                        Some(l) => sow_ui::ui::asset_loader::AvatarFetchKey::Leader(l),
                        None => sow_ui::ui::asset_loader::AvatarFetchKey::Fallback,
                    };
                    match self.ui.app.asset_loader.ingest_avatar_webp_bytes(
                        &self.ui.egui_ctx,
                        key,
                        &bytes,
                    ) {
                        Ok(()) => log::debug!("Loaded avatar {:?}", key),
                        Err(e) => log::warn!("Failed to ingest avatar {:?}: {e}", key),
                    }
                    // GPU atlas upload is fed inside `ingest_avatar_webp_bytes` (covers native too).
                }
                MapDownloadEvent::AvatarFailed { leader, reason } => {
                    let key = match leader {
                        Some(l) => sow_ui::ui::asset_loader::AvatarFetchKey::Leader(l),
                        None => sow_ui::ui::asset_loader::AvatarFetchKey::Fallback,
                    };
                    log::warn!("Avatar fetch failed for {:?}: {reason}", key);
                    self.ui
                        .app
                        .asset_loader
                        .note_avatar_fetch_failed(key, reason);
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
                    self.ui.app.asset_loader.note_leader_portrait_fetch_failed(
                        leader,
                        mobile,
                        reason.clone(),
                    );
                    if let Some((attempt, retry_in, last_error)) =
                        self.ui.app.asset_loader.leader_retry_debug(leader, mobile)
                    {
                        log::debug!(
                            "Leader portrait retry scheduled for {:?} mobile={} attempt={} retry_in_ms={} last_error={}",
                            leader,
                            mobile,
                            attempt,
                            retry_in.as_millis(),
                            last_error
                        );
                    }
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

        if self.ui.app.phase == ClientPhase::MainMenu || self.ui.app.phase == ClientPhase::Splash {
            // Same orientation test as the backdrop's set_leader_portrait_focus
            // (`width < height`); compact_viewport would compute a different key
            // on wide-but-short windows and the decode would never match.
            let mobile = sow_ui_kit::theme::portrait_layout(&self.ui.egui_ctx);
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
        let url = self.asset_config.map_url(&map_name, "thumbnail.webp");
        let tx = self.tasks.map_tx.clone();
        let map_name_for_closure = map_name.clone();
        let request = ehttp::Request::get(&url);
        log::debug!("Fetching map thumbnail: {}", url);
        ehttp::fetch(
            request,
            move |result: ehttp::Result<ehttp::Response>| match result {
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
            },
        );
    }

    fn fetch_leader_portrait(
        url: String,
        tx: crossbeam_channel::Sender<MapDownloadEvent>,
        leader: sow_core::player::Leader,
        mobile: bool,
    ) {
        let request = ehttp::Request::get(&url);
        ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
            let send = match result {
                Ok(res) if res.ok => MapDownloadEvent::LeaderPortraitReady {
                    leader,
                    mobile,
                    bytes: res.bytes,
                },
                Ok(res) => MapDownloadEvent::LeaderPortraitFailed {
                    leader,
                    mobile,
                    reason: format!("HTTP {}", res.status),
                },
                Err(e) => MapDownloadEvent::LeaderPortraitFailed {
                    leader,
                    mobile,
                    reason: e.to_string(),
                },
            };
            let _ = tx.send(send);
        });
    }

    fn fetch_boot_ui(
        url: String,
        tx: crossbeam_channel::Sender<MapDownloadEvent>,
        kind: sow_ui::ui::asset_loader::UiSplashTexture,
    ) {
        let request = ehttp::Request::get(&url);
        ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
            let send = match result {
                Ok(res) if res.ok => MapDownloadEvent::BootUiReady {
                    kind,
                    bytes: res.bytes,
                },
                Ok(res) => MapDownloadEvent::BootUiFailed {
                    kind,
                    reason: format!("HTTP {}", res.status),
                },
                Err(e) => MapDownloadEvent::BootUiFailed {
                    kind,
                    reason: e.to_string(),
                },
            };
            let _ = tx.send(send);
        });
    }

    fn poll_boot_ui_fetches(&mut self) {
        use sow_ui::ui::asset_loader::MAX_BOOT_UI_FETCHES_IN_FLIGHT;

        while self.ui.app.asset_loader.boot_ui_in_flight.len() < MAX_BOOT_UI_FETCHES_IN_FLIGHT {
            let Some(kind) = self.ui.app.asset_loader.take_next_boot_ui_fetch_pending() else {
                break;
            };
            let url = self.asset_config.boot_ui_asset_url(kind.filename());
            log::debug!("Fetching boot UI {:?} url={}", kind, url);
            let tx = self.tasks.map_tx.clone();
            Self::fetch_boot_ui(url, tx, kind);
        }
    }

    fn fetch_avatar(
        url: String,
        tx: crossbeam_channel::Sender<MapDownloadEvent>,
        leader: Option<sow_core::player::Leader>,
    ) {
        let request = ehttp::Request::get(&url);
        ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
            let send = match result {
                Ok(res) if res.ok => MapDownloadEvent::AvatarReady {
                    leader,
                    bytes: res.bytes,
                },
                Ok(res) => MapDownloadEvent::AvatarFailed {
                    leader,
                    reason: format!("HTTP {}", res.status),
                },
                Err(e) => MapDownloadEvent::AvatarFailed {
                    leader,
                    reason: e.to_string(),
                },
            };
            let _ = tx.send(send);
        });
    }

    fn poll_avatar_fetches(&mut self) {
        use sow_ui::ui::asset_loader::{AssetLoader, AvatarFetchKey, MAX_AVATAR_FETCHES_IN_FLIGHT};

        let priority_leader = self.ui.app.main_menu_state.selected_leader;
        let priority = AvatarFetchKey::Leader(priority_leader);

        while self.ui.app.asset_loader.avatars_in_flight.len() < MAX_AVATAR_FETCHES_IN_FLIGHT {
            let Some(key) = self
                .ui
                .app
                .asset_loader
                .take_next_avatar_fetch_pending(priority)
            else {
                break;
            };

            let filename = AssetLoader::avatar_filename(key);
            let url = self.asset_config.avatar_url(&filename);
            let leader = match key {
                AvatarFetchKey::Fallback => None,
                AvatarFetchKey::Leader(l) => Some(l),
            };
            log::debug!("Fetching avatar {:?} url={}", key, url);
            let tx = self.tasks.map_tx.clone();
            Self::fetch_avatar(url, tx, leader);
        }
    }

    fn poll_leader_portrait_fetches(&mut self) {
        use sow_ui::ui::asset_loader::{
            AssetLoader, LeaderPortraitKey, MAX_LEADER_FETCHES_IN_FLIGHT,
        };

        // Must match the orientation test used when the backdrop sets focus
        // (draw_leader_hero_backdrop: `width < height`). compact_viewport adds
        // width<768/height<600 thresholds that disagree on wide-but-short
        // windows (e.g. CrazyGames iframe embeds), stranding fetched bytes.
        let portrait = sow_ui_kit::theme::portrait_layout(&self.ui.egui_ctx);
        let priority_leader = self.ui.app.main_menu_state.selected_leader;
        let priority = LeaderPortraitKey {
            leader: priority_leader,
            mobile: portrait,
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
            log::info!(
                "Fetching leader portrait {:?} mobile={} url={}",
                key.leader,
                key.mobile,
                url
            );
            let tx = self.tasks.map_tx.clone();
            let leader = key.leader;
            let mobile = key.mobile;
            Self::fetch_leader_portrait(url, tx, leader, mobile);
        }
    }

    fn poll_database_events(&mut self) {
        while let Ok(event) = self.tasks.db_rx.try_recv() {
            match event {
                crate::player_progress::DbEvent::ProfileLoaded {
                    progress,
                    account_id,
                    provider,
                } => {
                    let old_level = self.progress.level;
                    let is_crazygames = provider == "crazygames";
                    self.apply_cloud_profile(progress, account_id, provider);
                    log::info!(
                        "Successfully synced profile from cloud database: level {} ({} XP)",
                        self.progress.level,
                        self.progress.xp
                    );
                    if self.progress.level > old_level {
                        crate::store_portals::happytime();
                    }
                    if is_crazygames {
                        crate::store_portals::submit_leaderboard_score(self.progress.xp);
                    }
                    self.maybe_link_platform_identity();
                    #[cfg(target_arch = "wasm32")]
                    {
                        self.boot_db_settled = true;
                    }
                }
                crate::player_progress::DbEvent::LoadFailed => {
                    log::warn!("sow-database sync failed. Continuing with local offline progress.");
                    #[cfg(target_arch = "wasm32")]
                    {
                        self.boot_db_settled = true;
                    }
                }
                crate::player_progress::DbEvent::LinkConflict(info) => {
                    log::warn!(
                        "Platform link conflict: local L{} vs existing L{}",
                        info.current_level,
                        info.existing_level
                    );
                    self.ui.app.main_menu_state.active_conflict =
                        Some(sow_ui::ui::main_menu::UiLinkConflictInfo {
                            current_account_id: info.current_account_id,
                            existing_account_id: info.existing_account_id,
                            current_level: info.current_level,
                            existing_level: info.existing_level,
                            target_provider: info.target_provider,
                            target_external_id: info.target_external_id,
                        });
                }
                crate::player_progress::DbEvent::LinkResolved {
                    progress,
                    account_id,
                    provider,
                } => {
                    let old_level = self.progress.level;
                    self.apply_cloud_profile(progress, account_id, provider);
                    self.ui.app.main_menu_state.active_conflict = None;
                    if self.progress.level > old_level {
                        crate::store_portals::happytime();
                    }
                }
            }
        }
    }
}
