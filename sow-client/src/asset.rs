use crate::app::SowApp;
use crate::{get_assets_url, MapDownloadEvent};
use sow_ui::app::ClientPhase;

impl SowApp {
    pub fn update_assets(&mut self) {
        #[cfg(target_arch = "wasm32")]
        self.poll_leader_portrait_fetches();

        // Poll map download channel
        while let Ok(res) = self.tasks.map_rx.try_recv() {
            match res {
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
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        let size = [img.width() as _, img.height() as _];
                        let image_buffer = img.to_rgba8();
                        let pixels = image_buffer.as_flat_samples();
                        let color_image =
                            egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                        let texture = self.ui.egui_ctx.load_texture(
                            &map_name,
                            color_image,
                            egui::TextureOptions::LINEAR,
                        );
                        let key = map_name.to_lowercase().replace([' ', '_'], "");
                        self.ui.app.asset_loader.thumbnails.insert(key, texture);
                    } else {
                        log::warn!("Failed to decode thumbnail for {}", map_name);
                    }
                    self.ui
                        .app
                        .asset_loader
                        .thumbnails_in_flight
                        .remove(&map_name);
                }
                MapDownloadEvent::MapReady(map_name, bytes) => {
                    let mut valid = true;
                    if let Some(expected_md5) =
                        self.ui.app.asset_loader.expected_md5s.get(&map_name)
                    {
                        let digest = md5::compute(&bytes);
                        let actual_md5 = format!("{:x}", digest);
                        if actual_md5 != *expected_md5 {
                            log::error!(
                                "MD5 mismatch for map {}: expected {}, got {}",
                                map_name,
                                expected_md5,
                                actual_md5
                            );
                            valid = false;
                        } else {
                            log::info!("MD5 verified for map {}", map_name);
                        }
                    }

                    self.ui.app.asset_loader.maps_in_flight.remove(&map_name);
                    if valid {
                        self.ui
                            .app
                            .asset_loader
                            .maps
                            .insert(map_name.clone(), bytes.clone());
                    } else {
                        // Optionally handle failure, we just drop it so it can be retried later
                        continue;
                    }

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
                MapDownloadEvent::ManifestReady(map_name, manifest) => {
                    self.ui
                        .app
                        .asset_loader
                        .manifests_in_flight
                        .remove(&map_name);
                    self.ui
                        .app
                        .asset_loader
                        .manifests
                        .insert(map_name.clone(), manifest.clone());
                    if Some(map_name) == self.ui.app.main_menu_state.downloading_map_name {
                        self.ui.app.main_menu_state.cached_manifest = Some(manifest);
                    }
                }
                MapDownloadEvent::CatalogReady(catalog) => {
                    self.ui.app.asset_loader.catalog_in_flight = false;
                    self.ui.app.asset_loader.map_catalog = Some(catalog);
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

    #[cfg(target_arch = "wasm32")]
    fn poll_leader_portrait_fetches(&mut self) {
        let pending = self.ui.app.asset_loader.drain_leader_fetch_pending();
        if pending.is_empty() {
            return;
        }
        let assets_base = get_assets_url();
        for key in pending {
            let url = format!(
                "{}/ui/leaders/{}",
                assets_base.trim_end_matches('/'),
                sow_ui::ui::asset_loader::AssetLoader::leader_portrait_filename(key)
            );
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
