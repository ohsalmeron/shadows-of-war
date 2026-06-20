use super::state::SowApp;

impl SowApp {
    fn apply_platform_auth(request: &mut ehttp::Request) {
        if let Some(token) = crate::store_portals::load_identity("Player")
            .auth_token
            .filter(|t| !t.is_empty())
        {
            request.headers.insert("X-Platform-Auth", token);
        }
    }

    pub(crate) fn fetch_cloud_progress(&self) {
        let (provider, ext_id) = crate::store_portals::database_identity("Player");
        let display_name = crate::store_portals::load_identity("Player").display_name;
        let db_url = self.asset_config.database_base.clone();

        let encoded_provider =
            url::form_urlencoded::byte_serialize(provider.as_bytes()).collect::<String>();
        let encoded_id =
            url::form_urlencoded::byte_serialize(ext_id.as_bytes()).collect::<String>();
        let encoded_name =
            url::form_urlencoded::byte_serialize(display_name.as_bytes()).collect::<String>();

        let url = format!(
            "{}/profile?provider={}&external_id={}&fallback_name={}",
            db_url.trim_end_matches('/'),
            encoded_provider,
            encoded_id,
            encoded_name
        );

        log::info!("Fetching profile from sow-database: {provider}/{ext_id}");
        let tx = self.tasks.db_tx.clone();
        let profile_provider = provider.clone();
        let mut request = ehttp::Request::get(&url);
        Self::apply_platform_auth(&mut request);

        ehttp::fetch(
            request,
            move |result: ehttp::Result<ehttp::Response>| match result {
                Ok(res) => {
                    if res.ok {
                        #[derive(serde::Deserialize)]
                        struct DbAccount {
                            id: String,
                            profile: crate::player_progress::PlayerProgress,
                        }
                        match serde_json::from_slice::<DbAccount>(&res.bytes) {
                            Ok(account) => {
                                let _ = tx.send(crate::player_progress::DbEvent::ProfileLoaded {
                                    progress: account.profile,
                                    account_id: account.id,
                                    provider: profile_provider,
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to parse database profile JSON: {}", e);
                                let _ = tx.send(crate::player_progress::DbEvent::LoadFailed);
                            }
                        }
                    } else {
                        log::warn!("sow-database responded with HTTP {}", res.status);
                        let _ = tx.send(crate::player_progress::DbEvent::LoadFailed);
                    }
                }
                Err(e) => {
                    log::error!("sow-database request failed: {}", e);
                    let _ = tx.send(crate::player_progress::DbEvent::LoadFailed);
                }
            },
        );
    }

    pub(crate) fn resolve_link_conflict(&self, keep_account_id: String) {
        let Some(conflict) = self.ui.app.main_menu_state.active_conflict.clone() else {
            return;
        };

        let db_url = self.asset_config.database_base.clone();
        let url = format!("{}/profile/link/resolve", db_url.trim_end_matches('/'));
        #[derive(serde::Serialize)]
        struct ResolveRequest {
            account_id: String,
            keep_account_id: String,
            target_provider: String,
            target_external_id: String,
        }
        let resolved_provider = conflict.target_provider.clone();
        let payload = ResolveRequest {
            account_id: conflict.current_account_id,
            keep_account_id,
            target_provider: resolved_provider.clone(),
            target_external_id: conflict.target_external_id,
        };
        let Ok(body) = serde_json::to_vec(&payload) else {
            return;
        };
        let tx = self.tasks.db_tx.clone();
        let mut request = ehttp::Request::post(&url, body);
        request.headers.insert("Content-Type", "application/json");
        Self::apply_platform_auth(&mut request);
        log::info!("Resolving platform link conflict...");
        ehttp::fetch(request, move |result| {
            let Ok(res) = result else {
                log::error!("Link resolve request failed");
                return;
            };
            if !res.ok {
                log::warn!("Profile link resolve returned HTTP {}", res.status);
                return;
            }
            #[derive(serde::Deserialize)]
            struct DbAccount {
                id: String,
                profile: crate::player_progress::PlayerProgress,
            }
            #[derive(serde::Deserialize)]
            struct ResolveResponse {
                status: String,
                account: Option<DbAccount>,
            }
            let Ok(parsed) = serde_json::from_slice::<ResolveResponse>(&res.bytes) else {
                log::error!("Failed to parse link resolve response");
                return;
            };
            if parsed.status == "resolved" {
                if let Some(account) = parsed.account {
                    let _ = tx.send(crate::player_progress::DbEvent::LinkResolved {
                        progress: account.profile,
                        account_id: account.id,
                        provider: resolved_provider,
                    });
                }
            }
        });
    }

    pub(crate) fn maybe_link_platform_identity(&self) {
        let Some(account_id) = &self.progress_account_id else {
            return;
        };
        let platform = crate::store_portals::load_identity("Player");
        let Some(ext_id) = platform.external_id.filter(|s| !s.is_empty()) else {
            return;
        };
        if platform.provider == "local" || platform.provider == "self" {
            return;
        }

        let db_url = self.asset_config.database_base.clone();
        let url = format!("{}/profile/link", db_url.trim_end_matches('/'));
        #[derive(serde::Serialize)]
        struct LinkRequest {
            account_id: String,
            target_provider: String,
            target_external_id: String,
        }
        let current_account_id = account_id.clone();
        let target_provider = platform.provider.to_string();
        let target_external_id = ext_id.clone();
        let payload = LinkRequest {
            account_id: current_account_id.clone(),
            target_provider: target_provider.clone(),
            target_external_id: target_external_id.clone(),
        };
        let Ok(body) = serde_json::to_vec(&payload) else {
            return;
        };
        let tx = self.tasks.db_tx.clone();
        let mut request = ehttp::Request::post(&url, body);
        request.headers.insert("Content-Type", "application/json");
        Self::apply_platform_auth(&mut request);
        log::info!(
            "Attempting to link platform {} to account {}",
            platform.provider,
            account_id
        );
        ehttp::fetch(request, move |result| {
            let Ok(res) = result else { return };
            if !res.ok {
                log::warn!("Profile link returned HTTP {}", res.status);
                return;
            }
            #[derive(serde::Deserialize)]
            struct LinkSummary {
                level: u32,
            }
            #[derive(serde::Deserialize)]
            struct LinkResponse {
                status: String,
                existing_account_id: Option<String>,
                existing: Option<LinkSummary>,
                current: Option<LinkSummary>,
            }
            let Ok(parsed) = serde_json::from_slice::<LinkResponse>(&res.bytes) else {
                return;
            };
            match parsed.status.as_str() {
                "linked" | "resolved" => {
                    log::info!("Platform identity linked successfully");
                }
                "conflict" => {
                    if let (Some(existing_id), Some(existing), Some(current)) =
                        (parsed.existing_account_id, parsed.existing, parsed.current)
                    {
                        log::warn!(
                            "Account link conflict: local level {} vs existing level {} (id {})",
                            current.level,
                            existing.level,
                            existing_id
                        );
                        let _ = tx.send(crate::player_progress::DbEvent::LinkConflict(
                            crate::player_progress::LinkConflictInfo {
                                current_account_id: current_account_id.clone(),
                                existing_account_id: existing_id,
                                existing_level: existing.level,
                                current_level: current.level,
                                target_provider,
                                target_external_id,
                            },
                        ));
                    }
                }
                other => log::warn!("Unexpected link status: {other}"),
            }
        });
    }

    pub(crate) fn apply_progress_preferences(&mut self) {
        if let Some(leader) = self.progress.preferred_leader {
            self.ui.app.main_menu_state.selected_leader = leader;
            self.ui.app.main_menu_state.selected_civilization = leader.civilization();
        }
    }

    pub(crate) fn apply_cloud_profile(
        &mut self,
        cloud: crate::player_progress::PlayerProgress,
        account_id: String,
        provider: String,
    ) {
        let portal = self.progress.clone();
        self.progress.merge_boot_profile(cloud);
        self.progress_account_id = Some(account_id);
        self.progress_provider = provider;
        if !self.progress.has_history() && portal.has_history() {
            self.progress = portal;
        }
        self.apply_progress_preferences();
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn should_portal_auto_intro(&self) -> bool {
        if !crate::store_portals::is_portal_embed() {
            return false;
        }
        let mm = &self.ui.app.main_menu_state;
        if self.progress.is_first_game() {
            // A real invite link should still bypass the intro, but instant-MP host intent should not.
            mm.pending_join_lobby_id.is_none()
        } else {
            mm.pending_join_lobby_id.is_none() && !mm.host_private_pending
        }
    }

    pub(crate) fn save_local_progress(&self) {
        crate::store_portals::save_portal_progress(&self.progress);
    }
}
