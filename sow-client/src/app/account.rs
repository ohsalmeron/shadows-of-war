use super::state::SowApp;

fn account_hint(account_id: Option<&str>) -> String {
    account_id
        .map(|id| id.chars().take(8).collect::<String>())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| "none".to_string())
}

fn fetch_anonymous_profile_request(
    url: String,
    account_id: Option<String>,
    requested_display_name: Option<String>,
    tx: crossbeam_channel::Sender<crate::player_progress::DbEvent>,
    reset_stale_id: bool,
    request_id: u64,
) {
    #[derive(serde::Serialize)]
    struct AnonymousProfileRequest {
        account_id: Option<String>,
        display_name: Option<String>,
        auth_secret: Option<String>,
    }

    let retry_display_name = requested_display_name.clone();
    let payload = AnonymousProfileRequest {
        account_id: account_id.clone(),
        display_name: requested_display_name,
        auth_secret: crate::anonymous_identity::load_account_secret(),
    };
    let Ok(body) = serde_json::to_vec(&payload) else {
        log::error!(
            "[identity] profile request id={request_id} serialize_failed account={}",
            account_hint(account_id.as_deref())
        );
        let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
            request_id,
            status: None,
        });
        return;
    };
    let mut request = ehttp::Request::post(&url, body);
    request.headers.insert("Content-Type", "application/json");
    request
        .headers
        .insert("X-SOW-Identity-Request", request_id.to_string());
    log::info!(
        "[identity] profile request id={request_id} start account={} reset_stale_id={reset_stale_id}",
        account_hint(account_id.as_deref())
    );
    ehttp::fetch(request, move |result| match result {
        Ok(res) if res.ok => {
            #[derive(serde::Deserialize)]
            struct DbAccount {
                id: String,
                #[serde(default)]
                public_id: Option<String>,
                #[serde(default)]
                display_name: String,
                profile: crate::player_progress::PlayerProgress,
                /// One-time ownership secret, present only when just minted.
                #[serde(default)]
                auth_secret: Option<String>,
            }
            match serde_json::from_slice::<DbAccount>(&res.bytes) {
                Ok(account) => {
                    crate::anonymous_identity::save_account_id(&account.id);
                    if let Some(secret) = account.auth_secret.as_deref()
                        && !secret.is_empty()
                    {
                        crate::anonymous_identity::save_account_secret(secret);
                        log::info!(
                            "[identity] account secret minted and stored account={}",
                            account_hint(Some(&account.id))
                        );
                    }
                    log::info!(
                        "[identity] profile request id={request_id} ack account={} name_len={}",
                        account_hint(Some(&account.id)),
                        account.display_name.chars().count()
                    );
                    let _ = tx.send(crate::player_progress::DbEvent::ProfileLoaded {
                        progress: account.profile,
                        account_id: account.id,
                        public_id: account.public_id,
                        display_name: account.display_name,
                        provider: "anonymous".to_string(),
                        request_id,
                    });
                }
                Err(error) => {
                    log::error!("[identity] profile request id={request_id} parse_failed: {error}");
                    let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                        request_id,
                        status: Some(res.status),
                    });
                }
            }
        }
        Ok(res) => {
            if res.status == 404 && reset_stale_id && account_id.is_some() {
                log::warn!(
                    "[identity] profile request id={request_id} missing_account account={} action=create_replacement",
                    account_hint(account_id.as_deref())
                );
                crate::anonymous_identity::clear_account_id();
                // The current UI name is the only client-side presentation value. The
                // server persists it with the newly issued account ID.
                fetch_anonymous_profile_request(
                    url,
                    None,
                    retry_display_name,
                    tx,
                    false,
                    request_id,
                );
                return;
            }
            log::warn!(
                "[identity] profile request id={request_id} failed status={} account={}",
                res.status,
                account_hint(account_id.as_deref())
            );
            let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                request_id,
                status: Some(res.status),
            });
        }
        Err(error) => {
            log::error!("[identity] profile request id={request_id} network_failed: {error}");
            let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                request_id,
                status: None,
            });
        }
    });
}

