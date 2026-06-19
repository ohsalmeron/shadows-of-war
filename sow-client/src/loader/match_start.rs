use crate::app::SowApp;
use crate::hud::tutorial::TutorialStep;
use sow_core::game_config::GameConfig;

impl SowApp {
    #[cfg(target_arch = "wasm32")]
    fn finish_boot_to_main_menu(&mut self) {
        self.boot_route_waiting = false;
        self.ui.app.splash_state.done = true;
        self.ui.app.phase = ClientPhase::MainMenu;
        hide_web_loader();
        crate::store_portals::load_stop();
        crate::store_portals::gameplay_stop();
        self.web_loader_hidden = true;
        if let Some(locale_str) = crate::store_portals::get_portal_locale() {
            let detected_lang = sow_i18n::Language::from_locale(&locale_str);
            self.ui.app.settings_state.language = detected_lang;
            log::info!(
                "Auto-configured language from portal locale: {:?} (source: {})",
                detected_lang,
                locale_str
            );
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn finish_boot_route(&mut self) {
        self.boot_route_waiting = false;
        crate::store_portals::load_stop();
        if let Some(locale_str) = crate::store_portals::get_portal_locale() {
            let detected_lang = sow_i18n::Language::from_locale(&locale_str);
            self.ui.app.settings_state.language = detected_lang;
            log::info!(
                "Auto-configured language from portal locale: {:?} (source: {})",
                detected_lang,
                locale_str
            );
        }
        if !self.progress.is_first_game() {
            log::info!("Portal boot: returning player -> main menu");
            hide_web_loader();
            self.web_loader_hidden = true;
            crate::store_portals::gameplay_stop();
            self.ui.app.splash_state.done = true;
            self.ui.app.phase = ClientPhase::MainMenu;
        } else {
            log::info!("Portal boot: new player -> intro skirmish");
            self.ui.app.main_menu_state.host_private_pending = false;
            self.start_portal_intro_match();
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn start_portal_intro_match(&mut self) {
        let tutorial_seed = web_time::SystemTime::now()
            .duration_since(web_time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let config = GameConfig {
            map_name: sow_core::maps::DEFAULT_MAP_KEY.to_string(),
            bot_count: 2,
            nation_count: 1,
            seed: tutorial_seed,
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
                log::debug!("Map '{}' not in catalog.bin", map_id);
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
            self.ui.app.main_menu_state.cached_map = self.ui.app.asset_loader.take_map(&map_id);
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
                            let final_bytes = std::mem::take(&mut *accumulated.lock().unwrap());
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
