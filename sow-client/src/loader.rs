use crate::app::SowApp;
use crate::EngineInitEvent;
use crate::hud::tutorial::TutorialStep;
use sow_core::game_config::GameConfig;
use sow_core::protocol::SimCommand;
use sow_ui::app::ClientPhase;

#[cfg(target_arch = "wasm32")]
use std::time::Duration;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// Empty splash status → loading screen shows i18n `loading_screen.loading` + progress %.
fn splash_show_loading(splash: &mut sow_ui::ui::loading_screen::SplashState) {
    splash.status_text.clear();
}

fn splash_show_loading_progress(
    splash: &mut sow_ui::ui::loading_screen::SplashState,
    progress: f32,
) {
    splash_show_loading(splash);
    splash.progress = splash.progress.max(progress);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn hide_web_loader() {
    if let Some(window) = web_sys::window() {
        let _ = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("hideWebLoader"))
            .and_then(|f| {
                if f.is_function() {
                    let func: js_sys::Function = f.unchecked_into();
                    let _ = func.call0(&wasm_bindgen::JsValue::NULL);
                }
                Ok(wasm_bindgen::JsValue::NULL)
            });
    }
}

impl SowApp {
    pub(crate) fn abort_engine_init(&mut self) {
        while self.tasks.engine_init_rx.try_recv().is_ok() {}
        self.tasks.engine_init_queued_msg = None;
        self.tasks.pending_engine_init_data = None;
    }

    fn release_client_game_gpu(&mut self) {
        if let Some(render_ctx) = self.gfx.render_ctx.as_mut() {
            if let Some(sp) = self.gfx.prev_sync_point.take() {
                let _ = render_ctx.context.wait_for(&sp, !0);
            }
            if let Some(mut mr) = self.gfx.map_renderer.take() {
                mr.destroy(render_ctx);
            }
            if let Some(mut mover) = self.gfx.mover_renderer.take() {
                mover.destroy(render_ctx);
            }
            render_ctx.reset_command_encoder();
        }
    }

    pub(crate) fn cleanup_game_session_stub(&mut self) {
        self.abort_engine_init();
        self.sim.turn_queue.clear();
        self.sim.offline_intents.clear();
        self.sim.last_synced_cost_tick = None;
        self.sim.my_lobby_id = None;
        self.sim.my_player_id = None;
        self.ui.app.hud_state.spawn_timer_secs = None;
        self.ui.app.hud_state.sync_state = None;
        self.ui.app.main_menu_state.wait_timer_secs = 0.0;
        self.ui.app.main_menu_state.cached_map = None;
        self.ui.app.main_menu_state.cached_map_key = None;
        self.net.pending_lobby_rejoin = false;

        let config = GameConfig {
            map_width: 1,
            map_height: 1,
            nation_count: 0,
            bot_count: 0,
            ..Default::default()
        };
        self.dispatch_sim_command(SimCommand::Init {
            config: Box::new(config),
            seed: 0,
            map_bytes: vec![0b10000000],
            players: vec![],
        });
        self.ui.label_positions.clear();
        self.ui.label_sizes.clear();
        self.sim.current_snapshot = None;
        self.gfx.needs_first_upload = true;
        self.release_client_game_gpu();
    }

    fn map_bytes_for_start(&mut self, map_name: &str) -> Option<Vec<u8>> {
        if let Some(bytes) = self.ui.app.asset_loader.take_map(map_name) {
            return Some(bytes);
        }
        if self.ui.app.main_menu_state.downloading_map_name.as_deref() == Some(map_name) {
            return self.ui.app.main_menu_state.cached_map.take();
        }
        None
    }