impl SowApp {
    fn next_identity_request_id(&mut self) -> u64 {
        self.identity_request_seq = self.identity_request_seq.wrapping_add(1);
        if self.identity_request_seq == 0 {
            self.identity_request_seq = 1;
        }
        self.identity_request_seq
    }

    fn apply_platform_auth(request: &mut ehttp::Request) {
        if let Some(token) = crate::store_portals::load_identity("Player")
            .auth_token
            .filter(|t| !t.is_empty())
        {
            request.headers.insert("X-Platform-Auth", token);
        }
    }

    pub(crate) fn fetch_cloud_progress(&mut self) {
        if self.display_name_save_in_flight {
            self.profile_refresh_pending = true;
            log::debug!(
                "[identity] profile refresh queued behind rename request; account={}",
                account_hint(self.progress_account_id.as_deref())
            );
            return;
        }
        if self.profile_request_in_flight {
            self.profile_refresh_pending = true;
            log::debug!("[identity] profile refresh coalesced behind in-flight request");
            return;
        }
        let identity = crate::store_portals::load_identity("Player");
        let provider = identity.provider.to_string();
        let android_twa = crate::store_portals::is_android_twa();
        let Some(ext_id) = identity
            .external_id
            .clone()
            .filter(|id| !id.is_empty())
            .filter(|_| matches!(provider.as_str(), "crazygames" | "wou" | "playgames"))
        else {
            if android_twa {
                log::error!("[identity] Play Games identity required on Android");
                return;
            }
            self.fetch_anonymous_progress();
            return;
        };
        let request_id = self.next_identity_request_id();
        self.profile_request_in_flight = true;
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

        log::info!(
            "[identity] profile request id={request_id} start provider={provider} external_id_len={}",
            ext_id.chars().count()
        );
        let tx = self.tasks.db_tx.clone();
        let profile_provider = provider.clone();
        let mut request = ehttp::Request::get(&url);
        Self::apply_platform_auth(&mut request);
        request
            .headers
            .insert("X-SOW-Identity-Request", request_id.to_string());

        ehttp::fetch(
            request,
            move |result: ehttp::Result<ehttp::Response>| match result {
                Ok(res) => {
                    if res.ok {
                        #[derive(serde::Deserialize)]
                        struct DbAccount {
                            id: String,
                            #[serde(default)]
                            public_id: Option<String>,
                            #[serde(default)]
                            display_name: String,
                            profile: crate::player_progress::PlayerProgress,
                        }
                        match serde_json::from_slice::<DbAccount>(&res.bytes) {
                            Ok(account) => {
                                let _ = tx.send(crate::player_progress::DbEvent::ProfileLoaded {
                                    progress: account.profile,
                                    account_id: account.id,
                                    public_id: account.public_id,
                                    display_name: account.display_name,
                                    provider: profile_provider,
                                    request_id,
                                });
                            }
                            Err(e) => {
                                log::error!(
                                    "[identity] profile request id={request_id} parse_failed: {e}"
                                );
                                let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                                    request_id,
                                    status: Some(res.status),
                                });
                            }
                        }
                    } else {
                        log::warn!(
                            "[identity] profile request id={request_id} failed status={}",
                            res.status
                        );
                        let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                            request_id,
                            status: Some(res.status),
                        });
                    }
                }
                Err(e) => {
                    log::error!("[identity] profile request id={request_id} network_failed: {e}");
                    let _ = tx.send(crate::player_progress::DbEvent::LoadFailed {
                        request_id,
                        status: None,
                    });
                }
            },
        );
    }

    fn fetch_anonymous_progress(&mut self) {
        let request_id = self.next_identity_request_id();
        self.profile_request_in_flight = true;
        let url = format!(
            "{}/profile/anonymous",
            self.asset_config.database_base.trim_end_matches('/')
        );
        let tx = self.tasks.db_tx.clone();
        fetch_anonymous_profile_request(
            url,
            crate::anonymous_identity::load_account_id(),
            Some(self.ui.app.main_menu_state.player_name.clone()),
            tx,
            true,
            request_id,
        );
    }

    pub(crate) fn save_display_name(&mut self, display_name: String) {
        let display_name = display_name.trim().to_string();
        if display_name.is_empty() {
            log::warn!("Refusing to save an empty display name");
            return;
        }
        if self.profile_request_in_flight {
            self.queued_display_name = Some(display_name);
            log::debug!(
                "[identity] rename queued behind profile request account={}",
                account_hint(self.progress_account_id.as_deref())
            );
            return;
        }
        if self.display_name_save_in_flight {
            self.queued_display_name = Some(display_name);
            log::debug!("[identity] newer rename queued behind in-flight rename");
            return;
        }
        if self.progress_provider != "anonymous" {
            if self.progress_provider == "local" && self.progress_account_id.is_none() {
                self.pending_display_name = Some(display_name);
                log::debug!("Queued anonymous display-name update until the account is created");
            } else {
                log::debug!(
                    "Skipping anonymous display-name save for provider {}",
                    self.progress_provider
                );
            }
            return;
        }
        let Some(account_id) = self.progress_account_id.clone() else {
            self.pending_display_name = Some(display_name);
            log::warn!("Cannot save display name before anonymous account is loaded");
            return;
        };
        let request_id = self.next_identity_request_id();
        let account_hint_value = account_hint(Some(&account_id));
        let requested_name_len = display_name.chars().count();
        self.display_name_save_in_flight = true;
        let url = format!(
            "{}/profile/anonymous/name",
            self.asset_config.database_base.trim_end_matches('/')
        );
        #[derive(serde::Serialize)]
        struct RenameRequest {
            account_id: String,
            display_name: String,
            auth_secret: String,
        }
        #[derive(serde::Deserialize)]
        struct DbAccount {
            id: String,
            #[serde(default)]
            display_name: String,
        }
        let Some(auth_secret) = crate::anonymous_identity::load_account_secret() else {
            log::error!(
                "[identity] rename request id={request_id} missing account secret account={account_hint_value}"
            );
            self.display_name_save_in_flight = false;
            let _ = self
                .tasks
                .db_tx
                .send(crate::player_progress::DbEvent::DisplayNameSaveFailed {
                    request_id,
                    status: Some(401),
                });
            return;
        };
        let body = match serde_json::to_vec(&RenameRequest {
            account_id,
            display_name,
            auth_secret,
        }) {
            Ok(body) => body,
            Err(error) => {
                log::error!(
                    "[identity] rename request id={request_id} serialize_failed account={account_hint_value}: {error}"
                );
                self.display_name_save_in_flight = false;
                let _ =
                    self.tasks
                        .db_tx
                        .send(crate::player_progress::DbEvent::DisplayNameSaveFailed {
                            request_id,
                            status: None,
                        });
                return;
            }
        };
        let tx = self.tasks.db_tx.clone();
        let mut request = ehttp::Request::post(&url, body);
        request.headers.insert("Content-Type", "application/json");
        request
            .headers
            .insert("X-SOW-Identity-Request", request_id.to_string());
        log::info!(
            "[identity] rename request id={request_id} start account={account_hint_value} name_len={requested_name_len}"
        );
        ehttp::fetch(request, move |result| match result {
            Ok(response) if response.ok => {
                match serde_json::from_slice::<DbAccount>(&response.bytes) {
                    Ok(account) => {
                        crate::anonymous_identity::save_account_id(&account.id);
                        log::info!(
                            "[identity] rename request id={request_id} ack account={} name_len={}",
                            account_hint(Some(&account.id)),
                            account.display_name.chars().count()
                        );
                        let _ = tx.send(crate::player_progress::DbEvent::DisplayNameSaved {
                            account_id: account.id,
                            display_name: account.display_name,
                            request_id,
                        });
                    }
                    Err(error) => {
                        log::error!(
                            "[identity] rename request id={request_id} parse_failed: {error}"
                        );
                        let _ = tx.send(crate::player_progress::DbEvent::DisplayNameSaveFailed {
                            request_id,
                            status: Some(response.status),
                        });
                    }
                }
            }
            Ok(response) => {
                log::warn!(
                    "[identity] rename request id={request_id} failed status={} account={account_hint_value}",
                    response.status
                );
                let _ = tx.send(crate::player_progress::DbEvent::DisplayNameSaveFailed {
                    request_id,
                    status: Some(response.status),
                });
            }
            Err(error) => {
                log::error!("[identity] rename request id={request_id} network_failed: {error}");
                let _ = tx.send(crate::player_progress::DbEvent::DisplayNameSaveFailed {
                    request_id,
                    status: None,
                });
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
        public_id: Option<String>,
        display_name: String,
        provider: String,
    ) {
        let retry_tutorial = provider == "anonymous"
            && self.progress.intro_completed.unwrap_or(false)
            && !cloud.intro_completed.unwrap_or(false);
        let portal = self.progress.clone();
        self.progress.merge_boot_profile(cloud);
        self.progress_account_id = Some(account_id);
        self.profile_public_id = public_id;
        self.progress_provider = provider;
        if self.progress_provider == "anonymous" {
            let pending_display_name = self
                .pending_display_name
                .take()
                .or_else(|| self.queued_display_name.take());
            self.confirmed_display_name = Some(display_name.clone());
            self.ui.app.main_menu_state.player_name = pending_display_name
                .clone()
                .unwrap_or_else(|| display_name.clone());
            self.ui.app.main_menu_state.name_locked = false;
            if let Some(pending_display_name) = pending_display_name {
                self.save_display_name(pending_display_name);
            }
        }
        if !self.progress.has_history() && portal.has_history() {
            self.progress = portal;
        }
        self.apply_progress_preferences();
        if retry_tutorial {
            log::info!(
                "[tutorial] cloud profile is missing completion; retrying one-time reward sync"
            );
            self.persist_tutorial_completion();
        }
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

    fn post_store_action(
        &mut self,
        operation: &str,
        path: &str,
        mut fields: serde_json::Map<String, serde_json::Value>,
    ) {
        if self.ui.app.main_menu_state.store_busy {
            return;
        }
        let Some(public_id) = self.profile_public_id.clone() else {
            self.ui.app.main_menu_state.error_message =
                Some("Your player profile is still loading.".into());
            return;
        };
        let Some(auth_secret) = crate::anonymous_identity::load_account_secret() else {
            self.ui.app.main_menu_state.error_message =
                Some("Account setup is required for store actions.".into());
            return;
        };
        fields.insert(
            "public_id".into(),
            serde_json::Value::String(public_id.clone()),
        );
        fields.insert("auth_secret".into(), serde_json::Value::String(auth_secret));
        let Ok(body) = serde_json::to_vec(&serde_json::Value::Object(fields)) else {
            self.ui.app.main_menu_state.error_message =
                Some("Could not prepare the store request.".into());
            return;
        };
        self.ui.app.main_menu_state.store_busy = true;
        self.ui.app.main_menu_state.error_message = None;
        let url = format!(
            "{}{}",
            self.asset_config.database_base.trim_end_matches('/'),
            path
        );
        let operation = operation.to_string();
        let tx = self.tasks.db_tx.clone();
        let mut request = ehttp::Request::post(&url, body);
        request.headers.insert("Content-Type", "application/json");
        ehttp::fetch(request, move |result| match result {
            Ok(response) if response.ok => {
                #[derive(serde::Deserialize)]
                struct StoreAccount {
                    id: String,
                    profile: crate::player_progress::PlayerProgress,
                }
                match serde_json::from_slice::<StoreAccount>(&response.bytes) {
                    Ok(account) => {
                        let _ = tx.send(crate::player_progress::DbEvent::StoreProfileLoaded {
                            account_id: account.id,
                            progress: account.profile,
                            operation: operation.clone(),
                        });
                    }
                    Err(error) => {
                        log::error!("[store] {operation} response parse failed: {error}");
                        let _ = tx.send(crate::player_progress::DbEvent::StoreActionFailed {
                            operation: operation.clone(),
                            status: Some(response.status),
                            message: "Store response was invalid.".into(),
                        });
                    }
                }
            }
            Ok(response) => {
                #[derive(serde::Deserialize)]
                struct ApiError {
                    error: String,
                }
                let message = serde_json::from_slice::<ApiError>(&response.bytes)
                    .map(|error| error.error)
                    .unwrap_or_else(|_| "Store action failed.".into());
                log::error!("[store] {operation} failed status={}", response.status);
                let _ = tx.send(crate::player_progress::DbEvent::StoreActionFailed {
                    operation: operation.clone(),
                    status: Some(response.status),
                    message,
                });
            }
            Err(error) => {
                log::error!("[store] {operation} network failed: {error}");
                let _ = tx.send(crate::player_progress::DbEvent::StoreActionFailed {
                    operation: operation.clone(),
                    status: None,
                    message: "Store action could not reach the server.".into(),
                });
            }
        });
    }

    pub(crate) fn unlock_native_leader(&mut self, leader_id: String, currency: String) {
        let mut fields = serde_json::Map::new();
        fields.insert("leader_id".into(), serde_json::Value::String(leader_id));
        fields.insert("currency".into(), serde_json::Value::String(currency));
        self.post_store_action("leader unlock", "/store/leaders/unlock", fields);
    }

    pub(crate) fn unlock_native_skin(&mut self, skin_id: String) {
        let mut fields = serde_json::Map::new();
        fields.insert("skin_id".into(), serde_json::Value::String(skin_id));
        self.post_store_action("skin unlock", "/store/skins/unlock", fields);
    }

    pub(crate) fn equip_native_skin(&mut self, skin_id: String) {
        let mut fields = serde_json::Map::new();
        fields.insert("skin_id".into(), serde_json::Value::String(skin_id));
        self.post_store_action("skin equip", "/store/skins/equip", fields);
    }

    pub(crate) fn load_native_profile(&mut self) {
        let Some(public_id) = self
            .ui
            .app
            .main_menu_state
            .profile
            .public_id
            .clone()
            .or_else(|| self.profile_public_id.clone())
        else {
            self.ui.app.main_menu_state.profile.loading = false;
            self.ui.app.main_menu_state.profile.error =
                Some("Your player profile is still loading.".into());
            return;
        };
        self.ui.app.main_menu_state.profile.loading = true;
        self.ui.app.main_menu_state.profile.error = None;
        self.ui.app.main_menu_state.profile.public_id = Some(public_id.clone());
        let url = format!(
            "{}/profiles/{}",
            self.asset_config.database_base.trim_end_matches('/'),
            url::form_urlencoded::byte_serialize(public_id.as_bytes()).collect::<String>()
        );
        let tx = self.tasks.db_tx.clone();
        ehttp::fetch(ehttp::Request::get(&url), move |result| match result {
            Ok(response) if response.ok => {
                match serde_json::from_slice::<sow_data::profile::PublicProfileView>(
                    &response.bytes,
                ) {
                    Ok(view) => {
                        let _ = tx.send(crate::player_progress::DbEvent::NativeProfileLoaded {
                            public_id,
                            view,
                        });
                    }
                    Err(error) => {
                        log::error!("[profile] native profile parse failed: {error}");
                        let _ = tx.send(crate::player_progress::DbEvent::NativeProfileLoadFailed {
                            public_id,
                            status: Some(response.status),
                        });
                    }
                }
            }
            Ok(response) => {
                log::error!(
                    "[profile] native profile request failed status={}",
                    response.status
                );
                let _ = tx.send(crate::player_progress::DbEvent::NativeProfileLoadFailed {
                    public_id,
                    status: Some(response.status),
                });
            }
            Err(error) => {
                log::error!("[profile] native profile request failed: {error}");
                let _ = tx.send(crate::player_progress::DbEvent::NativeProfileLoadFailed {
                    public_id,
                    status: None,
                });
            }
        });
    }

    pub(crate) fn load_native_profile_history(&mut self) {
        if self.ui.app.main_menu_state.profile.loading {
            return;
        }
        let Some(public_id) = self
            .ui
            .app
            .main_menu_state
            .profile
            .public_id
            .clone()
            .or_else(|| self.profile_public_id.clone())
        else {
            self.ui.app.main_menu_state.profile.error =
                Some("Your player profile is still loading.".into());
            return;
        };
        let cursor = self.ui.app.main_menu_state.profile.history_cursor;
        self.ui.app.main_menu_state.profile.loading = true;
        self.ui.app.main_menu_state.profile.error = None;
        let encoded_id =
            url::form_urlencoded::byte_serialize(public_id.as_bytes()).collect::<String>();
        let url = format!(
            "{}/profiles/{}/matches?cursor={cursor}&limit=20",
            self.asset_config.database_base.trim_end_matches('/'),
            encoded_id
        );
        let tx = self.tasks.db_tx.clone();
        ehttp::fetch(ehttp::Request::get(&url), move |result| match result {
            Ok(response) if response.ok => {
                #[derive(serde::Deserialize)]
                struct MatchHistoryPage {
                    items: Vec<sow_data::profile::PublicMatchSummary>,
                    next_cursor: Option<usize>,
                }
                match serde_json::from_slice::<MatchHistoryPage>(&response.bytes) {
                    Ok(page) => {
                        let _ = tx.send(
                            crate::player_progress::DbEvent::NativeProfileHistoryLoaded {
                                public_id,
                                items: page.items,
                                next_cursor: page.next_cursor,
                            },
                        );
                    }
                    Err(error) => {
                        log::error!("[profile] history response parse failed: {error}");
                        let _ = tx.send(
                            crate::player_progress::DbEvent::NativeProfileOperationFailed {
                                public_id: Some(public_id),
                                operation: "match history".into(),
                                message: "Match history is unavailable.".into(),
                            },
                        );
                    }
                }
            }
            Ok(response) => {
                log::error!(
                    "[profile] history request failed status={}",
                    response.status
                );
                let _ = tx.send(
                    crate::player_progress::DbEvent::NativeProfileOperationFailed {
                        public_id: Some(public_id),
                        operation: "match history".into(),
                        message: format!(
                            "Match history is unavailable (HTTP {}).",
                            response.status
                        ),
                    },
                );
            }
            Err(error) => {
                log::error!("[profile] history request failed: {error}");
                let _ = tx.send(
                    crate::player_progress::DbEvent::NativeProfileOperationFailed {
                        public_id: Some(public_id),
                        operation: "match history".into(),
                        message: "Match history could not reach the server.".into(),
                    },
                );
            }
        });
    }

    pub(crate) fn load_native_profile_ratings(&mut self) {
        if self.ui.app.main_menu_state.profile.loading {
            return;
        }
        let Some(public_id) = self
            .ui
            .app
            .main_menu_state
            .profile
            .public_id
            .clone()
            .or_else(|| self.profile_public_id.clone())
        else {
            self.ui.app.main_menu_state.profile.error =
                Some("Your player profile is still loading.".into());
            return;
        };
        self.ui.app.main_menu_state.profile.loading = true;
        self.ui.app.main_menu_state.profile.error = None;
        let encoded_id =
            url::form_urlencoded::byte_serialize(public_id.as_bytes()).collect::<String>();
        let url = format!(
            "{}/profiles/{}/seasons",
            self.asset_config.database_base.trim_end_matches('/'),
            encoded_id
        );
        let tx = self.tasks.db_tx.clone();
        ehttp::fetch(ehttp::Request::get(&url), move |result| match result {
            Ok(response) if response.ok => {
                #[derive(serde::Deserialize)]
                struct RatingsResponse {
                    items: Vec<sow_data::profile::PublicRatingView>,
                }
                match serde_json::from_slice::<RatingsResponse>(&response.bytes) {
                    Ok(payload) => {
                        let _ = tx.send(
                            crate::player_progress::DbEvent::NativeProfileRatingsLoaded {
                                public_id,
                                items: payload.items,
                            },
                        );
                    }
                    Err(error) => {
                        log::error!("[profile] ratings response parse failed: {error}");
                        let _ = tx.send(
                            crate::player_progress::DbEvent::NativeProfileOperationFailed {
                                public_id: Some(public_id),
                                operation: "ranked records".into(),
                                message: "Ranked records are unavailable.".into(),
                            },
                        );
                    }
                }
            }
            Ok(response) => {
                log::error!(
                    "[profile] ratings request failed status={}",
                    response.status
                );
                let _ = tx.send(
                    crate::player_progress::DbEvent::NativeProfileOperationFailed {
                        public_id: Some(public_id),
                        operation: "ranked records".into(),
                        message: format!(
                            "Ranked records are unavailable (HTTP {}).",
                            response.status
                        ),
                    },
                );
            }
            Err(error) => {
                log::error!("[profile] ratings request failed: {error}");
                let _ = tx.send(
                    crate::player_progress::DbEvent::NativeProfileOperationFailed {
                        public_id: Some(public_id),
                        operation: "ranked records".into(),
                        message: "Ranked records could not reach the server.".into(),
                    },
                );
            }
        });
    }

    pub(crate) fn search_native_profiles(&mut self, query: String) {
        let query = query.trim().to_string();
        if query.is_empty() || self.ui.app.main_menu_state.profile.loading {
            return;
        }
        self.ui.app.main_menu_state.profile.loading = true;
        self.ui.app.main_menu_state.profile.error = None;
        let encoded_query =
            url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
        let url = format!(
            "{}/profiles/search?q={}&limit=20",
            self.asset_config.database_base.trim_end_matches('/'),
            encoded_query
        );
        let tx = self.tasks.db_tx.clone();
        ehttp::fetch(ehttp::Request::get(&url), move |result| match result {
            Ok(response) if response.ok => {
                #[derive(serde::Deserialize)]
                struct SearchResponse {
                    items: Vec<sow_data::profile::PublicProfileSummary>,
                }
                match serde_json::from_slice::<SearchResponse>(&response.bytes) {
                    Ok(payload) => {
                        let _ =
                            tx.send(crate::player_progress::DbEvent::NativeProfileSearchLoaded {
                                query,
                                items: payload.items,
                            });
                    }
                    Err(error) => {
                        log::error!("[profile] search response parse failed: {error}");
                        let _ = tx.send(
                            crate::player_progress::DbEvent::NativeProfileOperationFailed {
                                public_id: None,
                                operation: "profile search".into(),
                                message: "Player search is unavailable.".into(),
                            },
                        );
                    }
                }
            }
            Ok(response) => {
                log::error!("[profile] search request failed status={}", response.status);
                let _ = tx.send(
                    crate::player_progress::DbEvent::NativeProfileOperationFailed {
                        public_id: None,
                        operation: "profile search".into(),
                        message: format!(
                            "Player search is unavailable (HTTP {}).",
                            response.status
                        ),
                    },
                );
            }
            Err(error) => {
                log::error!("[profile] search request failed: {error}");
                let _ = tx.send(
                    crate::player_progress::DbEvent::NativeProfileOperationFailed {
                        public_id: None,
                        operation: "profile search".into(),
                        message: "Player search could not reach the server.".into(),
                    },
                );
            }
        });
    }

    pub(crate) fn load_native_match_detail(&mut self, match_id: String) {
        let match_id = match_id.trim().to_string();
        if match_id.is_empty() {
            return;
        }
        let encoded_id =
            url::form_urlencoded::byte_serialize(match_id.as_bytes()).collect::<String>();
        let url = format!(
            "{}/matches/{}",
            self.asset_config.database_base.trim_end_matches('/'),
            encoded_id
        );
        let tx = self.tasks.db_tx.clone();
        ehttp::fetch(ehttp::Request::get(&url), move |result| match result {
            Ok(response) if response.ok => {
                match serde_json::from_slice::<sow_data::profile::PublicMatchDetail>(
                    &response.bytes,
                ) {
                    Ok(detail) => {
                        let _ = tx.send(crate::player_progress::DbEvent::NativeMatchDetailLoaded {
                            match_id,
                            detail,
                        });
                    }
                    Err(error) => {
                        log::error!("[profile] match detail response parse failed: {error}");
                        let _ = tx.send(
                            crate::player_progress::DbEvent::NativeProfileOperationFailed {
                                public_id: None,
                                operation: "match detail".into(),
                                message: "Match details are unavailable.".into(),
                            },
                        );
                    }
                }
            }
            Ok(response) => {
                log::error!(
                    "[profile] match detail request failed status={}",
                    response.status
                );
                let _ = tx.send(
                    crate::player_progress::DbEvent::NativeProfileOperationFailed {
                        public_id: None,
                        operation: "match detail".into(),
                        message: format!(
                            "Match details are unavailable (HTTP {}).",
                            response.status
                        ),
                    },
                );
            }
            Err(error) => {
                log::error!("[profile] match detail request failed: {error}");
                let _ = tx.send(
                    crate::player_progress::DbEvent::NativeProfileOperationFailed {
                        public_id: None,
                        operation: "match detail".into(),
                        message: "Match details could not reach the server.".into(),
                    },
                );
            }
        });
    }

    /// Persist the tutorial completion through the anonymous account proof.
    /// The server owns the one-time reward; local storage remains the offline
    /// fallback when the account has not been minted or the request fails.
    pub(crate) fn persist_tutorial_completion(&mut self) {
        let Some(account_id) = self.progress_account_id.clone() else {
            return;
        };
        let Some(auth_secret) = crate::anonymous_identity::load_account_secret() else {
            return;
        };
        let request_id = self.next_identity_request_id();
        self.profile_request_in_flight = true;
        let url = format!(
            "{}/profile/anonymous/tutorial-complete",
            self.asset_config.database_base.trim_end_matches('/')
        );
        #[derive(serde::Serialize)]
        struct TutorialCompleteRequest {
            account_id: String,
            auth_secret: String,
        }
        let body = match serde_json::to_vec(&TutorialCompleteRequest {
            account_id,
            auth_secret,
        }) {
            Ok(body) => body,
            Err(error) => {
                log::error!("[tutorial] completion serialize failed: {error}");
                self.profile_request_in_flight = false;
                return;
            }
        };
        let tx = self.tasks.db_tx.clone();
        let mut request = ehttp::Request::post(&url, body);
        request.headers.insert("Content-Type", "application/json");
        request
            .headers
            .insert("X-SOW-Identity-Request", request_id.to_string());
        ehttp::fetch(request, move |result| match result {
            Ok(response) if response.ok => {
                #[derive(serde::Deserialize)]
                struct DbAccount {
                    id: String,
                    #[serde(default)]
                    public_id: Option<String>,
                    #[serde(default)]
                    display_name: String,
                    profile: crate::player_progress::PlayerProgress,
                }
                match serde_json::from_slice::<DbAccount>(&response.bytes) {
                    Ok(account) => {
                        let _ = tx.send(crate::player_progress::DbEvent::ProfileLoaded {
                            progress: account.profile,
                            account_id: account.id,
                            public_id: account.public_id,
                            display_name: account.display_name,
                            provider: "anonymous".to_string(),
                            request_id,
                        });
                    }
                    Err(error) => {
                        log::error!("[tutorial] completion response parse failed: {error}");
                        let _ =
                            tx.send(crate::player_progress::DbEvent::TutorialCompletionFailed {
                                request_id,
                                status: Some(response.status),
                            });
                    }
                }
            }
            Ok(response) => {
                log::warn!(
                    "[tutorial] completion request failed status={}",
                    response.status
                );
                let _ = tx.send(crate::player_progress::DbEvent::TutorialCompletionFailed {
                    request_id,
                    status: Some(response.status),
                });
            }
            Err(error) => {
                log::warn!("[tutorial] completion request failed: {error}");
                let _ = tx.send(crate::player_progress::DbEvent::TutorialCompletionFailed {
                    request_id,
                    status: None,
                });
            }
        });
    }
}
