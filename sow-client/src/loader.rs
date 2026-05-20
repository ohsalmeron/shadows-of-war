use crate::app::SowApp;
use crate::EngineInitEvent;
use sow_core::game_config::GameConfig;
use sow_core::protocol::SimCommand;
use sow_ui::app::ClientPhase;

impl SowApp {
    pub fn update_loader(&mut self) {
        if let Some(start_msg) = self.tasks.engine_init_queued_msg.take() {
            if self.ui.app.main_menu_state.is_downloading_map {
                self.tasks.engine_init_queued_msg = Some(start_msg);
            } else {
                log::info!("Map downloaded, computing heavy init in background");

                self.ui.app.splash_state.status_text =
                    "Computing terrain and water geometry...".to_string();
                self.ui.app.splash_state.progress = 0.1;

                let cached_map = self.ui.app.main_menu_state.cached_map.take();
                let cached_manifest = self.ui.app.main_menu_state.cached_manifest.take();
                let mut start_msg_clone = start_msg.clone();
                if let Some(man) = cached_manifest {
                    start_msg_clone.nations = man.nations;
                    start_msg_clone.config.map_width = man.map.width;
                    start_msg_clone.config.map_height = man.map.height;
                }
                
                let tx = self.tasks.engine_init_tx.clone();

                let init_logic = move || {
                    let _ = tx.send(EngineInitEvent::Status("Decompressing map...".to_string()));

                    let mut uncompressed_map = None;
                    if let Some(bytes) = cached_map {
                        let expected_len = (start_msg_clone.config.map_width
                            * start_msg_clone.config.map_height)
                            as usize;
                        if bytes.len() == expected_len {
                            log::info!(
                                "Map payload is already uncompressed (browser auto-decompression)"
                            );
                            uncompressed_map = Some(bytes);
                        } else {
                            let mut uncompressed = Vec::new();
                            let mut decompressor =
                                brotli::Decompressor::new(bytes.as_slice(), 4096);
                            if std::io::Read::read_to_end(&mut decompressor, &mut uncompressed)
                                .is_ok()
                            {
                                uncompressed_map = Some(uncompressed);
                            } else {
                                log::error!("Failed to decompress map.bin.br payload");
                            }
                        }
                    } else {
                        log::error!("Cached map data not found! Terrain will be empty.");
                    }

                    let _ = tx.send(EngineInitEvent::Status(
                        "Computing terrain and water geometry...".to_string(),
                    ));

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
                                std::ptr::copy_nonoverlapping(
                                    bytes.as_ptr(),
                                    dest_ptr,
                                    bytes.len(),
                                );
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
                self.ui.nameplate_cache.clear();
                self.ui.troop_label_throttle.clear();
                self.sim.current_snapshot = None;
                self.gfx.needs_first_upload = true;
            }
        }
        // Poll engine init channel
        if self.ui.app.phase == sow_ui::app::ClientPhase::Splash {
            match self.ui.app.splash_state.job {
                sow_ui::ui::loading_screen::SplashJob::Boot => {
                    if self.ui.app.main_menu_state.is_connected {
                        self.ui.app.phase = ClientPhase::MainMenu;
                    } else {
                        self.ui.app.splash_state.status_text =
                            "Connecting to Server...".to_string();
                    }
                }
                sow_ui::ui::loading_screen::SplashJob::ExitGame => {
                    let step = self.ui.app.splash_state.gpu_load_step;
                    if step == 0 {
                        self.ui.app.splash_state.status_text =
                            "Reconnecting to Orchestrator...".to_string();
                        self.ui.app.splash_state.progress = 0.2;
                        self.ui.app.splash_state.gpu_load_step = 1;
                        self.ui.app.splash_state.frames_drawn = 0;
                    } else if step == 1 {
                        // Wait for connection to orchestrator or timeout (3 seconds @ 60fps = 180 frames)
                        if self.net.client.is_some() || self.ui.app.splash_state.frames_drawn > 180
                        {
                            self.ui.app.splash_state.status_text =
                                "Cleaning up Game Session...".to_string();
                            self.ui.app.splash_state.progress = 0.5;
                            self.ui.app.splash_state.gpu_load_step = 2;
                            self.ui.app.splash_state.frames_drawn = 0;
                        }
                    } else if step == 2 && self.ui.app.splash_state.frames_drawn > 1 {
                        // Clean the engine state
                        let mut config = GameConfig::default();
                        config.map_width = 1;
                        config.map_height = 1;
                        config.nation_count = 0;
                        config.bot_count = 0;
                        self.dispatch_sim_command(SimCommand::Init {
                            config: Box::new(config),
                            seed: 0,
                            map_bytes: vec![0b10000000], // 1 land tile
                            players: vec![],
                            nations: None,
                        });
                        self.sim.turn_queue.clear();
                        self.ui.label_positions.clear();
                        self.ui.nameplate_cache.clear();
                        self.ui.troop_label_throttle.clear();
                        self.sim.current_snapshot = None;
                        self.gfx.needs_first_upload = true;

                        // Free GPU memory
                        if let Some(sp) = self.gfx.prev_sync_point.take() {
                            let _ = self.gfx.render_ctx.context.wait_for(&sp, !0);
                        }
                        if let Some(mut mr) = self.gfx.map_renderer.take() {
                            mr.destroy(&self.gfx.render_ctx);
                        }

                        self.ui.app.phase = ClientPhase::MainMenu;
                    }
                }
                sow_ui::ui::loading_screen::SplashJob::EnterGame => {
                    while let Ok(event) = self.tasks.engine_init_rx.try_recv() {
                        match event {
                            EngineInitEvent::Status(msg) => {
                                self.ui.app.splash_state.status_text = msg;
                            }
                            EngineInitEvent::Progress(prog) => {
                                self.ui.app.splash_state.progress = prog;
                            }
                            EngineInitEvent::Complete(state, water, start_msg) => {
                                log::info!("Engine initialization complete in background thread.");
                                self.ui.app.splash_state.status_text =
                                    "Allocating GPU Memory...".to_string();
                                self.ui.app.splash_state.progress = 0.95;
                                self.ui.app.splash_state.frames_drawn = 0; // Reset to ensure we draw the new text
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
                    let (state, water, start_msg) =
                        self.tasks.pending_engine_init_data.take().unwrap();
                    let map_bytes: Vec<u8> =
                        state.map.terrain.iter().map(|t| t.as_byte()).collect();

                    self.sim.current_snapshot = None; // MANDATORY: Clear old snapshot so Step 3 waits for the new one!

                    self.dispatch_sim_command(SimCommand::Init {
                        config: Box::new(start_msg.config.clone()),
                        seed: start_msg.seed,
                        map_bytes: map_bytes.clone(),
                        players: start_msg.players.clone(),
                        nations: start_msg.nations.clone(),
                    });

                    for turn in &start_msg.missed_turns {
                        self.dispatch_sim_command(SimCommand::Turn(turn.clone()));
                    }

                    self.sim.map_w = start_msg.config.map_width;
                    self.sim.map_h = start_msg.config.map_height;
                    if let Some(sp) = self.gfx.prev_sync_point.take() {
                        let _ = self.gfx.render_ctx.context.wait_for(&sp, !0);
                    }
                    if let Some(mut mr) = self.gfx.map_renderer.take() {
                        mr.destroy(&self.gfx.render_ctx); // MANDATORY MEMORY LEAK FIX
                    }
                    if let Some(ref s) = self.gfx.surface {
                        self.gfx.map_renderer = Some(sow_render::map_renderer::MapRenderer::new(
                            &self.gfx.render_ctx.context,
                            self.sim.map_w,
                            self.sim.map_h,
                            s.info().format,
                            &map_bytes,
                        ));
                        self.gfx.needs_first_upload = true;
                    }

                    // Move to step 2: Texture uploading happens automatically next frame
                    self.ui.app.splash_state.gpu_load_step = 2;
                    self.ui.app.splash_state.frames_drawn = 0;
                    self.ui.app.splash_state.progress = 0.98;
                    self.ui.app.splash_state.status_text = "Uploading Map Texture...".to_string();

                    // Re-insert pending data so we stay in this block until Step 4
                    self.tasks.pending_engine_init_data = Some((state, water, start_msg));
                } else if step == 2 && !self.gfx.needs_first_upload {
                    // Step 2 Finished: GPU Texture is uploaded!
                    self.ui.app.splash_state.gpu_load_step = 3;
                    self.ui.app.splash_state.progress = 0.99;
                    self.ui.app.splash_state.status_text =
                        "Simulating Initial Expansions...".to_string();
                }
            }
        }
    }
}