    pub fn update_loader(&mut self) {
        if let Some(start_msg) = self.tasks.engine_init_queued_msg.take() {
            let map_name = start_msg.config.map_name.clone();
            let has_map = self.ui.app.asset_loader.has_map(&map_name)
                || self.ui.app.main_menu_state.cached_map.is_some();

            if !has_map {
                self.tasks.engine_init_queued_msg = Some(start_msg);

                let pct = self.ui.app.main_menu_state.map_download_progress;
                log::debug!("Map download progress: {pct}%");
                splash_show_loading_progress(&mut self.ui.app.splash_state, pct as f32 / 100.0);
            } else {
                let cached_map = self.map_bytes_for_start(&map_name);
                log::info!(
                    "Map '{}' ready, computing heavy init in background",
                    map_name
                );

                splash_show_loading_progress(&mut self.ui.app.splash_state, 0.1);

                let mut start_msg_clone = start_msg.clone();

                let tx = self.tasks.engine_init_tx.clone();

                let init_logic = move || {
                    let parsed_map = cached_map.as_ref().and_then(|bytes| {
                        sow_core::maps::load_map_from_payload(bytes)
                            .map_err(|e| {
                                log::error!("Failed to parse map.bin: {e}");
                                e
                            })
                            .ok()
                    });

                    if cached_map.is_none() {
                        log::error!("Cached map data not found! Terrain will be empty.");
                    }

                    let (w, h) = if let Some(ref m) = parsed_map {
                        (m.width, m.height)
                    } else {
                        (
                            start_msg_clone.config.map_width,
                            start_msg_clone.config.map_height,
                        )
                    };
                    start_msg_clone.config.map_width = w;
                    start_msg_clone.config.map_height = h;

                    let mut state = sow_core::game::GameState::new(
                        start_msg_clone.seed,
                        w,
                        h,
                        start_msg_clone.config.clone(),
                    );

                    if let Some(map_file) = parsed_map {
                        state.total_land_tiles = map_file.num_land_tiles;
                        state.map_spawns = map_file.spawns;
                        if map_file.terrain.len() == state.map.terrain.len() {
                            let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
                            unsafe {
                                std::ptr::copy_nonoverlapping(
                                    map_file.terrain.as_ptr(),
                                    dest_ptr,
                                    map_file.terrain.len(),
                                );
                            }
                        } else {
                            log::error!(
                                "Terrain length mismatch: expected {}, got {}",
                                state.map.terrain.len(),
                                map_file.terrain.len()
                            );
                        }
                    }

                    let tx_prog = tx.clone();
                    let water = sow_core::water_components::WaterComponents::compute(
                        &state.map,
                        move |prog| {
                            let _ = tx_prog.send(EngineInitEvent::Progress(prog));
                        },
                    );
                    let _ = tx.send(EngineInitEvent::Complete(
                        Box::new(state),
                        water,
                        Box::new(start_msg_clone),
                    ));
                };

                #[cfg(target_arch = "wasm32")]
                init_logic();

                #[cfg(not(target_arch = "wasm32"))]
                std::thread::spawn(init_logic);

                self.sim.turn_queue.clear();

                self.sim.current_snapshot = None;
                self.gfx.needs_first_upload = true;
            }
        }
        // Poll engine init channel
        if self.ui.app.phase == sow_ui::app::ClientPhase::Splash {
            match self.ui.app.splash_state.job {
                sow_ui::ui::loading_screen::SplashJob::Boot => {
                    let leader = self.ui.app.main_menu_state.selected_leader;
                    let mobile = sow_ui::ui::theme::compact_viewport(&self.ui.egui_ctx);

                    splash_show_loading(&mut self.ui.app.splash_state);

                    self.ui
                        .app
                        .asset_loader
                        .ensure_ui_assets_loaded(&self.ui.egui_ctx);
                    let ui_ready = self.ui.app.asset_loader.ui_splash_ready();
                    if !ui_ready {
                        splash_show_loading_progress(&mut self.ui.app.splash_state, 0.35);
                    } else {
                        self.ui
                            .app
                            .asset_loader
                            .ensure_boot_leader_loaded(&self.ui.egui_ctx, leader);
                        self.ui
                            .app
                            .asset_loader
                            .set_leader_portrait_focus(leader, mobile);
                        let hero_ready = self.ui.app.asset_loader.boot_leader_ready(leader, mobile);
                        if !hero_ready {
                            splash_show_loading_progress(&mut self.ui.app.splash_state, 0.65);
                        } else {
                            splash_show_loading_progress(&mut self.ui.app.splash_state, 0.9);
                        }
                    }

                    let ui_ready = self.ui.app.asset_loader.ui_splash_ready();
                    let hero_ready = self.ui.app.asset_loader.boot_leader_ready(leader, mobile);
                    let boot_ready = ui_ready && hero_ready;

                    if boot_ready {
                        #[cfg(target_arch = "wasm32")]
                        {
                            if self.should_portal_auto_intro() {
                                if !self.boot_route_waiting {
                                    self.boot_route_waiting = true;
                                    self.boot_ready_since = Some(web_time::Instant::now());
                                }
                                splash_show_loading_progress(
                                    &mut self.ui.app.splash_state,
                                    0.95,
                                );
                                let timed_out = self
                                    .boot_ready_since
                                    .is_some_and(|t| t.elapsed() > Duration::from_millis(1500));
                                if self.boot_db_settled || timed_out {
                                    self.finish_boot_route();
                                }
                            } else {
                                self.finish_boot_to_main_menu();
                            }
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            self.ui.app.splash_state.done = true;
                            if self.ui.app.splash_state.target_phase.is_none() {
                                self.ui.app.splash_state.target_phase =
                                    Some(ClientPhase::MainMenu);
                            }
                        }
                    }
                }
                sow_ui::ui::loading_screen::SplashJob::ExitGame => {
                    let step = self.ui.app.splash_state.gpu_load_step;
                    if step == 0 {
                        log::info!("Exit game splash: reconnecting to orchestrator");
                        splash_show_loading_progress(&mut self.ui.app.splash_state, 0.2);
                        self.ui.app.splash_state.gpu_load_step = 1;
                        self.ui.app.splash_state.frames_drawn = 0;
                    } else if step == 1 {
                        // Wait for connection to orchestrator or timeout (3 seconds @ 60fps = 180 frames)
                        if self.net.client.is_some() || self.ui.app.splash_state.frames_drawn > 180
                        {
                            log::info!("Exit game splash: cleaning up game session");
                            splash_show_loading_progress(&mut self.ui.app.splash_state, 0.5);
                            self.ui.app.splash_state.gpu_load_step = 2;
                            self.ui.app.splash_state.frames_drawn = 0;
                        }
                    } else if step == 2 && self.ui.app.splash_state.frames_drawn > 1 {
                        self.cleanup_game_session_stub();
                        self.ui.app.splash_state.done = true;
                        if self.ui.app.splash_state.target_phase.is_none() {
                            self.ui.app.splash_state.target_phase = Some(ClientPhase::MainMenu);
                        }
                    }
                }
                sow_ui::ui::loading_screen::SplashJob::EnterGame => {
                    while let Ok(event) = self.tasks.engine_init_rx.try_recv() {
                        match event {
                            EngineInitEvent::Status(msg) => {
                                log::debug!("[loader] {msg}");
                                splash_show_loading(&mut self.ui.app.splash_state);
                            }
                            EngineInitEvent::Progress(prog) => {
                                self.ui.app.splash_state.progress = prog;
                            }
                            EngineInitEvent::Complete(state, water, start_msg) => {
                                if self.ui.app.splash_state.job
                                    != sow_ui::ui::loading_screen::SplashJob::EnterGame
                                {
                                    continue;
                                }
                                log::info!("Engine initialization complete; allocating GPU memory");
                                splash_show_loading_progress(&mut self.ui.app.splash_state, 0.95);
                                self.ui.app.splash_state.frames_drawn = 0;
                                self.ui.app.splash_state.gpu_load_step = 1;
                                self.tasks.pending_engine_init_data =
                                    Some((*state, water, *start_msg));
                            }
                        }
                    }
                }
            }

            if self.ui.app.splash_state.job == sow_ui::ui::loading_screen::SplashJob::EnterGame
                && self.tasks.pending_engine_init_data.is_some()
            {
                let step = self.ui.app.splash_state.gpu_load_step;
                if step == 1 && self.ui.app.splash_state.frames_drawn > 1 {
                    // Step 1: Allocate GPU Memory & Send Init Command
                    if let Some((state, water, start_msg)) =
                        self.tasks.pending_engine_init_data.take()
                    {
                        let map_bytes: Vec<u8> =
                            state.map.terrain.iter().map(|t| t.as_byte()).collect();

                        self.sim.current_snapshot = None; // MANDATORY: Clear old snapshot so Step 3 waits for the new one!

                        self.dispatch_sim_command(SimCommand::Init {
                            config: Box::new(start_msg.config.clone()),
                            seed: start_msg.seed,
                            map_bytes: map_bytes.clone(),
                            players: start_msg.players.clone(),
                        });

                        for turn in &start_msg.missed_turns {
                            self.dispatch_sim_command(SimCommand::Turn(turn.clone()));
                        }

                        self.sim.map_w = start_msg.config.map_width;
                        self.sim.map_h = start_msg.config.map_height;
                        if let Some(render_ctx) = self.gfx.render_ctx.as_mut() {
                            if let Some(sp) = self.gfx.prev_sync_point.take() {
                                let _ = render_ctx.context.wait_for(&sp, !0);
                            }
                            if let Some(mut mr) = self.gfx.map_renderer.take() {
                                mr.destroy(render_ctx);
                            }
                            if let Some(mut mover) = self.gfx.mover_renderer.take() {
                                mover.destroy(render_ctx);
                            }
                            if let Some(ref s) = self.gfx.surface {
                                let format = s.info().format;
                                self.gfx.map_renderer =
                                    Some(sow_render::map_renderer::MapRenderer::new(
                                        &render_ctx.context,
                                        self.sim.map_w,
                                        self.sim.map_h,
                                        format,
                                        &map_bytes,
                                    ));
                                self.gfx.mover_renderer = Some(sow_render::MoverRenderer::new(
                                    &render_ctx.context,
                                    format,
                                ));
                                self.gfx.needs_first_upload = true;
                            }
                        }

                        // Move to step 2: Texture uploading happens automatically next frame
                        self.ui.app.splash_state.gpu_load_step = 2;
                        self.ui.app.splash_state.frames_drawn = 0;
                        log::info!("Enter game splash: uploading map texture");
                        splash_show_loading_progress(&mut self.ui.app.splash_state, 0.98);

                        // Re-insert pending data so we stay in this block until Step 4
                        self.tasks.pending_engine_init_data = Some((state, water, start_msg));
                    }
                } else if step == 2 && !self.gfx.needs_first_upload {
                    // Step 2 Finished: GPU Texture is uploaded!
                    self.ui.app.splash_state.gpu_load_step = 3;
                    splash_show_loading_progress(&mut self.ui.app.splash_state, 0.99);
                } else if step == 3 && self.sim.current_snapshot.is_some() {
                    let ready_to_release = if self.net.is_offline {
                        true
                    } else {
                        self.ui.app.main_menu_state.is_connected
                            && self.net.client.is_some()
                            && self.ws_on_relay()
                    };

                    if ready_to_release {
                        self.ui.app.splash_state.gpu_load_step = 4;
                        self.ui.app.splash_state.done = true;
                        if self.ui.app.splash_state.target_phase.is_none() {
                            self.ui.app.splash_state.target_phase =
                                Some(sow_ui::app::ClientPhase::Playing);
                            crate::store_portals::gameplay_start();
                        }

                        // Clear pending init data to completely finish EnterGame phase
                        self.tasks.pending_engine_init_data = None;
                        log::info!("EnterGame load complete; fading out loader");

                        if !self.net.is_offline {
                            if let Some(c) = self.net.client.as_ref() {
                                if let (Some(lid), Some(pid)) =
                                    (self.sim.my_lobby_id, self.sim.my_player_id)
                                {
                                    let ready_msg = sow_core::protocol::ClientMessage::Ready {
                                        lobby_id: lid,
                                        player_id: pid,
                                    };
                                    let json = bincode::serialize(&ready_msg).unwrap();
                                    c.send(json);
                                }
                            }
                        }
                    } else {
                        splash_show_loading_progress(&mut self.ui.app.splash_state, 0.99);
                        if self.ui.app.splash_state.frames_drawn.is_multiple_of(120) {
                            log::warn!(
                                "[LOADER] Waiting for relay connection before releasing loader: is_connected={}, has_client={}, on_relay={}, my_lobby_id={:?}, my_player_id={:?}, phase={:?}",
                                self.ui.app.main_menu_state.is_connected,
                                self.net.client.is_some(),
                                self.ws_on_relay(),
                                self.sim.my_lobby_id,
                                self.sim.my_player_id,
                                self.ui.app.phase
                            );
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn finish_boot_to_main_menu(&mut self) {
        self.boot_route_waiting = false;
        self.ui.app.splash_state.done = true;
        self.ui.app.phase = ClientPhase::MainMenu;
        hide_web_loader();
        crate::store_portals::load_stop();
        crate::store_portals::gameplay_stop();
        self.web_loader_hidden = true;
    }

    #[cfg(target_arch = "wasm32")]
    fn finish_boot_route(&mut self) {
        self.boot_route_waiting = false;
        hide_web_loader();
        crate::store_portals::load_stop();
        crate::store_portals::gameplay_stop();
        self.web_loader_hidden = true;
        if self.progress.has_history() {
            log::info!("Portal boot: returning player → main menu");
            self.ui.app.splash_state.done = true;
            self.ui.app.phase = ClientPhase::MainMenu;
        } else {
            log::info!("Portal boot: new player → intro skirmish");
            self.start_portal_intro_match();
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn start_portal_intro_match(&mut self) {
        let config = GameConfig {
            map_name: sow_core::maps::DEFAULT_MAP_KEY.to_string(),
            bot_count: 0,
            nation_count: 0,
            seed: 42,
            player_leader: self.ui.app.main_menu_state.selected_leader,
            player_civilization: self.ui.app.main_menu_state.selected_civilization,
            ..Default::default()
        };
        self.start_offline_match(config, true);
    }

    pub(crate) fn start_offline_match(&mut self, mut config: GameConfig, tutorial: bool) {
        self.net.is_offline = true;
        self.sim.offline_tick_timer = 0.0;
        self.sim.offline_last_update = web_time::Instant::now();
        self.net.client = None;
        self.begin_enter_game_loader();
        self.sim.my_player_id = Some(1);
        self.sim.my_lobby_id = Some(0);
        self.ui.tutorial_active = tutorial;
        self.ui.tutorial_step = TutorialStep::Welcome;

        let map_id = sow_ui::ui::asset_loader::AssetLoader::map_key(&config.map_name);
        self.ui.app.main_menu_state.downloading_map_name = Some(map_id.clone());

        config.map_name = map_id.clone();
        if let Some(catalog) = &self.ui.app.asset_loader.map_catalog {
            if let Some(entry) = sow_core::maps::catalog_lookup(catalog, &map_id) {
                config.map_width = entry.width;
                config.map_height = entry.height;
                config.map_name = entry.key.clone();
            } else {
                log::warn!("Map '{}' not in catalog.bin", map_id);
            }
        }
        if let Some(payload) =
            sow_core::maps::load_map_br_payload(&map_id, crate::map_cache::load(&map_id))
        {
            if let Ok(map_file) = sow_core::maps::load_map_from_payload(&payload) {
                config.map_width = map_file.width;
                config.map_height = map_file.height;
                self.ui
                    .app
                    .asset_loader
                    .maps
                    .insert(map_id.clone(), payload);
            }
        }

        let start_msg = sow_core::protocol::ServerStartMessage {
            lobby_id: None,
            config: config.clone(),
            my_player_id: Some(1),
            seed: config.seed,
            players: vec![sow_core::protocol::PlayerInfo {
                id: 1,
                name: {
                    let name = &self.ui.app.main_menu_state.player_name;
                    let tag = &self.ui.app.main_menu_state.clan_tag;
                    if tag.is_empty() {
                        name.clone()
                    } else {
                        format!("[{}] {}", tag, name)
                    }
                },
                color: self.ui.app.main_menu_state.selected_leader.filler_rgb(),
                player_type: sow_core::player::PlayerType::Human,
                team: None,
                spawn_x: 0,
                spawn_y: 0,
                civilization: self.ui.app.main_menu_state.selected_civilization,
                leader: self.ui.app.main_menu_state.selected_leader,
            }],
            missed_turns: vec![],
            map_data: None,
            relay_port: None,
        };
        self.tasks.engine_init_queued_msg = Some(start_msg);

        if self.ui.app.asset_loader.has_map(&map_id) {
            self.ui.app.main_menu_state.cached_map =
                self.ui.app.asset_loader.take_map(&map_id);
            self.ui.app.main_menu_state.cached_map_key = Some(map_id.clone());
            self.ui.app.main_menu_state.is_downloading_map = false;
        } else {
            self.ui.app.main_menu_state.is_downloading_map = true;
            self.ui.app.main_menu_state.cached_map = None;
            self.ui.app.main_menu_state.cached_map_key = None;
            let maps_base = self.asset_config.maps_base.clone();
            let url = format!("{}/{}/map.bin.br", maps_base.trim_end_matches('/'), map_id);
            let tx = self.tasks.map_tx.clone();
            let request = ehttp::Request::get(&url);
            let map_name_for_closure = map_id.clone();
            let accumulated = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let total_bytes = std::sync::Arc::new(std::sync::Mutex::new(0usize));

            ehttp::streaming::fetch(
                request,
                move |result: ehttp::Result<ehttp::streaming::Part>| match result {
                    Ok(ehttp::streaming::Part::Response(res)) => {
                        if !res.ok {
                            let _ = tx.send(crate::MapDownloadEvent::Error(format!(
                                "HTTP Error: {}",
                                res.status
                            )));
                            return std::ops::ControlFlow::Break(());
                        }
                        let cl = res
                            .headers
                            .get("content-length")
                            .or_else(|| res.headers.get("Content-Length"));
                        if let Some(cl_str) = cl {
                            if let Ok(b) = cl_str.parse::<usize>() {
                                *total_bytes.lock().unwrap() = b;
                            }
                        }
                        std::ops::ControlFlow::Continue(())
                    }
                    Ok(ehttp::streaming::Part::Chunk(chunk)) => {
                        if chunk.is_empty() {
                            let final_bytes =
                                std::mem::take(&mut *accumulated.lock().unwrap());
                            let _ = tx.send(crate::MapDownloadEvent::MapReady(
                                map_name_for_closure.clone(),
                                final_bytes,
                            ));
                            return std::ops::ControlFlow::Break(());
                        }
                        let mut acc = accumulated.lock().unwrap();
                        acc.extend_from_slice(&chunk);
                        let total = *total_bytes.lock().unwrap();
                        let pct = if total > 0 {
                            ((acc.len() as f64 / total as f64) * 100.0) as u8
                        } else {
                            0
                        };
                        let _ = tx.send(crate::MapDownloadEvent::Progress(
                            map_name_for_closure.clone(),
                            pct,
                        ));
                        std::ops::ControlFlow::Continue(())
                    }
                    Err(e) => {
                        let _ = tx.send(crate::MapDownloadEvent::Error(e.to_string()));
                        std::ops::ControlFlow::Break(())
                    }
                },
            );
        }
    }
}
