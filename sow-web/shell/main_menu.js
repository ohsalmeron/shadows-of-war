(function () {
    "use strict";

    var root = document.getElementById("sow-menu");
    if (!root) return;

    var state = null;
    var lastRaw = "";
    var lastRenderKey = "";
    var previousScreen = null;
    var filter = "all";
    var leaderPickerOpen = false;
    var settingsOpen = false;
    var createDraft = null;
    var createOffline = false;
    var createPrivate = false;
    var createPassword = "";
    var passwordLobbyId = null;
    var passwordDraft = "";
    var pendingCommands = [];

    function esc(value) {
        return String(value == null ? "" : value)
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;")
            .replace(/"/g, "&quot;")
            .replace(/'/g, "&#39;");
    }

    function asset(path) {
        var base = String(window.SOW_ASSETS_URL || "/assets").replace(/\/$/, "");
        return base + "/" + path.split("/").map(encodeURIComponent).join("/");
    }

    function leaderById(id) {
        var leaders = state && Array.isArray(state.leaders) ? state.leaders : [];
        return leaders.find(function (leader) { return leader.id === id; }) || leaders[0] || {
            id: "Caesar", name: "Caesar", civilization: "Roman Empire", perk: "", slug: "caesar"
        };
    }

    function findLobby(id) {
        return (state && state.lobbies || []).find(function (lobby) { return lobby.id === id; }) || null;
    }

    function currentScreen() {
        if (!state) return "boot";
        if (state.waiting) return "queue";
        if (state.show_create) return "create";
        if (state.show_browser) return "browser";
        return "home";
    }

    function send(type, extra) {
        var command = Object.assign({ type: type }, extra || {});
        var serialized = JSON.stringify(command);
        if (typeof window.SOW_menu_command !== "function") {
            pendingCommands.push(serialized);
            return true;
        }
        window.SOW_menu_command(serialized);
        return true;
    }

    window.SOW_flush_menu_commands = function () {
        if (typeof window.SOW_menu_command !== "function") return;
        while (pendingCommands.length) window.SOW_menu_command(pendingCommands.shift());
    };

    function heroImage() {
        var leader = leaderById(state && state.selected_leader);
        return asset("shell/leaders/" + leader.slug + "_desktop.webp");
    }

    function avatarImage() {
        var leader = leaderById(state && state.selected_leader);
        return asset("gameplay/avatars/" + leader.slug + ".webp");
    }

    function lobbyThumb(lobby) {
        var base = String(window.SOW_MAPS_URL || "/maps").replace(/\/$/, "");
        return base + "/" + encodeURIComponent(lobby.map_name || "world") + "/thumbnail.webp";
    }

    function stableLobby(lobby) {
        return {
            id: lobby.id,
            kind: lobby.kind,
            mode: lobby.game_mode,
            map: lobby.map_name,
            players: lobby.num_players,
            max: lobby.max_players,
            // Phase flip only; the running timer moves too fast to drive re-renders.
            countdown: lobby.is_counting_down,
            password: lobby.has_password,
            host: lobby.host_name,
            names: (lobby.players || []).map(function (player) {
                return [player.player_id, player.name, player.team];
            })
        };
    }

    function renderKey() {
        var lobbies = state.lobbies || [];
        return JSON.stringify({
            phase: state.phase,
            waiting: state.waiting,
            browser: state.show_browser,
            create: state.show_create,
            connected: state.connected,
            connecting: state.connecting,
            name: state.player_name,
            locked: state.name_locked,
            leader: state.selected_leader,
            level: state.level,
            xp: state.xp,
            laurels: state.laurels,
            joined: state.joined_lobby_id,
            pending: state.pending_lobby_id,
            host: state.is_lobby_host,
            my_player_id: state.my_player_id,
            private_game: state.custom_game_is_private,
            single_player: state.custom_game_is_sp,
            downloading: state.is_downloading_map,
            download_name: state.downloading_map_name,
            download_progress: state.map_download_progress,
            error: state.error,
            notice: state.notice,
            maps: (state.map_catalog || []).map(function (map) {
                return [map.key, map.display_name, map.width, map.height];
            }),
            settings: state.settings,
            lobbies: lobbies.map(stableLobby)
        });
    }

    function renderTopbar() {
        var leader = leaderById(state.selected_leader);
        var name = state.player_name || "ANONYMOUS";
        var signIn = state.name_locked ? "ACCOUNT" : "SIGN IN";
        return "" +
            "<header class='sow-menu__topbar'>" +
                "<div class='sow-menu__brand'>" +
                    "<img class='sow-menu__brand-logo' src='/sow-long.svg' alt='Shadows of War' height='22'>" +
                    "<small>RTS</small>" +
                "</div>" +
                "<div class='sow-menu__identity'>" +
                    "<button class='sow-menu__avatar' type='button' data-command='open_leader_picker' " +
                        "aria-label='Select leader' style=\"background-image:url('" + esc(avatarImage()) + "')\"></button>" +
                    "<div class='sow-menu__profile'>" +
                        "<input data-role='display-name' name='display_name' value=\"" + esc(name) + "\" maxlength='20' " +
                            (state.name_locked ? "readonly" : "") + " aria-label='Display name'>" +
                        "<small>" + esc(leader.name) + " · " + esc(leader.civilization) + "</small>" +
                    "</div>" +
                    "<div class='sow-menu__progress' data-progression><span class='sow-menu__level'>LV " + esc(state.level) +
                        "</span> · <span class='sow-menu__xp'>" + esc(state.xp) + " XP</span> · <span class='sow-menu__laurels'>✦ " + esc(state.laurels) + "</span></div>" +
                    "<div class='sow-menu__top-actions'>" +
                        "<button class='sow-menu__signin' type='button' data-command='sign_in'>" + signIn + "</button>" +
                        "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='Settings'>⚙</button>" +
                    "</div>" +
                "</div>" +
            "</header>";
    }

    function renderCommandPanel() {
        return "" +
            "<section class='sow-menu__command'>" +
                "<p class='sow-menu__eyebrow'>REAL-TIME STRATEGY</p>" +
                "<h1>SHADOWS<br><em>OF WAR</em></h1>" +
                "<p class='sow-menu__tagline'>Match-based real-time strategy on world maps. Choose a leader, build your economy, and conquer territory.</p>" +
                "<button class='sow-menu__primary' type='button' data-command='quick_match'>QUICK MATCH <span>↗</span></button>" +
                "<button class='sow-menu__secondary' type='button' data-command='open_browser'>LOBBY BROWSER <span>→</span></button>" +
                "<form class='sow-menu__join' data-form='join'>" +
                    "<input name='code' inputmode='numeric' autocomplete='off' placeholder='LOBBY CODE' aria-label='Lobby code'>" +
                    "<button type='submit'>JOIN</button>" +
                "</form>" +
                "<button class='sow-menu__secondary' type='button' data-command='open_create'>CREATE CUSTOM GAME <span>+</span></button>" +
                "<div class='sow-menu__status' data-connection data-connected='false'>CONNECTING...</div>" +
                renderFeedback() +
            "</section>";
    }

    function renderFeedback() {
        var error = state.error ? "<div class='sow-menu__status sow-menu__status--error'>" + esc(state.error) + "</div>" : "";
        var notice = state.notice ? "<div class='sow-menu__status sow-menu__status--notice'>" +
            esc({ host_left: "Host left the lobby", kicked: "You were removed from the lobby", banned: "You are banned from this lobby", connection_lost: "Connection lost" }[state.notice] || state.notice) +
            "</div>" : "";
        return error + notice;
    }

    function publicLobbies(includeMatchmaking) {
        return (state.lobbies || []).filter(function (lobby) {
            if (lobby.kind === "Matchmaking") {
                if (!includeMatchmaking) return false;
            } else if (lobby.kind !== "Custom" || lobby.is_private) {
                return false;
            }
            if (filter === "all") return true;
            if (filter === "ffa") return lobby.game_mode === "FFA";
            if (filter === "teams") return lobby.game_mode === "Teams";
            return lobby.game_mode === "HumansVsNations";
        });
    }

    function filterButton(id, label) {
        return "<button class='sow-menu__filter' type='button' data-command='filter' data-filter='" + id +
            "' data-active='" + (filter === id) + "'>" + label + "</button>";
    }

    function lobbyTimerText(lobby) {
        return lobby.is_counting_down ? "STARTING " + Math.ceil(lobby.timer_secs || 0) + "s" :
            (lobby.max_players ? lobby.num_players + "/" + lobby.max_players : lobby.num_players + " PLAYERS");
    }

    function renderLobbyCard(lobby) {
        var lock = lobby.has_password ? "<span class='sow-menu__lobby-lock' aria-label='Password protected'>🔒</span>" : "";
        var label = (lobby.game_mode || "FFA") + " " + (lobby.map_name || "WORLD MAP");
        return "" +
            "<article class='sow-menu__lobby' role='button' tabindex='0' aria-label='" + esc(label) + "' data-command='join_lobby' data-lobby-id='" + lobby.id +
                "' style=\"background-image:url('" + esc(lobbyThumb(lobby)) + "')\">" +
                "<div class='sow-menu__lobby-top'><span>" + esc(lobby.game_mode || "FFA") + "</span><span data-timer-for='" + lobby.id + "'></span>" + lock + "</div>" +
                "<h3>" + esc(lobby.map_name || "WORLD MAP") + "</h3>" +
                "<div class='sow-menu__lobby-bottom'><span>" + esc(lobby.host_name || "OPEN LOBBY") + "</span><span>JOIN ↗</span></div>" +
            "</article>";
    }

    function renderPublicPanel() {
        var lobbies = publicLobbies(true);
        var cards = lobbies.map(renderLobbyCard).join("");
        if (!cards) {
            cards = "<div class='sow-menu__empty'>No active public lobbies found.</div>";
        }
        return "" +
            "<section class='sow-menu__public'>" +
                "<div class='sow-menu__public-head'>" +
                    "<p class='sow-menu__panel-label'>PUBLIC GAMES</p>" +
                    "<div class='sow-menu__filters'>" + filterButton("all", "ALL") + filterButton("ffa", "FFA") +
                        filterButton("teams", "TEAMS") + filterButton("hvn", "HVN") + "</div>" +
                "</div>" +
                "<div class='sow-menu__lobbies'>" + cards + "</div>" +
            "</section>";
    }

    function renderFooter(label) {
        return "<footer class='sow-menu__footer'><span>" + esc(label) + "</span><nav class='sow-menu__footer-links' aria-label='Game links'>" +
            "<a href='/how-to-play/'>GUIDES</a><a href='/terms/'>TERMS</a><a href='/privacy/'>PRIVACY</a>" +
            "<a href='https://discord.gg/eauHRf7zP' rel='noreferrer'>DISCORD</a><a href='https://github.com/worldofunreal/shadows-of-war' rel='noreferrer'>GITHUB</a>" +
            "</nav><span>SHADOWSOFWAR.IO</span></footer>";
    }

    function renderHome() {
        var leader = leaderById(state.selected_leader);
        var leaderOverlay = leaderPickerOpen ? renderLeaderPicker() : "";
        var settingsOverlay = settingsOpen ? renderSettings() : "";
        return "" +
            "<div class='sow-menu__backdrop'></div>" +
            "<div class='sow-menu__shell'>" +
                renderTopbar() +
                "<main class='sow-menu__main'>" +
                    renderCommandPanel() +
                    "<section class='sow-menu__battlefield'>" +
                        "<div class='sow-menu__leader-copy'><small>" + esc(leader.civilization) + "</small><h2>" + esc(leader.name) +
                            "</h2><p>" + esc(leader.perk) + "</p></div>" +
                        renderPublicPanel() +
                    "</section>" +
                "</main>" +
                renderFooter("CROSS-PLATFORM RTS") +
            "</div>" + leaderOverlay + settingsOverlay + renderPasswordModal();
    }

    function renderBrowser() {
        var lobbies = publicLobbies(false);
        var cards = lobbies.map(renderLobbyCard).join("");
        if (!cards) cards = "<div class='sow-menu__empty'>No public games match this filter.</div>";
        return "" +
            "<div class='sow-menu__backdrop'></div><div class='sow-menu__shell'>" +
                renderTopbar() +
                "<main class='sow-menu__main'>" +
                    "<section class='sow-menu__command'><p class='sow-menu__eyebrow'>LOBBY BROWSER</p><h1>ACTIVE<br><em>MATCHES</em></h1>" +
                        "<p class='sow-menu__tagline'>Browse and join public matches, or enter a private lobby code.</p>" +
                        "<button class='sow-menu__secondary' type='button' data-command='close_overlay'>← BACK</button>" +
                        "<form class='sow-menu__join' data-form='join'><input name='code' inputmode='numeric' autocomplete='off' placeholder='LOBBY CODE' aria-label='Lobby code'><button type='submit'>JOIN</button></form>" + renderFeedback() +
                    "</section>" +
                    "<section class='sow-menu__battlefield'><section class='sow-menu__public'><div class='sow-menu__public-head'><p class='sow-menu__panel-label'>PUBLIC GAMES</p><div class='sow-menu__filters'>" +
                        filterButton("all", "ALL") + filterButton("ffa", "FFA") + filterButton("teams", "TEAMS") + filterButton("hvn", "HVN") +
                    "</div></div><div class='sow-menu__lobbies'>" + cards + "</div></section></section>" +
                "</main>" + renderFooter("ACTIVE LOBBIES") +
            "</div>" + renderPasswordModal();
    }

    function cloneConfig() {
        return JSON.parse(JSON.stringify(state.custom_game_config || {}));
    }

    function syncCreateDraft(form) {
        if (!createDraft) createDraft = cloneConfig();
        Array.prototype.forEach.call(form.elements, function (field) {
            if (!field.name) return;
            if (field.name === "session_mode") {
                createOffline = field.value === "offline";
            } else if (field.name === "visibility") {
                createPrivate = field.value === "private";
            } else if (field.name === "password") {
                createPassword = field.value;
            } else if (field.name === "bot_count" || field.name === "nation_count") {
                createDraft[field.name] = Number(field.value);
            } else if (field.name === "map_name") {
                createDraft.map_name = field.value;
                var map = (state.map_catalog || []).find(function (entry) { return entry.key === field.value; });
                if (map) {
                    createDraft.map_width = map.width;
                    createDraft.map_height = map.height;
                }
            } else if (field.name !== "visibility" && field.name !== "max_players") {
                createDraft[field.name] = field.value;
            }
        });
    }

    function renderPasswordModal() {
        if (passwordLobbyId == null) return "";
        var lobby = findLobby(passwordLobbyId);
        var title = lobby ? (lobby.map_name || "PRIVATE LOBBY") : "PRIVATE LOBBY";
        var error = state.error ? "<div class='sow-menu__status sow-menu__status--error'>" + esc(state.error) + "</div>" : "";
        return "<div class='sow-menu__overlay'><form class='sow-menu__modal sow-menu__password-modal' data-form='password' novalidate>" +
            "<div class='sow-menu__modal-head'><div><p class='sow-menu__panel-label'>PASSWORD REQUIRED</p><h2>" + esc(title) + "</h2></div>" +
            "<button class='sow-menu__icon-button' type='button' data-command='close_password' aria-label='Close'>×</button></div>" +
            "<p class='sow-menu__tagline'>This lobby is private. Enter password to join.</p>" +
            "<label class='sow-menu__form-field'>PASSWORD<input class='sow-menu__field' name='password' type='password' autocomplete='current-password' value='" + esc(passwordDraft) + "' autofocus></label>" +
            error +
            "<div class='sow-menu__modal-actions'><button class='sow-menu__ghost-button' type='button' data-command='close_password'>CANCEL</button><button class='sow-menu__primary' type='submit'>JOIN LOBBY <span>↗</span></button></div></form></div>";
    }

    function modeOptions(config) {
        return ["FFA", "Teams", "HumansVsNations"].map(function (mode) {
            return "<option value='" + mode + "' " + (config.game_mode === mode ? "selected" : "") + ">" + mode + "</option>";
        }).join("");
    }

    function mapOptions(config) {
        var maps = state.map_catalog || [];
        if (!maps.length) {
            return "<option value='" + esc(config.map_name || "world") + "'>" + esc(config.map_name || "WORLD MAP") + "</option>";
        }
        return maps.map(function (map) {
            return "<option value='" + esc(map.key) + "' " + (config.map_name === map.key ? "selected" : "") + ">" + esc(map.display_name) + "</option>";
        }).join("");
    }

    function renderCreate() {
        var config = createDraft || cloneConfig();
        createDraft = config;
        var privateGame = createPrivate;
        var onlineFields = createOffline ? "" :
            "<label class='sow-menu__form-field sow-menu__form-field--wide'>VISIBILITY<select class='sow-menu__select' name='visibility'><option value='public' " + (!privateGame ? "selected" : "") + ">PUBLIC</option><option value='private' " + (privateGame ? "selected" : "") + ">PRIVATE (CODE ONLY)</option></select></label>" +
            "<label class='sow-menu__form-field sow-menu__form-field--wide'>PASSWORD (OPTIONAL)<input class='sow-menu__field' name='password' type='password' autocomplete='new-password' value='" + esc(createPassword) + "' placeholder='Leave empty for public access'></label>";
        return "" +
            "<div class='sow-menu__backdrop'></div><div class='sow-menu__shell'>" + renderTopbar() +
                "<main class='sow-menu__main'><section class='sow-menu__command'><p class='sow-menu__eyebrow'>CUSTOM GAME</p><h1>MATCH<br><em>SETTINGS</em></h1><p class='sow-menu__tagline'>Configure match rules, map, bot counts, and lobby visibility.</p><button class='sow-menu__secondary' type='button' data-command='close_overlay'>← CANCEL</button></section>" +
                    "<section class='sow-menu__battlefield'><form class='sow-menu__modal' data-form='create'><div class='sow-menu__modal-head'><div><p class='sow-menu__panel-label'>CUSTOM MATCH</p><h2>MATCH RULES</h2></div></div>" +
                        "<div class='sow-menu__form-grid'>" +
                            "<label class='sow-menu__form-field sow-menu__form-field--wide'>MODE<select class='sow-menu__select' name='session_mode'><option value='online' " + (!createOffline ? "selected" : "") + ">ONLINE LOBBY</option><option value='offline' " + (createOffline ? "selected" : "") + ">SOLO / PRACTICE</option></select></label>" +
                            "<label class='sow-menu__form-field sow-menu__form-field--wide'>MAP<select class='sow-menu__select' name='map_name'>" + mapOptions(config) + "</select></label>" +
                            "<label class='sow-menu__form-field'>GAME TYPE<select class='sow-menu__select' name='game_mode'>" + modeOptions(config) + "</select></label>" +
                            "<label class='sow-menu__form-field'>TRIBES<input class='sow-menu__field' name='bot_count' type='number' min='0' max='1000' value='" + esc(config.bot_count || 128) + "'></label>" +
                            "<label class='sow-menu__form-field'>NATIONS<input class='sow-menu__field' name='nation_count' type='number' min='0' max='400' value='" + esc(config.nation_count || 32) + "'></label>" +
                            "<label class='sow-menu__form-field'>DIFFICULTY<select class='sow-menu__select' name='bot_difficulty'><option value='Vanilla' " + (config.bot_difficulty === "Vanilla" ? "selected" : "") + ">VANILLA</option><option value='Terminator' " + (config.bot_difficulty === "Terminator" ? "selected" : "") + ">TERMINATOR</option></select></label>" +
                        onlineFields + renderFeedback() +
                        "</div><div class='sow-menu__modal-actions'><button class='sow-menu__ghost-button' type='button' data-command='close_overlay'>CANCEL</button><button class='sow-menu__primary' type='submit'>" + (createOffline ? "START SOLO MATCH" : "CREATE LOBBY") + " <span>↗</span></button></div></form></section>" +
                "</main>" + renderFooter("CUSTOM MATCH") + "</div>";
    }

    function joinedLobby() {
        var id = state.joined_lobby_id || state.pending_lobby_id;
        return (state.lobbies || []).find(function (lobby) { return lobby.id === id; }) || null;
    }

    function renderQueuePlayer(player, lobby) {
        var canModerate = lobby && lobby.kind === "Custom" && state.is_lobby_host && player.player_id !== state.my_player_id;
        var isHost = lobby && lobby.host_name === player.name;
        var hostBadge = isHost ? " <small class='sow-menu__host-badge'>HOST</small>" : "";
        var controls = canModerate ?
            "<div class='sow-menu__player-actions'><button type='button' data-command='kick_player' data-lobby-id='" + lobby.id + "' data-player-id='" + player.player_id + "'>KICK</button>" +
            (lobby.game_mode === "Teams" ? "<button type='button' data-command='move_player_team' data-lobby-id='" + lobby.id + "' data-player-id='" + player.player_id + "'>MOVE TEAM</button>" : "") +
            "<button class='sow-menu__player-action--danger' type='button' data-command='ban_player' data-lobby-id='" + lobby.id + "' data-player-id='" + player.player_id + "'>BAN</button></div>" : "";
        return "<div class='sow-menu__player'><span><strong>" + esc(player.name) + "</strong>" + (player.player_id === state.my_player_id ? " <small>YOU</small>" : "") + hostBadge + "</span><span><small>" + esc(player.team || "PLAYER") + "</small>" + controls + "</span></div>";
    }

    function renderQueue() {
        var lobby = joinedLobby();
        var title = lobby ? (lobby.map_name || "WORLD MAP") : "MATCHMAKING";
        var feedback = state.is_downloading_map ?
            "<div class='sow-menu__queue-feedback'>DOWNLOADING " + esc(state.downloading_map_name || title) + " · " + esc(state.map_download_progress || 0) + "%</div>" :
            (state.error ? "<div class='sow-menu__queue-feedback sow-menu__queue-feedback--error'>" + esc(state.error) + "</div>" : "");
        var roster = lobby && lobby.players ? lobby.players.map(function (player) {
            return renderQueuePlayer(player, lobby);
        }).join("") : "<div class='sow-menu__empty'>Connecting to match...</div>";
        var hostAction = lobby && state.is_lobby_host && lobby.kind === "Custom" ?
            "<button class='sow-menu__primary' type='button' data-command='start_private' data-lobby-id='" + lobby.id + "'>START GAME <span>↗</span></button>" : "";
        var code = lobby && lobby.kind === "Custom" ?
            "<div class='sow-menu__lobby-code'><span>LOBBY CODE</span><strong>" + lobby.id + "</strong><button type='button' data-command='copy_lobby_code' data-lobby-id='" + lobby.id + "'>COPY</button></div>" : "";
        var passwordBadge = lobby && lobby.has_password ? "<span class='sow-menu__queue-badge'>🔒 PASSWORD</span>" : "";
        return "" +
            "<div class='sow-menu__backdrop'></div><div class='sow-menu__shell'>" + renderTopbar() +
                "<main class='sow-menu__main'><section class='sow-menu__command'><p class='sow-menu__eyebrow'>MATCHMAKING</p><h1>SEARCHING<br><em>FOR MATCH</em></h1><p class='sow-menu__tagline'>Waiting for players to join the lobby.</p><div class='sow-menu__status' data-connection data-connected='" + state.connected + "'>" + (state.connected ? "ONLINE" : "CONNECTING...") + "</div><button class='sow-menu__danger' type='button' data-command='leave_lobby'>LEAVE LOBBY <span>×</span></button></section>" +
                    "<section class='sow-menu__battlefield'><section class='sow-menu__queue'><div class='sow-menu__queue-head'><div><p class='sow-menu__panel-label'>" + esc(lobby ? (lobby.game_mode || "MATCHMAKING") : "MATCHMAKING") + "</p><h2>" + esc(title) + "</h2></div><span data-live-countdown></span></div><div class='sow-menu__queue-meta'>" + passwordBadge + code + "</div><div class='sow-menu__queue-status' data-queue-status>PLAYERS IN LOBBY<strong>" + (lobby ? esc((lobby.num_players || 0) + "/" + (lobby.max_players || "?")) : "—") + "</strong></div>" + feedback + "<div class='sow-menu__roster'>" + roster + "</div><div class='sow-menu__modal-actions'>" + hostAction + "</div></section></section>" +
                "</main>" + renderFooter("LIVE LOBBY") + "</div>";
    }

    function renderLeaderPicker() {
        var leaders = state.leaders || [];
        return "<div class='sow-menu__overlay'><section class='sow-menu__modal'><div class='sow-menu__modal-head'><div><p class='sow-menu__panel-label'>LEADERS</p><h2>SELECT LEADER</h2></div><button class='sow-menu__icon-button' type='button' data-command='close_leader_picker'>×</button></div><div class='sow-menu__leader-list'>" +
            leaders.map(function (leader) {
                return "<button class='sow-menu__leader-option' type='button' data-command='set_leader' data-leader-id='" + esc(leader.id) + "' data-selected='" + (leader.id === state.selected_leader) + "'><img src='" + esc(asset("gameplay/avatars/" + leader.slug + ".webp")) + "' alt=''><span>" + esc(leader.name) + "</span></button>";
            }).join("") + "</div></section></div>";
    }

    function renderSettings() {
        var settings = state.settings || {};
        var fullscreen = document.fullscreenElement ? "EXIT FULLSCREEN" : "FULLSCREEN";
        return "<div class='sow-menu__overlay'><section class='sow-menu__modal'><div class='sow-menu__modal-head'><div><p class='sow-menu__panel-label'>SYSTEM</p><h2>SETTINGS</h2></div><button class='sow-menu__icon-button' type='button' data-command='toggle_settings'>×</button></div><div class='sow-menu__form-grid'><label class='sow-menu__form-field sow-menu__form-field--wide'>MASTER AUDIO<select class='sow-menu__select' name='mute_all' data-setting='mute'><option value='on' " + (!settings.mute_all ? "selected" : "") + ">ON</option><option value='off' " + (settings.mute_all ? "selected" : "") + ">OFF</option></select></label><label class='sow-menu__form-field sow-menu__form-field--wide'>MUSIC VOLUME<input class='sow-menu__field' type='range' name='music_volume' min='0' max='1' step='0.05' value='" + esc(settings.music_volume == null ? 0.8 : settings.music_volume) + "' data-setting='music_volume'></label><label class='sow-menu__form-field sow-menu__form-field--wide'>MOTION<select class='sow-menu__select' name='reduced_motion' data-setting='reduced_motion'><option value='full' " + (!settings.reduced_motion ? "selected" : "") + ">FULL</option><option value='reduced' " + (settings.reduced_motion ? "selected" : "") + ">REDUCED</option></select></label></div><button class='sow-menu__secondary' type='button' data-command='toggle_fullscreen'>" + fullscreen + "</button><div class='sow-menu__modal-actions'><button class='sow-menu__primary' type='button' data-command='toggle_settings'>DONE <span>✓</span></button></div></section></div>";
    }

    function render() {
        if (!state) return;
        var screen = currentScreen();
        if (screen === "create" && previousScreen !== "create") {
            createDraft = cloneConfig();
            createOffline = !!state.custom_game_is_sp;
            createPrivate = !!state.custom_game_is_private;
            createPassword = "";
        }
        if (screen !== "create") {
            createDraft = null;
            createOffline = false;
            createPrivate = false;
            createPassword = "";
        }
        previousScreen = screen;
        root.style.setProperty("--sow-hero", "url(\"" + heroImage() + "\")");
        root.dataset.ready = typeof window.SOW_menu_command === "function" ? "true" : "false";
        root.hidden = state.phase !== "MainMenu";
        if (screen === "home") root.innerHTML = renderHome();
        else if (screen === "browser") root.innerHTML = renderBrowser();
        else if (screen === "create") root.innerHTML = renderCreate();
        else if (screen === "queue") root.innerHTML = renderQueue();
        else root.innerHTML = "";
        updateDynamic();
    }

    function updateDynamic() {
        if (!state || root.hidden) return;
        var connection = root.querySelector("[data-connection]");
        if (connection) {
            connection.dataset.connected = String(!!state.connected);
            connection.textContent = state.connected ? "ONLINE" : (state.connecting ? "CONNECTING..." : "OFFLINE · RETRYING");
        }
        var progression = root.querySelector("[data-progression]");
        if (progression) progression.textContent = "LV " + state.level + " · " + state.xp + " XP · ✦ " + state.laurels;
        var timer = root.querySelector("[data-live-countdown]");
        var lobby = joinedLobby();
        if (timer && lobby) timer.textContent = lobby.is_counting_down ? "STARTING IN " + Math.ceil(lobby.timer_secs) + "s" : "WAITING FOR PLAYERS";
        var queueStatus = root.querySelector("[data-queue-status]");
        if (queueStatus && lobby) {
            var strong = queueStatus.querySelector("strong");
            if (strong) strong.textContent = (lobby.num_players || 0) + "/" + (lobby.max_players || "?");
        }
        var cardTimers = root.querySelectorAll("[data-timer-for]");
        for (var i = 0; i < cardTimers.length; i++) {
            var cardLobby = findLobby(Number(cardTimers[i].dataset.timerFor));
            cardTimers[i].textContent = cardLobby ? lobbyTimerText(cardLobby) : "";
        }
    }

    root.addEventListener("click", function (event) {
        var target = event.target.closest("[data-command]");
        if (!target || !root.contains(target)) return;
        var command = target.dataset.command;
        if (command === "open_leader_picker") {
            leaderPickerOpen = true;
            settingsOpen = false;
            render();
            return;
        }
        if (command === "close_leader_picker") {
            leaderPickerOpen = false;
            render();
            return;
        }
        if (command === "toggle_settings") {
            settingsOpen = !settingsOpen;
            leaderPickerOpen = false;
            render();
            return;
        }
        if (command === "filter") {
            filter = target.dataset.filter || "all";
            render();
            return;
        }
        if (command === "close_password") {
            passwordLobbyId = null;
            passwordDraft = "";
            render();
            return;
        }
        if (command === "copy_lobby_code") {
            var lobbyCode = String(target.dataset.lobbyId || "");
            if (navigator.clipboard && lobbyCode) {
                navigator.clipboard.writeText(lobbyCode).then(function () {
                    target.textContent = "COPIED";
                }).catch(function () {
                    target.textContent = "COPY FAILED";
                });
            } else if (lobbyCode) {
                target.textContent = "COPY UNAVAILABLE";
            }
            return;
        }
        if (command === "toggle_fullscreen") {
            if (document.fullscreenElement) {
                document.exitFullscreen().catch(function (error) {
                    console.warn("[WEB MENU] unable to exit fullscreen:", error);
                });
            } else if (document.documentElement.requestFullscreen) {
                document.documentElement.requestFullscreen().catch(function (error) {
                    console.warn("[WEB MENU] unable to enter fullscreen:", error);
                });
            }
            return;
        }
        if (command === "ban_player" && !window.confirm("Ban this player from the lobby?")) {
            return;
        }
        if (command === "join_lobby") {
            var selectedLobby = findLobby(Number(target.dataset.lobbyId));
            if (selectedLobby && selectedLobby.has_password) {
                passwordLobbyId = selectedLobby.id;
                passwordDraft = "";
                render();
            } else {
                send("join_lobby", { lobby_id: Number(target.dataset.lobbyId) });
            }
            return;
        }
        if (command === "set_leader") {
            if (send("set_leader", { leader_id: target.dataset.leaderId })) {
                leaderPickerOpen = false;
            }
            return;
        }
        var payload = {};
        if (target.dataset.lobbyId) payload.lobby_id = Number(target.dataset.lobbyId);
        if (target.dataset.playerId) payload.target_player_id = Number(target.dataset.playerId);
        send(command, payload);
    });

    root.addEventListener("keydown", function (event) {
        if (event.key !== "Enter" && event.key !== " ") return;
        var target = event.target.closest("[data-command='join_lobby']");
        if (!target || !root.contains(target)) return;
        event.preventDefault();
        target.click();
    });

    root.addEventListener("submit", function (event) {
        var form = event.target;
        if (form.dataset.form === "join") {
            event.preventDefault();
            var code = form.elements.code.value.trim();
            send("join_code", { code: code });
        }
        if (form.dataset.form === "password") {
            event.preventDefault();
            passwordDraft = form.elements.password.value;
            if (passwordLobbyId != null && passwordDraft) {
                send("join_with_password", { lobby_id: passwordLobbyId, password: passwordDraft });
            }
        }
        if (form.dataset.form === "create") {
            event.preventDefault();
            syncCreateDraft(form);
            var config = createDraft || cloneConfig();
            if (createOffline) {
                send("start_single_player", { config: config });
            } else {
                send("create_game", { config: config, is_private: createPrivate, password: createPassword || null });
            }
        }
    });

    root.addEventListener("input", function (event) {
        var createForm = event.target.closest("form[data-form='create']");
        if (createForm) syncCreateDraft(createForm);
        if (event.target.name === "password" && event.target.closest("form[data-form='password']")) {
            passwordDraft = event.target.value;
        }
    });

    root.addEventListener("focusout", function (event) {
        var input = event.target;
        if (input.dataset.role !== "display-name" || state.name_locked) return;
        var name = input.value.trim();
        if (name && name !== state.player_name) send("save_display_name", { name: name });
    });

    root.addEventListener("change", function (event) {
        var input = event.target;
        var createForm = input.closest("form[data-form='create']");
        if (createForm) {
            syncCreateDraft(createForm);
            render();
            return;
        }
        if (!input.dataset.setting) return;
        if (input.dataset.setting === "mute") send("set_mute", { value: input.value === "off" });
        if (input.dataset.setting === "music_volume") send("set_music_volume", { value: Number(input.value) });
        if (input.dataset.setting === "reduced_motion") send("set_reduced_motion", { value: input.value === "reduced" });
    });

    document.addEventListener("fullscreenchange", function () {
        if (state && settingsOpen) render();
    });

    function poll() {
        var raw = window.SOW_MENU_STATE;
        if (typeof raw !== "string" || raw === lastRaw) {
            updateDynamic();
            return;
        }
        lastRaw = raw;
        try {
            state = JSON.parse(raw);
        } catch (error) {
            console.warn("[WEB MENU] invalid state:", error);
            return;
        }
        if (typeof window.SOW_syncWebLoader === "function") {
            window.SOW_syncWebLoader(state);
        }
        if (state.waiting && passwordLobbyId != null) {
            passwordLobbyId = null;
            passwordDraft = "";
        }
        var key = renderKey();
        if (key !== lastRenderKey) {
            lastRenderKey = key;
            render();
        } else {
            root.hidden = state.phase !== "MainMenu";
            updateDynamic();
        }
    }

    root.hidden = true;
    window.setInterval(poll, 80);
})();
