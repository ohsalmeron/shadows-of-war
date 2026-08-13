use super::state::SowApp;

fn fetch_anonymous_profile_request(
    url: String,
    account_id: Option<String>,
    tx: crossbeam_channel::Sender<crate::player_progress::DbEvent>,
    reset_stale_id: bool,
) {
    #[derive(serde::Serialize)]
    struct AnonymousProfileRequest {
        account_id: Option<String>,
    }

    let payload = AnonymousProfileRequest {
        account_id: account_id.clone(),
    };
    let Ok(body) = serde_json::to_vec(&payload) else {
        log::error!("Failed to serialize anonymous profile request");
        return;
    };
    let mut request = ehttp::Request::post(&url, body);
    request
        .headers
        .insert("Content-Type", "application/json");
    log::info!("Fetching canonical anonymous account from sow-database");
    ehttp::fetch(request, move |result| match result {
        Ok(res) if res.ok => {
            #[derive(serde::Deserialize)]
            struct DbAccount {
                id: String,
                profile: crate::player_progress::PlayerProgress,
            }
            match serde_json::from_slice::<DbAccount>(&res.bytes) {
                Ok(account) => {
                    crate::anonymous_identity::save_account_id(&account.id);
                    let _ = tx.send(crate::player_progress::DbEvent::ProfileLoaded {
                        progress: account.profile,
                        account_id: account.id,
                        provider: "anonymous".to_string(),
                    });
                }
                Err(error) => {
                    log::error!("Failed to parse anonymous profile JSON: {error}");
                    let _ = tx.send(crate::player_progress::DbEvent::LoadFailed);
                }
            }
        }
        Ok(res) => {
            if res.status == 404 && reset_stale_id && account_id.is_some() {
                log::warn!(
                    "Stored anonymous account was not found; issuing a fresh canonical account"
                );
                crate::anonymous_identity::clear_account_id();
                fetch_anonymous_profile_request(url, None, tx, false);
                return;
            }
            log::warn!(
                "sow-database anonymous profile responded with HTTP {}",
                res.status
            );
            let _ = tx.send(crate::player_progress::DbEvent::LoadFailed);
        }
        Err(error) => {
            log::error!("Anonymous profile request failed: {error}");
            let _ = tx.send(crate::player_progress::DbEvent::LoadFailed);
        }
    });
}

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
        let identity = crate::store_portals::load_identity("Player");
        let provider = identity.provider.to_string();
        let Some(ext_id) = identity
            .external_id
            .clone()
            .filter(|id| !id.is_empty())
            .filter(|_| provider == "crazygames")
        else {
            self.fetch_anonymous_progress();
            return;
        };
        let db_url = self.asset_config.database_base.clone();

        let encoded_provider =
            url::form_urlencoded::byte_serialize(provider.as_bytes()).collect::<String>();
        let encoded_id =
            url::form_urlencoded::byte_serialize(ext_id.as_bytes()).collect::<String>();
        let url = format!(
            "{}/profile?provider={}&external_id={}",
            db_url.trim_end_matches('/'),
            encoded_provider,
            encoded_id
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

    fn fetch_anonymous_progress(&self) {
        let url = format!(
            "{}/profile/anonymous",
            self.asset_config.database_base.trim_end_matches('/')
        );
        let tx = self.tasks.db_tx.clone();
        fetch_anonymous_profile_request(
            url,
            crate::anonymous_identity::load_account_id(),
            tx,
            true,
        );
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
