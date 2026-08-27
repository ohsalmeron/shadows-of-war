use crate::MapDownloadEvent;
use crate::app::SowApp;
use sow_ui_kit::ClientPhase;

impl SowApp {
    pub fn update_assets(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
        self.poll_thumbnail_fetches();
        self.poll_leader_portrait_fetches();
        self.poll_boot_ui_fetches();
        }
        self.poll_avatar_fetches();
        self.poll_portal_avatar_fetch();
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
                MapDownloadEvent::PortalAvatarReady { bytes } => {
                    match self
                        .ui
                        .app
                        .asset_loader
                        .ingest_portal_avatar_bytes(&self.ui.egui_ctx, &bytes)
                    {
                        Ok(()) => log::info!("Loaded portal identity avatar"),
                        Err(e) => log::warn!("Failed to ingest portal avatar: {e}"),
                    }
                }
                MapDownloadEvent::PortalAvatarFailed { reason } => {
                    self.ui
                        .app
                        .asset_loader
                        .note_portal_avatar_failed(reason);
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

        #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_thumbnail_fetches(&mut self) {
        let pending = self.ui.app.asset_loader.drain_thumbnail_fetch_pending();
        for map_name in pending {
            self.start_thumbnail_fetch(map_name);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(not(target_arch = "wasm32"))]
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

    fn poll_portal_avatar_fetch(&mut self) {
        if self.ui.app.asset_loader.portal_avatar.is_some()
            || self.ui.app.asset_loader.portal_avatar_in_flight
        {
            return;
        }
        let Some(url) = self.ui.app.asset_loader.portal_avatar_request.clone() else {
            return;
        };
        self.ui.app.asset_loader.portal_avatar_in_flight = true;
        self.ui.app.asset_loader.portal_avatar_request = None;
        log::debug!("Fetching portal avatar url={url}");
        let tx = self.tasks.map_tx.clone();
        Self::fetch_portal_avatar(url, tx);
    }

    fn fetch_portal_avatar(url: String, tx: crossbeam_channel::Sender<MapDownloadEvent>) {
        let request = ehttp::Request::get(&url);
        ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
            let send = match result {
                Ok(res) if res.ok => MapDownloadEvent::PortalAvatarReady {
                    bytes: res.bytes,
                },
                Ok(res) => MapDownloadEvent::PortalAvatarFailed {
                    reason: format!("HTTP {}", res.status),
                },
                Err(e) => MapDownloadEvent::PortalAvatarFailed {
                    reason: e.to_string(),
                },
            };
            let _ = tx.send(send);
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
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
                    display_name,
                    provider,
                    request_id,
                } => {
                    if request_id < self.profile_last_applied_request {
                        log::warn!(
                            "[identity] ignoring stale profile response id={request_id} last_applied={}",
                            self.profile_last_applied_request
                        );
                        continue;
                    }
                    self.profile_request_in_flight = false;
                    self.profile_last_applied_request = request_id;
                    let old_level = self.progress.level;
                    log::info!(
                        "[identity] applying profile request id={request_id} provider={provider} account_len={} name_len={}",
                        account_id.chars().count(),
                        display_name.chars().count()
                    );
                    self.apply_cloud_profile(progress, account_id, display_name, provider);
                    log::info!(
                        "Successfully synced profile from cloud database: level {} ({} XP)",
                        self.progress.level,
                        self.progress.xp
                    );
                    if self.progress.level > old_level {
                        crate::store_portals::happytime();
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        self.boot_db_settled = true;
                    }
                    if self.join_waiting_for_identity
                        && self.ui.app.main_menu_state.is_waiting
                        && self.ui.app.main_menu_state.joined_lobby_id.is_none()
                        && self.net.client.is_some()
                    {
                        let join_msg = self.make_join_message(
                            self.ui.app.main_menu_state.pending_join_lobby_id,
                            self.ui.app.main_menu_state.custom_game_is_private,
                            (!self.join_matchmaking)
                                .then(|| self.ui.app.main_menu_state.custom_game_config.clone()),
                            Some(self.ui.app.main_menu_state.custom_game_password.clone())
                                .filter(|password| !password.is_empty()),
                        );
                        if let Ok(json) = bincode::serialize(&join_msg)
                            && let Some(client) = self.net.client.as_ref()
                        {
                            client.send(json);
                            self.join_waiting_for_identity = false;
                        }
                    }
                    if self.profile_refresh_pending && !self.display_name_save_in_flight {
                        self.profile_refresh_pending = false;
                        self.fetch_cloud_progress();
                    }
                }
                crate::player_progress::DbEvent::DisplayNameSaved {
                    account_id,
                    display_name,
                    request_id,
                } => {
                    self.display_name_save_in_flight = false;
                    if self.progress_account_id.as_deref() != Some(account_id.as_str()) {
                        log::error!(
                            "[identity] ignoring rename ACK id={request_id}: account changed while request was in flight"
                        );
                        if let Some(confirmed) = self.confirmed_display_name.clone() {
                            self.ui.app.main_menu_state.player_name = confirmed;
                        }
                        continue;
                    }
                    self.confirmed_display_name = Some(display_name.clone());
                    if let Some(next_name) = self.queued_display_name.take() {
                        // No False Victories: the UI name is provisional until the
                        // database ACK; serialize a newer edit after this ACK.
                        self.ui.app.main_menu_state.player_name = next_name.clone();
                        self.save_display_name(next_name);
                    } else {
                        self.ui.app.main_menu_state.player_name = display_name;
                        self.ui.app.main_menu_state.name_locked = false;
                        if self.profile_refresh_pending {
                            self.profile_refresh_pending = false;
                            self.fetch_cloud_progress();
                        }
                    }
                }
                crate::player_progress::DbEvent::DisplayNameSaveFailed { request_id, status } => {
                    self.display_name_save_in_flight = false;
                    log::warn!(
                        "[identity] rename request id={request_id} not acknowledged status={status:?}; restoring confirmed name"
                    );
                    if let Some(confirmed) = self.confirmed_display_name.clone() {
                        self.ui.app.main_menu_state.player_name = confirmed;
                    }
                    if let Some(next_name) = self.queued_display_name.take() {
                        self.ui.app.main_menu_state.player_name = next_name.clone();
                        self.save_display_name(next_name);
                    } else if self.profile_refresh_pending {
                        self.profile_refresh_pending = false;
                        self.fetch_cloud_progress();
                    }
                }
                crate::player_progress::DbEvent::LoadFailed { request_id, status } => {
                    self.profile_request_in_flight = false;
                    self.profile_refresh_pending = false;
                    // No fallback: continuing without an identity once masked a
                    // 403 from a misrouted endpoint and booted the wrong mode.
                    // A failed identity load is a hard failure — crash loudly.
                        panic!(
                        "[identity] profile request id={request_id} failed status={status:?} — no fallback, refusing to continue without identity"
                        );
                    }
                crate::player_progress::DbEvent::TutorialCompletionFailed { request_id, status } => {
                    self.profile_request_in_flight = false;
                    log::warn!(
                        "[tutorial] completion request id={request_id} failed status={status:?}; local reward retained"
                    );
                    if self.profile_refresh_pending {
                        self.profile_refresh_pending = false;
                        self.fetch_cloud_progress();
                    }
                }
            }
        }
    }
}
