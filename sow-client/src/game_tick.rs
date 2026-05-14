#![allow(unused_imports)]
use sow_render::{RenderContext, MapRenderer, MapGlobals};
use crate::sim_bridge::{SimBridge, PlatformSimBridge};
use sow_core::protocol::{SimCommand, SimSnapshot};

use sow_core::game_config::GameConfig;

use blade_graphics as gpu;
use blade_egui::GuiPainter;
use egui::{Context, RawInput, Pos2, Rect, Vec2};
use sow_ui::{ClientApp, app::ClientPhase, UiAction};
use web_time::{Instant, Duration};
use sow_net::client::SowClient;
use std::collections::HashMap;
use crate::{CAMERA_MIN_ZOOM, camera_zoom_upper_bound, NAMEPLATE_REFERENCE_ZOOM};
use crate::{spawn_sow_client_connect, get_build_version, get_maps_url};
use crate::nameplates::*;
use crate::client_config::ClientVisualConfig;
use crate::{MapDownloadEvent, EngineInitEvent};
use winit::event::{WindowEvent, MouseButton, ElementState, MouseScrollDelta};

use crate::app_state::SowApp;
use std::io::Read;






impl SowApp {
    pub fn game_tick(&mut self, _event_loop: &dyn winit::event_loop::ActiveEventLoop) {
        if self.surface.is_none() && self.window.is_some() {
            let win = self.window.as_ref().unwrap();
            let sz = win.surface_size();
            match self.render_ctx.create_surface(win, sz.width.max(1), sz.height.max(1)) {
                Ok(s) => {
                    self.screen_w = sz.width as f32;
                    self.screen_h = sz.height as f32;
                    let zmax = camera_zoom_upper_bound(self.screen_w, self.screen_h);
                    self.camera_zoom = self.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);
                    self.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::new(self.screen_w, self.screen_h)
                    ));
                    let format = s.info().format;
                    
                    if let Some(sp) = self.prev_sync_point.take() {
                        let _ = self.render_ctx.context.wait_for(&sp, !0);
                    }
                    let mut old_terrain = vec![128; (self.map_w * self.map_h) as usize];
                    if let Some(mut old_mr) = self.map_renderer.take() {
                        old_terrain = old_mr.terrain.clone();
                        old_mr.destroy(&self.render_ctx);
                    }
                    self.map_renderer = Some(sow_render::MapRenderer::new(&self.render_ctx.context, self.map_w, self.map_h, format, &old_terrain));
                    self.needs_first_upload = true;
                    
                    self.gui_painter = Some(blade_egui::GuiPainter::new(s.info(), &self.render_ctx.context));
                    self.surface = Some(s);
                    
                    self.egui_ctx = egui::Context::default();
                    sow_ui::ui::theme::apply_theme(&self.egui_ctx);
                    log::info!("Successfully created surface on retry.");
                }
                Err(_) => {
                    // Still unavailable
                }
            }
        }

        let now = Instant::now();
        #[cfg(target_arch = "wasm32")]
                {
                    let doc_visible = web_sys::window()
                        .and_then(|w| w.document())
                        .map(|d| d.visibility_state() == web_sys::VisibilityState::Visible)
                        .unwrap_or(true);
                    if doc_visible && !self.wasm_doc_was_visible {
                        self.ws_reconnect_after_resume = true;
                    }
                    self.wasm_doc_was_visible = doc_visible;
                }

                if self.ws_reconnect_after_resume {
                    self.ws_reconnect_after_resume = false;
                    self.ws_connect_not_before = self.ws_connect_not_before.min(now);
                }
                // No fake map download simulation! Progress is real!

                while let Ok(res) = self.connect_rx.try_recv() {
                    match res {
                        Ok(client) => {
                            log::info!("Connected to server!");
                            self.net_client = Some(client);
                            self.app.main_menu_state.is_connected = true;
                            self.app.main_menu_state.is_connecting = false;
                            self.ws_connect_fail_backoff_ms = 400;
                        }
                        Err(e) => {
                            log::error!("Failed to connect: {}", e);
                            self.app.main_menu_state.is_connected = false;
                            self.app.main_menu_state.is_connecting = false;
                            self.ws_connect_fail_backoff_ms =
                                (self.ws_connect_fail_backoff_ms.saturating_mul(2)).min(30_000);
                            self.ws_connect_not_before =
                                now + Duration::from_millis(self.ws_connect_fail_backoff_ms);
                        }
                    }
                }

                let mut ws_disconnected = false;
                #[cfg(target_arch = "wasm32")]
                if let Some(c) = self.net_client.as_ref() {
                    if c.is_socket_closed() {
                        ws_disconnected = true;
                    }
                }

                // Process network messages
                if let Some(c) = self.net_client.as_ref() {
                    if !ws_disconnected {
                        loop {
                            let msg = match c.rx.try_recv() {
                                Ok(msg) => msg,
                                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                    ws_disconnected = true;
                                    break;
                                }
                            };

                        use sow_core::protocol::ServerMessage;
                        let server_msg = match bincode::deserialize::<ServerMessage>(&msg) {
                            Ok(m) => m,
                            Err(e) => {
                                log::warn!("[NET] Failed to deserialize server message ({} bytes): {}", msg.len(), e);
                                continue;
                            }
                        };

                        match server_msg {
                            ServerMessage::Start(start_msg) => {
                                log::info!("Received ServerStartMessage; entering Splash phase immediately");
                                self.app.phase = sow_ui::app::ClientPhase::Splash;
                                self.app.splash_state.job = sow_ui::ui::loading_screen::SplashJob::EnterGame;
                                self.app.splash_state.frames_drawn = 0;
                                self.app.main_menu_state.is_waiting = false;
                                self.app.main_menu_state.pending_join_lobby_id = None;
                                self.app.main_menu_state.joined_lobby_id = None;
                                self.my_player_id = start_msg.my_player_id;
                                self.engine_init_queued_msg = Some(*start_msg);
                            }
                            ServerMessage::Turn(turn_msg) => {
                                self.turn_queue.push_back(turn_msg.turn);
                                self.app.hud_state.sync_state = None;
                            }
                            ServerMessage::SyncState(sync_msg) => {
                                self.app.hud_state.sync_state = Some(sync_msg);
                            }
                            ServerMessage::Pong { client_time } => {
                                let rtt = self.start_time.elapsed().as_secs_f64() - client_time;
                                self.current_ping_ms = Some((rtt * 1000.0) as u32);
                            }
                            ServerMessage::LobbiesBroadcast(broadcast) => {

                                self.app.main_menu_state.lobbies = broadcast.lobbies.clone();

                                let maps_base = get_maps_url();
                                let (thumbs_to_fetch, maps_to_fetch) = self.app.asset_loader.get_assets_to_fetch(&self.app.main_menu_state.lobbies);
                                
                                for map_name in thumbs_to_fetch {
                                    let url = format!("{}/{}/thumbnail.webp", maps_base.trim_end_matches('/'), map_name);
                                    let tx = self.map_tx.clone();
                                    let map_name_for_closure = map_name.clone();
                                    let request = ehttp::Request::get(&url);
                                    ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
                                        if let Ok(res) = result {
                                            if res.ok {
                                                let _ = tx.send(MapDownloadEvent::ThumbnailReady(map_name_for_closure, res.bytes));
                                            }
                                        }
                                    });
                                }
                                
                                for map_name in maps_to_fetch {
                                    let url = format!("{}/{}/map.bin.br", maps_base.trim_end_matches('/'), map_name);
                                    let tx = self.map_tx.clone();
                                    let map_name_for_closure = map_name.clone();
                                    let request = ehttp::Request::get(&url);
                                    let accumulated = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                                    ehttp::streaming::fetch(request, move |result: ehttp::Result<ehttp::streaming::Part>| {
                                        match result {
                                            Ok(ehttp::streaming::Part::Response(res)) => {
                                                if !res.ok {
                                                    log::warn!("Prefetch failed for {}", map_name_for_closure);
                                                    return std::ops::ControlFlow::Break(());
                                                }
                                                std::ops::ControlFlow::Continue(())
                                            }
                                            Ok(ehttp::streaming::Part::Chunk(chunk)) => {
                                                if chunk.is_empty() {
                                                    let final_bytes = std::mem::take(&mut *accumulated.lock().unwrap());
                                                    let _ = tx.send(MapDownloadEvent::MapReady(map_name_for_closure.clone(), final_bytes));
                                                    return std::ops::ControlFlow::Break(());
                                                }
                                                accumulated.lock().unwrap().extend_from_slice(&chunk);
                                                std::ops::ControlFlow::Continue(())
                                            }
                                            Err(_) => std::ops::ControlFlow::Break(()),
                                        }
                                    });
                                }

                                if self.app.main_menu_state.is_waiting {
                                    let key = self.my_lobby_id
                                        .or(self.app.main_menu_state.joined_lobby_id)
                                        .or(self.app.main_menu_state.pending_join_lobby_id);
                                    if let Some(l_id) = key {
                                        if let Some(lobby) = broadcast.lobbies.iter().find(|l| l.id == l_id) {
                                            if lobby.is_counting_down {

                                                self.app.main_menu_state.wait_timer_secs = lobby.timer_secs;
                                            }
                                        }
                                    }
                                }
                            }
                            ServerMessage::LobbyClosed(closed) => {
                                log::warn!("Lobby {} closed: {}", closed.lobby_id, closed.reason);
                                self.app.hud_state.sync_state = None;
                                self.my_lobby_id = None;
                                self.my_player_id = None;

                                if closed.reason.contains("Requeueing") {
                                    log::info!("Auto-requeueing to a new lobby...");
                                    self.app.phase = ClientPhase::MainMenu;
                                    self.app.main_menu_state.is_waiting = true;
                                    let join_msg = sow_core::protocol::ClientMessage::Join {
                                        name: self.app.main_menu_state.player_name.clone(),
                                        is_observer: false,
                                        target_lobby_id: None,
                                        build_version: get_build_version(),
                                    };
                                    c.send(bincode::serialize(&join_msg).unwrap());
                                } else {
                                    self.app.phase = ClientPhase::Splash;
                                    self.app.splash_state.job = sow_ui::ui::loading_screen::SplashJob::ExitGame;
                                    self.app.splash_state.frames_drawn = 0;
                                    self.app.main_menu_state.is_waiting = false;
                                    self.app.main_menu_state.pending_join_lobby_id = None;
                                    self.app.main_menu_state.joined_lobby_id = None;
                                }
                            }
                            ServerMessage::JoinFailed(fail) => {
                                log::warn!("Join failed: {}", fail.reason);
                                if fail.reason == "VERSION_MISMATCH" {
                                    log::info!("Version mismatch — reloading...");
                                    #[cfg(target_arch = "wasm32")]
                                    if let Some(window) = web_sys::window() {
                                        let _ = window.location().reload();
                                    }
                                }
                                self.app.main_menu_state.is_waiting = false;
                                self.app.main_menu_state.pending_join_lobby_id = None;
                                self.app.main_menu_state.joined_lobby_id = None;
                            }
                            ServerMessage::JoinAck(ack) => {
                                log::info!("[LOBBY] Joined lobby {} as player {} (map: {})", ack.lobby_id, ack.player_id, ack.map_name);
                                self.my_lobby_id = Some(ack.lobby_id);
                                self.my_player_id = Some(ack.player_id);
                                self.app.main_menu_state.joined_lobby_id = Some(ack.lobby_id);
                                
                                let map_name = ack.map_name.clone();
                                self.app.main_menu_state.downloading_map_name = Some(map_name.clone());
                                
                                if let Some(texture) = self.app.asset_loader.thumbnail(&map_name) {
                                    self.app.splash_state.thumbnail = Some(texture.clone());
                                } else {
                                    self.app.splash_state.thumbnail = None;
                                }
                                
                                if self.app.asset_loader.has_map(&map_name) {
                                    log::info!("Map already cached, skipping download.");
                                    self.app.main_menu_state.cached_map = self.app.asset_loader.take_map(&map_name);
                                    self.app.main_menu_state.is_downloading_map = false;
                                    self.app.main_menu_state.map_download_progress = 100;
                                    c.send(bincode::serialize(&sow_core::protocol::ClientMessage::MapDownloadProgress {
                                        lobby_id: ack.lobby_id,
                                        player_id: ack.player_id,
                                        progress: 100,
                                    }).unwrap());
                                } else {
                                    let tx = self.map_tx.clone();
                                    self.app.main_menu_state.is_downloading_map = true;
                                    self.app.main_menu_state.cached_map = None;
                                    
                                    let maps_base = get_maps_url();
                                    let url = format!("{}/{}/map.bin.br", maps_base.trim_end_matches('/'), map_name);
                                    log::info!("Downloading map from: {}", url);
                                    
                                    c.send(bincode::serialize(&sow_core::protocol::ClientMessage::MapDownloadProgress {
                                        lobby_id: ack.lobby_id,
                                        player_id: ack.player_id,
                                        progress: 0,
                                    }).unwrap());
                                
                                    let request = ehttp::Request::get(&url);
                                    let map_name_for_closure = map_name.clone();
                                    let accumulated = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                                    let total_bytes = std::sync::Arc::new(std::sync::Mutex::new(0usize));
                                    
                                    ehttp::streaming::fetch(request, move |result: ehttp::Result<ehttp::streaming::Part>| {
                                        match result {
                                            Ok(ehttp::streaming::Part::Response(res)) => {
                                                if !res.ok {
                                                    log::error!("Failed to fetch map, HTTP {}", res.status);
                                                    let _ = tx.send(MapDownloadEvent::Error(format!("HTTP Error: {}", res.status)));
                                                    return std::ops::ControlFlow::Break(());
                                                }
                                                log::info!("Server map response ok! headers: {:?}", res.headers);
                                                let cl = res.headers.get("content-length").or_else(|| res.headers.get("Content-Length"));
                                                if let Some(cl) = cl {
                                                    if let Ok(len) = cl.parse::<usize>() {
                                                        *total_bytes.lock().unwrap() = len;
                                                        log::info!("Map content-length parsed as: {}", len);
                                                    } else {
                                                        log::warn!("Failed to parse content-length: {}", cl);
                                                    }
                                                } else {
                                                    log::warn!("No content-length header received!");
                                                }
                                                std::ops::ControlFlow::Continue(())
                                            }
                                            Ok(ehttp::streaming::Part::Chunk(chunk)) => {
                                                if chunk.is_empty() {
                                                    let final_bytes = std::mem::take(&mut *accumulated.lock().unwrap());
                                                    log::info!("Map fully downloaded: {} bytes", final_bytes.len());
                                                    let _ = tx.send(MapDownloadEvent::MapReady(map_name_for_closure.clone(), final_bytes));
                                                    return std::ops::ControlFlow::Break(());
                                                }
                                                let mut acc = accumulated.lock().unwrap();
                                                acc.extend_from_slice(&chunk);
                                                let downloaded = acc.len();
                                                let total = *total_bytes.lock().unwrap();
                                                if total > 0 {
                                                    let progress = ((downloaded as f64 / total as f64) * 100.0) as u8;
                                                    let _ = tx.send(MapDownloadEvent::Progress(map_name_for_closure.clone(), progress.min(99)));
                                                    if downloaded % 524288 < chunk.len() {
                                                        log::info!("Downloading map... {} / {} bytes ({}%)", downloaded, total, progress.min(99));
                                                    }
                                                } else if downloaded % 524288 < chunk.len() {
                                                    log::info!("Downloading map... {} bytes (unknown total)", downloaded);
                                                }
                                                std::ops::ControlFlow::Continue(())
                                            }
                                            Err(err) => {
                                                log::error!("Failed to fetch map: {}", err);
                                                let _ = tx.send(MapDownloadEvent::Error(format!("Fetch error: {}", err)));
                                                std::ops::ControlFlow::Break(())
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        } // end loop
                    } // end if !ws_disconnected
                } // end if let Some(c)
                
                if ws_disconnected {
                    log::warn!("WebSocket disconnected; will reconnect.");
                    self.net_client = None;
                    self.app.main_menu_state.is_connected = false;
                    self.app.main_menu_state.is_connecting = false;
                    self.ws_connect_not_before =
                        self.ws_connect_not_before.min(now + Duration::from_millis(200));
                        
                    // Recover: Send the user back to the loader
                    if self.app.phase != sow_ui::app::ClientPhase::Splash {
                        #[cfg(target_arch = "wasm32")]
                        {
                            if let Some(window) = web_sys::window() {
                                let _ = window.location().reload();
                            }
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            self.app.splash_state.job = sow_ui::ui::loading_screen::SplashJob::Reconnect;
                            self.app.splash_state.status_text = "Connection lost. Reconnecting...".to_string();
                            self.app.splash_state.progress = 0.0;
                            self.app.phase = sow_ui::app::ClientPhase::Splash;
                        }
                    }
                }

                #[cfg(target_arch = "wasm32")]
                let allow_ws_spawn = self.wasm_doc_was_visible;
                #[cfg(not(target_arch = "wasm32"))]
                let allow_ws_spawn = true;

                if allow_ws_spawn
                    && self.net_client.is_none()
                    && !self.app.main_menu_state.is_connecting
                    && now >= self.ws_connect_not_before
                {
                    self.app.main_menu_state.is_connecting = true;
                    let url = self.app.main_menu_state.server_address.clone();
                    #[cfg(target_arch = "wasm32")]
                    spawn_sow_client_connect(url, &self.connect_tx);
                    #[cfg(not(target_arch = "wasm32"))]
                    spawn_sow_client_connect(url, &self.connect_tx, &self.tokio_rt);
                }

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

                if let Some(start_msg) = self.engine_init_queued_msg.take() {
                    if self.app.main_menu_state.is_downloading_map {
                        self.engine_init_queued_msg = Some(start_msg);
                    } else {
                        log::info!("Map downloaded, computing heavy init in background");
                        
                        self.app.splash_state.status_text = "Computing terrain and water geometry...".to_string();
                        self.app.splash_state.progress = 0.1;

                        let cached_map = self.app.main_menu_state.cached_map.take();
                        let start_msg_clone = start_msg.clone();
                        let tx = self.engine_init_tx.clone();

                        let init_logic = move || {
                            let _ = tx.send(EngineInitEvent::Status("Decompressing map...".to_string()));
                            
                            let mut uncompressed_map = None;
                            if let Some(bytes) = cached_map {
                                let mut uncompressed = Vec::new();
                                let mut decompressor = brotli::Decompressor::new(bytes.as_slice(), 4096);
                                if std::io::Read::read_to_end(&mut decompressor, &mut uncompressed).is_ok() {
                                    uncompressed_map = Some(uncompressed);
                                } else {
                                    log::error!("Failed to decompress map.bin.br payload");
                                }
                            } else {
                                log::error!("Cached map data not found! Terrain will be empty.");
                            }
                            
                            let _ = tx.send(EngineInitEvent::Status("Computing terrain and water geometry...".to_string()));

                            let w = start_msg_clone.config.map_width;
                            let h = start_msg_clone.config.map_height;
                            let mut state = sow_core::game::GameState::new(
                                start_msg_clone.seed,
                                w,
                                h,
                                start_msg_clone.config.clone(),
                            );
                            
                            if let Some(bytes) = uncompressed_map {
                                if bytes.len() == state.map.terrain.len() {
                                    let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
                                    unsafe {
                                        std::ptr::copy_nonoverlapping(bytes.as_ptr(), dest_ptr, bytes.len());
                                    }
                                } else {
                                    log::error!("Map size mismatch! Expected {} bytes but decompressed {} bytes. Map will be randomly generated.", state.map.terrain.len(), bytes.len());
                                    for (i, &b) in bytes.iter().enumerate() {
                                        if i < state.map.terrain.len() {
                                            state.map.terrain[i] = sow_core::map::MapTile::from_byte(b);
                                        }
                                    }
                                }
                            }

                            let tx_prog = tx.clone();
                            let water = sow_core::water_components::WaterComponents::compute(&state.map, move |prog| {
                                let _ = tx_prog.send(EngineInitEvent::Progress(prog));
                            });
                            let _ = tx.send(EngineInitEvent::Complete(Box::new(state), water, Box::new(start_msg_clone)));
                        };

                        #[cfg(target_arch = "wasm32")]
                        init_logic();

                        #[cfg(not(target_arch = "wasm32"))]
                        std::thread::spawn(init_logic);

                        self.turn_queue.clear();
                        self.nameplate_cache.clear();
                        self.troop_label_throttle.clear();
                        self.current_snapshot = None;
                        self.needs_first_upload = true;
                    }
                }
                // Poll engine init channel
                if self.app.phase == sow_ui::app::ClientPhase::Splash {
                    match self.app.splash_state.job {
                        sow_ui::ui::loading_screen::SplashJob::Boot | sow_ui::ui::loading_screen::SplashJob::Reconnect => {
                            if self.app.main_menu_state.is_connected {
                                self.app.phase = ClientPhase::MainMenu;
                            } else {
                                self.app.splash_state.status_text = "Connecting to Server...".to_string();
                            }
                        }
                        sow_ui::ui::loading_screen::SplashJob::ExitGame => {
                            // Clean the engine state
                            let config = GameConfig::default();
                            self.bridge.send_command(SimCommand::Init {
                                config,
                                seed: 12345,
                                map_bytes: vec![],
                                players: vec![],
                            });
                            self.turn_queue.clear();
                            self.label_positions.clear();
                            self.nameplate_cache.clear();
                            self.troop_label_throttle.clear();
                            self.current_snapshot = None;
                            self.needs_first_upload = true;

                            
                            self.app.phase = ClientPhase::MainMenu;
                        }
                        sow_ui::ui::loading_screen::SplashJob::EnterGame => {
                            while let Ok(event) = self.engine_init_rx.try_recv() {
                                match event {
                                    EngineInitEvent::Status(msg) => {
                                        self.app.splash_state.status_text = msg;
                                    }
                                    EngineInitEvent::Progress(prog) => {
                                        self.app.splash_state.progress = prog;
                                    }
                                    EngineInitEvent::Complete(state, water, start_msg) => {
                                        log::info!("Engine initialization complete in background thread.");
                                        self.app.splash_state.status_text = "Allocating GPU Memory...".to_string();
                                        self.app.splash_state.progress = 0.95;
                                        self.app.splash_state.frames_drawn = 0; // Reset to ensure we draw the new text
                                        self.app.splash_state.gpu_load_step = 1;
                                        self.pending_engine_init_data = Some((*state, water, *start_msg));
                                    }
                                }
                            }
                        }
                    }

                    if self.app.splash_state.job == sow_ui::ui::loading_screen::SplashJob::EnterGame && self.pending_engine_init_data.is_some() {
                        let step = self.app.splash_state.gpu_load_step;
                        if step == 1 && self.app.splash_state.frames_drawn > 1 {
                            // Step 1: Allocate GPU Memory & Send Init Command
                            let (state, water, start_msg) = self.pending_engine_init_data.take().unwrap();
                            let map_bytes: Vec<u8> = state.map.terrain.iter().map(|t| t.as_byte()).collect();
                            
                            self.bridge.send_command(SimCommand::Init {
                                config: start_msg.config.clone(),
                                seed: start_msg.seed,
                                map_bytes: map_bytes.clone(),
                                players: start_msg.players.clone(),
                            });

                            self.map_w = start_msg.config.map_width;
                            self.map_h = start_msg.config.map_height;
                            if let Some(sp) = self.prev_sync_point.take() {
                                let _ = self.render_ctx.context.wait_for(&sp, !0);
                            }
                            if let Some(mut mr) = self.map_renderer.take() {
                                mr.destroy(&self.render_ctx); // MANDATORY MEMORY LEAK FIX
                            }
                            if let Some(ref s) = self.surface {
                                self.map_renderer = Some(sow_render::map_renderer::MapRenderer::new(&self.render_ctx.context, self.map_w, self.map_h, s.info().format, &map_bytes));
                                self.needs_first_upload = true;
                            }
                            
                            // Move to step 2: Texture uploading happens automatically next frame
                            self.app.splash_state.gpu_load_step = 2;
                            self.app.splash_state.frames_drawn = 0;
                            self.app.splash_state.progress = 0.98;
                            self.app.splash_state.status_text = "Uploading Map Texture...".to_string();
                            
                            // Re-insert pending data so we stay in this block until Step 4
                            self.pending_engine_init_data = Some((state, water, start_msg));
                        } else if step == 2 && !self.needs_first_upload {
                            // Step 2 Finished: GPU Texture is uploaded!
                            self.app.splash_state.gpu_load_step = 3;
                            self.app.splash_state.progress = 0.99;
                            self.app.splash_state.status_text = "Simulating Initial Expansions...".to_string();
                        }
                    }
                }

                self.app.hud_state.is_mobile = self.screen_w < 900.0;
                if let Some(snap) = &self.current_snapshot {
                    if let sow_core::game::GamePhase::Spawning { end_tick } = snap.phase {
                        let rem_ticks = end_tick.saturating_sub(snap.tick);
                        let target_secs = rem_ticks as f32 * 0.1; // assume 100ms
                        if let Some(ref mut current) = self.app.hud_state.spawn_timer_secs {
                            if (*current - target_secs).abs() > 0.3 {
                                *current = target_secs;
                            }
                        } else {
                            self.app.hud_state.spawn_timer_secs = Some(target_secs);
                        }
                    } else {
                        self.app.hud_state.spawn_timer_secs = None;
                    }
                } else {
                    self.app.hud_state.spawn_timer_secs = None;
                }
                if self.app.phase == sow_ui::app::ClientPhase::Playing {
                    if self.net_client.is_some() {
                        // Multiplayer: lockstep execution dictated by server
                        let mut ticks_processed = 0;
                        while let Some(turn) = self.turn_queue.pop_front() {
                            self.bridge.send_command(SimCommand::Turn(turn));
                            
                            // Update UI HUD State from my player id
                            if let Some(player) = self.current_snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.id == self.my_player_id.unwrap_or(1))) {
                                self.app.hud_state.gold = player.gold;
                                self.app.hud_state.troops = player.troops;
                                let owned_tiles = player.tile_count as f64;
                                self.app.hud_state.max_troops = owned_tiles * 50.0;
                            }

                            ticks_processed += 1;
                            if ticks_processed >= 10 {
                                break;
                            }
                        }
                        self.last_tick = now;
                    } else {
                        // Singleplayer: run freely based on local timer
                        if now.duration_since(self.last_tick) >= self.tick_interval {
                            self.bridge.send_command(SimCommand::Turn(sow_core::protocol::Turn { turn_number: 0, intents: vec![] }));

                            self.last_tick = now;
                            
                            if let Some(player) = self.current_snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.id == self.my_player_id.unwrap_or(1))) {
                                self.app.hud_state.gold = player.gold;
                                self.app.hud_state.troops = player.troops;
                                let owned_tiles = player.tile_count as f64;
                                self.app.hud_state.max_troops = owned_tiles * 50.0;
                            }
                        }
                    }
                } else {
                    self.last_tick = now;
                }
                if let Some(snap) = self.bridge.try_recv_snapshot() {
                    self.current_snapshot = Some(snap);
                }
                    
                if self.app.splash_state.gpu_load_step == 3 && self.current_snapshot.is_some() {
                    self.app.splash_state.gpu_load_step = 4;
                    self.app.phase = sow_ui::app::ClientPhase::Playing;
                    
                    // Clear pending init data to completely finish EnterGame phase
                    self.pending_engine_init_data = None;
                    log::info!("First snapshot received, releasing loader!");
                    
                    if let Some(pid) = self.my_player_id {
                        if let Some(snap) = &self.current_snapshot {
                            if let Some(player) = snap.players.iter().find(|p| p.id == pid) {
                                if player.tile_count > 0 && player.alive {
                                    let cx = player.centroid_x;
                                    let cy = player.centroid_y;
                                    self.camera_zoom = 1.5;
                                    self.camera_x = self.screen_w * 0.5 - cx * self.camera_zoom;
                                    self.camera_y = self.screen_h * 0.5 - cy * self.camera_zoom;
                                }
                            }
                        }
                    }

                    if let Some(c) = self.net_client.as_ref() {
                        if let (Some(lid), Some(pid)) = (self.my_lobby_id, self.my_player_id) {
                            let ready_msg = sow_core::protocol::ClientMessage::Ready { lobby_id: lid, player_id: pid };
                            let json = bincode::serialize(&ready_msg).unwrap();
                            c.send(json);
                        }
                    }
                }
                
                if let Some(win) = self.window.as_ref() {
                    win.request_redraw();
                }

    }
}
