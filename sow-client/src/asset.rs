use crate::app::SowApp;
use crate::MapDownloadEvent;
use sow_ui::app::ClientPhase;

impl SowApp {
    pub fn update_assets(&mut self) {
                // Poll map download channel
                while let Ok(res) = self.map_rx.try_recv() {
                    match res {
                        MapDownloadEvent::Progress(downloaded_map_name, progress) => {
                            if Some(downloaded_map_name.clone()) == self.app.main_menu_state.downloading_map_name {
                                self.app.main_menu_state.map_download_progress = progress;
                                if let (Some(lid), Some(pid)) = (self.my_lobby_id, self.my_player_id) {
                                    if let Some(c) = self.net_client.as_ref() {
                                        c.send(bincode::serialize(&sow_core::protocol::ClientMessage::MapDownloadProgress {
                                            lobby_id: lid,
                                            player_id: pid,
                                            progress,
                                        }).unwrap());
                                    }
                                }
                            }
                        }
                        MapDownloadEvent::ThumbnailReady(map_name, bytes) => {
                            if let Ok(img) = image::load_from_memory(&bytes) {
                                let size = [img.width() as _, img.height() as _];
                                let image_buffer = img.to_rgba8();
                                let pixels = image_buffer.as_flat_samples();
                                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                                let texture = self.egui_ctx.load_texture(&map_name, color_image, egui::TextureOptions::LINEAR);
                                self.app.asset_loader.thumbnails.insert(map_name.clone(), texture);
                            } else {
                                log::warn!("Failed to decode thumbnail for {}", map_name);
                            }
                            self.app.asset_loader.thumbnails_in_flight.remove(&map_name);
                        }
                        MapDownloadEvent::MapReady(map_name, bytes) => {
                            let mut valid = true;
                            if let Some(expected_md5) = self.app.asset_loader.expected_md5s.get(&map_name) {
                                let digest = md5::compute(&bytes);
                                let actual_md5 = format!("{:x}", digest);
                                if actual_md5 != *expected_md5 {
                                    log::error!("MD5 mismatch for map {}: expected {}, got {}", map_name, expected_md5, actual_md5);
                                    valid = false;
                                } else {
                                    log::info!("MD5 verified for map {}", map_name);
                                }
                            }
                            
                            self.app.asset_loader.maps_in_flight.remove(&map_name);
                            if valid {
                                self.app.asset_loader.maps.insert(map_name.clone(), bytes.clone());
                            } else {
                                // Optionally handle failure, we just drop it so it can be retried later
                                continue;
                            }
                            
                            if Some(map_name.clone()) == self.app.main_menu_state.downloading_map_name {
                                log::info!("Map download completed successfully.");
                                self.app.main_menu_state.cached_map = Some(bytes);
                                self.app.main_menu_state.is_downloading_map = false;
                                self.app.main_menu_state.map_download_progress = 100;
                                
                                if let (Some(lid), Some(pid)) = (self.my_lobby_id, self.my_player_id) {
                                    if let Some(c) = self.net_client.as_ref() {
                                        c.send(bincode::serialize(&sow_core::protocol::ClientMessage::MapDownloadProgress {
                                            lobby_id: lid,
                                            player_id: pid,
                                            progress: 100,
                                        }).unwrap());
                                        // Also send Ready to Orchestrator to signal we are prepared for handoff
                                        c.send(bincode::serialize(&sow_core::protocol::ClientMessage::Ready {
                                            lobby_id: lid,
                                            player_id: pid,
                                        }).unwrap());
                                    }
                                }
                            }
                        }
                        MapDownloadEvent::Error(e) => {
                            log::error!("Map download aborted: {}", e);
                            self.app.main_menu_state.is_downloading_map = false;
                            // Optionally return to main menu or show error
                            self.app.phase = ClientPhase::MainMenu;
                            self.app.main_menu_state.is_waiting = false;
                            self.app.main_menu_state.pending_join_lobby_id = None;
                            self.app.main_menu_state.joined_lobby_id = None;
                        }
                    }
                }

    }
}
