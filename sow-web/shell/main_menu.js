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
    var tempSelectedLeader = null;
    var browserSearchQuery = "";
    var settingsOpen = false;
    var createDraft = null;
    var createOffline = false;
    var createPrivate = false;
    var createPassword = "";
    var passwordLobbyId = null;
    var passwordDraft = "";
    var pendingCommands = [];
    var pendingHud = null;
    var lastHudRaw = "";
    var pollTimer = null;

    var LEADER_REIGNS = {
        "Caesar": "Reigned 49 – 44 BC",
        "Cleopatra": "Reigned 51 – 30 BC",
        "Ragnar": "Reigned 800 – 845 AD",
        "SunTzu": "Reigned 544 – 496 BC",
        "Alexander": "Reigned 336 – 323 BC",
        "GenghisKhan": "Reigned 1206 – 1227 AD",
        "RichardTheLionheart": "Reigned 1189 – 1199 AD",
        "Vercingetorix": "Reigned 82 – 46 BC",
        "Boudica": "Reigned 60 – 61 AD",
        "LadySixSky": "Reigned 612 – 693 AD",
        "Leonidas": "Reigned 489 – 480 BC",
        "Napoleon": "Reigned 1804 – 1814 AD"
    };

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
        var found = leaders.find(function (leader) { return leader.id === id; });
        if (found) return found;
        return leaders[0] || {
            id: "Caesar", name: "Caesar", civilization: "Roman Empire", perk: "Imperium: +15% Territory Expansion Speed", slug: "caesar"
        };
    }

    function mapInfo(key) {
        var catalog = state && Array.isArray(state.map_catalog) ? state.map_catalog : [];
        var found = catalog.find(function (m) { return m.key === key; });
        if (found) return found;
        return { key: key || "world", display_name: (key || "WORLD MAP").toUpperCase(), width: 2048, height: 1024 };
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
        return base + "/" + encodeURIComponent((lobby && lobby.map_name) || "world") + "/thumbnail.webp";
    }

    function stableLobby(lobby) {
        return {
            id: lobby.id,
            kind: lobby.kind,
            mode: lobby.game_mode,
            map: lobby.map_name,
            players: lobby.num_players,
            max: lobby.max_players,
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

    var displayNameDraft = null;

    function renderTopbar() {
        var leader = leaderById(state.selected_leader);
        var name = displayNameDraft != null ? displayNameDraft : (state.player_name || "ANONYMOUS");
        var signIn = state.name_locked ? "ACCOUNT" : "SIGN IN";
        return "" +
            "<header class='sow-menu__topbar'>" +
                "<div class='sow-menu__brand'>" +
                    "<img class='sow-menu__brand-logo' src='/sow-long.svg' alt='Shadows of War'>" +
                "</div>" +
                "<div class='sow-menu__identity'>" +
                    "<button class='sow-menu__avatar' type='button' data-command='open_leader_picker' " +
                        "aria-label='Select leader' style=\"background-image:url('" + esc(avatarImage()) + "')\"></button>" +
                    "<div class='sow-menu__profile'>" +
                        "<input data-role='display-name' name='display_name' value=\"" + esc(name) + "\" maxlength='20' " +
                            (state.name_locked ? "readonly" : "") + " aria-label='Display name'>" +
                        "<small>" + esc(leader.name) + " · " + esc(leader.civilization) + "</small>" +
                    "</div>" +
                "</div>" +
                "<div class='sow-menu__top-actions'>" +
                    "<div class='sow-menu__progress' data-progression title='Account Progression'>" +
                        "<span class='sow-menu__level'><small>LV</small> " + esc(state.level) + "</span>" +
                        "<span class='sow-menu__progress-sep'>·</span>" +
                        "<span class='sow-menu__xp'>" + esc(state.xp) + " <small>XP</small></span>" +
                        "<span class='sow-menu__progress-sep'>·</span>" +
                        "<span class='sow-menu__laurels'>✦ " + esc(state.laurels) + "</span>" +
                    "</div>" +
                    "<button class='sow-menu__signin' type='button' data-command='sign_in'>" + signIn + "</button>" +
                    "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='Settings'>⚙</button>" +
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
        if (browserSearchQuery) {
            var q = browserSearchQuery.toLowerCase().trim();
            lobbies = lobbies.filter(function (l) {
                return (l.map_name && l.map_name.toLowerCase().indexOf(q) !== -1) ||
                       (l.host_name && l.host_name.toLowerCase().indexOf(q) !== -1) ||
                       (l.game_mode && l.game_mode.toLowerCase().indexOf(q) !== -1);
            });
        }
        var cards = lobbies.map(renderLobbyCard).join("");
        if (!cards) {
            cards = "<div class='sow-menu__empty'>No public games match your search.</div>";
        }
        return "" +
            "<div class='sow-menu__backdrop'></div>" +
            "<div class='sow-menu__shell'>" +
                renderTopbar() +
                "<main class='sow-menu__main'>" +
                    "<section class='sow-menu__command'>" +
                        "<p class='sow-menu__eyebrow'>LOBBY BROWSER</p>" +
                        "<h1>ACTIVE<br><em>MATCHES</em></h1>" +
                        "<p class='sow-menu__tagline'>Browse and join active multiplayer matches across all map sectors, or enter a private code.</p>" +
                        "<button class='sow-menu__secondary' type='button' data-command='close_overlay'>← BACK</button>" +
                        "<form class='sow-menu__join' data-form='join'>" +
                            "<input name='code' inputmode='numeric' autocomplete='off' placeholder='LOBBY CODE' aria-label='Lobby code'>" +
                            "<button type='submit'>JOIN</button>" +
                        "</form>" +
                        "<button class='sow-menu__secondary' type='button' data-command='open_create'>CREATE CUSTOM GAME <span>+</span></button>" +
                        renderFeedback() +
                    "</section>" +
                    "<section class='sow-menu__battlefield'>" +
                        "<section class='sow-menu__public'>" +
                            "<div class='sow-menu__public-head'>" +
                                "<p class='sow-menu__panel-label'>PUBLIC GAMES (" + lobbies.length + ")</p>" +
                                "<div class='sow-menu__filters'>" +
                                    filterButton("all", "ALL") +
                                    filterButton("ffa", "FFA") +
                                    filterButton("teams", "TEAMS") +
                                    filterButton("hvn", "HVN") +
                                "</div>" +
                            "</div>" +
                            "<div class='sow-menu__browser-search'>" +
                                "<input data-role='browser-search' type='search' placeholder='Search by map or host name...' value=\"" + esc(browserSearchQuery) + "\">" +
                            "</div>" +
                            "<div class='sow-menu__lobbies'>" + cards + "</div>" +
                        "</section>" +
                    "</section>" +
                "</main>" +
                renderFooter("GLOBAL LOBBY BROWSER") +
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
            } else if (field.name === "bot_count" || field.name === "nation_count" || field.name === "max_players" || field.name === "seed") {
                createDraft[field.name] = Number(field.value);
            } else if (field.name === "map_name") {
                createDraft.map_name = field.value;
                var map = mapInfo(field.value);
                if (map) {
                    createDraft.map_width = map.width;
                    createDraft.map_height = map.height;
                }
            } else if (field.name !== "visibility") {
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

    function renderCreate() {
        var config = createDraft || cloneConfig();
        createDraft = config;
        var isSp = createOffline;
        var selectedMap = mapInfo(config.map_name || "world");
        var mapThumbUrl = lobbyThumb({ map_name: selectedMap.key });

        var modeOptionsHtml = ["FFA", "Teams", "HumansVsNations"].map(function (mode) {
            var labels = { FFA: "FREE FOR ALL", Teams: "TEAMS (2)", HumansVsNations: "HVN" };
            var isSelected = (config.game_mode || "FFA") === mode;
            return "<button type='button' class='sow-menu__pill" + (isSelected ? " active" : "") + "' data-command='set_create_mode' data-mode='" + mode + "'>" + (labels[mode] || mode) + "</button>";
        }).join("");

        var diffOptionsHtml = ["Vanilla", "Terminator"].map(function (diff) {
            var isSelected = (config.bot_difficulty || "Vanilla") === diff;
            return "<button type='button' class='sow-menu__pill" + (isSelected ? " active" : "") + "' data-command='set_create_diff' data-diff='" + diff + "'>" + diff.toUpperCase() + "</button>";
        }).join("");

        var spawnOptionsHtml = [true, false].map(function (val) {
            var isSelected = (config.random_spawn !== false) === val;
            return "<button type='button' class='sow-menu__pill" + (isSelected ? " active" : "") + "' data-command='set_create_spawn' data-spawn='" + String(val) + "'>" + (val ? "RANDOM ON" : "PRESET OFF") + "</button>";
        }).join("");

        var visOptionsHtml = [false, true].map(function (val) {
            var isSelected = createPrivate === val;
            return "<button type='button' class='sow-menu__pill" + (isSelected ? " active" : "") + "' data-command='set_create_private' data-private='" + String(val) + "'>" + (val ? "PRIVATE (CODE)" : "PUBLIC") + "</button>";
        }).join("");

        var mapCatalogOptions = (state && state.map_catalog || []).map(function (m) {
            return "<option value='" + esc(m.key) + "' " + (m.key === selectedMap.key ? "selected" : "") + ">" + esc(m.display_name) + " (" + m.width + "×" + m.height + ")</option>";
        }).join("");

        var spControls = isSp ?
            "<div class='sow-menu__slider-field'>" +
                "<div class='sow-menu__slider-label'><span>PROCEDURAL SEED</span><b data-val-for='seed'>" + esc(config.seed || 42) + "</b></div>" +
                "<div class='sow-menu__slider-row'>" +
                    "<input class='sow-menu__range' name='seed' type='range' min='1' max='9999' step='1' value='" + esc(config.seed || 42) + "'>" +
                    "<button class='sow-menu__ghost-mini' type='button' data-command='randomize_seed'>🎲</button>" +
                "</div>" +
            "</div>" :
            "<div>" +
                "<label class='sow-menu__form-field'>VISIBILITY" +
                    "<div class='sow-menu__pill-group'>" + visOptionsHtml + "</div>" +
                "</label>" +
                (createPrivate ?
                    "<label class='sow-menu__form-field' style='margin-top:10px;'>PASSWORD (OPTIONAL)" +
                        "<input class='sow-menu__field' name='password' type='password' autocomplete='new-password' value='" + esc(createPassword) + "' placeholder='Leave empty for no password'>" +
                    "</label>" : "") +
            "</div>";

        return "" +
            "<div class='sow-menu__backdrop'></div>" +
            "<div class='sow-menu__shell'>" +
                renderTopbar() +
                "<main class='sow-menu__main sow-menu__main--custom'>" +
                    "<section class='sow-menu__command'>" +
                        "<p class='sow-menu__eyebrow'>CUSTOM GAME</p>" +
                        "<h1>MATCH<br><em>SETTINGS</em></h1>" +
                        "<p class='sow-menu__tagline'>" + (isSp ? "Configure offline simulation rules, AI tribes, and map dimensions." : "Host a public or private room on official game servers.") + "</p>" +
                        "<div class='sow-menu__mode-switcher'>" +
                            "<button type='button' class='sow-menu__mode-tab" + (isSp ? " active" : "") + "' data-command='set_session_mode' data-mode='offline'>🎮 SOLO / PRACTICE</button>" +
                            "<button type='button' class='sow-menu__mode-tab" + (!isSp ? " active" : "") + "' data-command='set_session_mode' data-mode='online'>🌐 ONLINE LOBBY</button>" +
                        "</div>" +
                        "<button class='sow-menu__secondary' type='button' data-command='close_overlay'>← CANCEL</button>" +
                    "</section>" +
                    "<section class='sow-menu__battlefield'>" +
                        "<form class='sow-menu__custom-grid' data-form='create'>" +
                            "<div class='sow-menu__custom-col'>" +
                                "<div class='sow-menu__custom-card'>" +
                                    "<div class='sow-menu__map-preview-wrap' style=\"background-image:url('" + esc(mapThumbUrl) + "')\">" +
                                        "<div class='sow-menu__map-preview-meta'>" +
                                            "<strong>" + esc(selectedMap.display_name) + "</strong>" +
                                            "<small>" + selectedMap.width + " × " + selectedMap.height + " TILES</small>" +
                                        "</div>" +
                                    "</div>" +
                                    "<label class='sow-menu__form-field' style='margin-top:12px;'>SELECT MAP" +
                                        "<select class='sow-menu__select' name='map_name'>" + mapCatalogOptions + "</select>" +
                                    "</label>" +
                                "</div>" +
                                "<div class='sow-menu__custom-card'>" +
                                    "<label class='sow-menu__form-field'>GAME TYPE" +
                                        "<div class='sow-menu__pill-group'>" + modeOptionsHtml + "</div>" +
                                    "</label>" +
                                    "<div class='sow-menu__form-row' style='margin-top:12px;'>" +
                                        "<label class='sow-menu__form-field'>BOT DIFFICULTY" +
                                            "<div class='sow-menu__pill-group'>" + diffOptionsHtml + "</div>" +
                                        "</label>" +
                                        "<label class='sow-menu__form-field'>SPAWN RULES" +
                                            "<div class='sow-menu__pill-group'>" + spawnOptionsHtml + "</div>" +
                                        "</label>" +
                                    "</div>" +
                                "</div>" +
                                "<div class='sow-menu__custom-card'>" + spControls + "</div>" +
                            "</div>" +
                            "<div class='sow-menu__custom-col'>" +
                                "<div class='sow-menu__custom-card'>" +
                                    "<p class='sow-menu__panel-sublabel'>POPULATION &amp; SCALE</p>" +
                                    "<div class='sow-menu__slider-field'>" +
                                        "<div class='sow-menu__slider-label'><span>MAX HUMAN PLAYERS</span><b data-val-for='max_players'>" + (config.max_players || 8) + "</b></div>" +
                                        "<input class='sow-menu__range' name='max_players' type='range' min='2' max='16' step='1' value='" + (config.max_players || 8) + "'>" +
                                    "</div>" +
                                    "<div class='sow-menu__slider-field'>" +
                                        "<div class='sow-menu__slider-label'><span>TRIBES (NEUTRAL BOTS)</span><b data-val-for='bot_count'>" + (config.bot_count != null ? config.bot_count : 128) + "</b></div>" +
                                        "<input class='sow-menu__range' name='bot_count' type='range' min='0' max='1000' step='8' value='" + (config.bot_count != null ? config.bot_count : 128) + "'>" +
                                    "</div>" +
                                    "<div class='sow-menu__slider-field'>" +
                                        "<div class='sow-menu__slider-label'><span>AI NATIONS (COMPLEX AI)</span><b data-val-for='nation_count'>" + (config.nation_count != null ? config.nation_count : 32) + "</b></div>" +
                                        "<input class='sow-menu__range' name='nation_count' type='range' min='0' max='400' step='4' value='" + (config.nation_count != null ? config.nation_count : 32) + "'>" +
                                    "</div>" +
                                "</div>" +
                                renderFeedback() +
                                "<div class='sow-menu__custom-actions'>" +
                                    "<button class='sow-menu__primary sow-menu__custom-launch-btn' type='submit'>" +
                                        (isSp ? "START SIMULATION" : "CREATE ONLINE LOBBY") + " <span>↗</span>" +
                                    "</button>" +
                                    "<button class='sow-menu__ghost-button' type='button' data-command='close_overlay'>CANCEL</button>" +
                                "</div>" +
                            "</div>" +
                        "</form>" +
                    "</section>" +
                "</main>" +
                renderFooter("CUSTOM MATCH CREATOR") +
            "</div>";
    }

    function joinedLobby() {
        var id = state.joined_lobby_id || state.pending_lobby_id;
        return (state.lobbies || []).find(function (lobby) { return lobby.id === id; }) || null;
    }

    function renderQueuePlayerRow(player, lobby) {
        var leader = leaderById(player.leader);
        var avatarUrl = asset("gameplay/avatars/" + leader.slug + ".webp");
        var isMe = player.player_id === state.my_player_id;
        var isHost = lobby && lobby.host_name === player.name;
        var canModerate = lobby && lobby.kind === "Custom" && state.is_lobby_host && !isMe;

        var statusBadge = player.download_progress === 100 || player.is_ready ?
            "<span class='sow-menu__sync-badge ready'>READY</span>" :
            "<span class='sow-menu__sync-badge syncing'>SYNC " + (player.download_progress || 0) + "%</span>";

        var controls = canModerate ?
            "<div class='sow-menu__player-mod-actions'>" +
                (lobby.game_mode === "Teams" ? "<button type='button' class='sow-menu__mod-btn' data-command='move_player_team' data-lobby-id='" + lobby.id + "' data-player-id='" + player.player_id + "'>MOVE TEAM</button>" : "") +
                "<button type='button' class='sow-menu__mod-btn' data-command='kick_player' data-lobby-id='" + lobby.id + "' data-player-id='" + player.player_id + "'>KICK</button>" +
                "<button type='button' class='sow-menu__mod-btn danger' data-command='ban_player' data-lobby-id='" + lobby.id + "' data-player-id='" + player.player_id + "'>BAN</button>" +
            "</div>" : "";

        return "" +
            "<div class='sow-menu__roster-row" + (isMe ? " is-me" : "") + "'>" +
                "<div class='sow-menu__roster-left'>" +
                    "<div class='sow-menu__roster-avatar' style=\"background-image:url('" + esc(avatarUrl) + "')\"></div>" +
                    "<div class='sow-menu__roster-name-group'>" +
                        "<strong>" + esc(player.name) + "</strong>" +
                        (isMe ? "<small class='sow-menu__me-tag'>YOU</small>" : "") +
                        (isHost ? "<small class='sow-menu__host-tag'>HOST</small>" : "") +
                    "</div>" +
                "</div>" +
                "<div class='sow-menu__roster-right'>" +
                    statusBadge +
                    controls +
                "</div>" +
            "</div>";
    }

    function renderQueue() {
        var lobby = joinedLobby();
        var title = lobby ? (lobby.map_name || "WORLD MAP").toUpperCase() : "MATCHMAKING";
        var mapThumbUrl = lobbyThumb(lobby || { map_name: "world" });
        var isTeams = lobby && lobby.game_mode === "Teams";
        var isCustom = lobby && lobby.kind === "Custom";
        var isHost = state && state.is_lobby_host && isCustom;

        var modeClass = "sow-menu__mode-chip--" + (lobby ? lobby.game_mode.toLowerCase() : "ffa");
        var modeLabel = lobby ? ({ FFA: "FREE FOR ALL", Teams: "TEAMS (RED VS BLUE)", HumansVsNations: "HUMANS VS NATIONS" }[lobby.game_mode] || lobby.game_mode) : "MATCHMAKING";

        var feedback = "";
        if (state.is_downloading_map) {
            var pct = state.map_download_progress || 0;
            feedback = "" +
                "<div class='sow-menu__map-download-wrap'>" +
                    "<div class='sow-menu__map-download-text'>DOWNLOADING MAP: " + esc(state.downloading_map_name || title) + " · " + pct + "%</div>" +
                    "<div class='sow-menu__map-download-bar'><div class='sow-menu__map-download-fill' style='width:" + pct + "%'></div></div>" +
                "</div>";
        } else if (state.error) {
            feedback = "<div class='sow-menu__queue-feedback sow-menu__queue-feedback--error'>" + esc(state.error) + "</div>";
        }

        var rosterHtml = "";
        var players = (lobby && lobby.players) || [];
        if (!players.length) {
            rosterHtml = "<div class='sow-menu__empty'>Connecting to tactical server...</div>";
        } else if (isTeams) {
            var redPlayers = players.filter(function (p) { return p.team === "Red"; });
            var bluePlayers = players.filter(function (p) { return p.team !== "Red"; });

            rosterHtml = "" +
                "<div class='sow-menu__teams-roster'>" +
                    "<div class='sow-menu__team-col sow-menu__team-col--red'>" +
                        "<div class='sow-menu__team-header'>🔴 RED TEAM (" + redPlayers.length + ")</div>" +
                        "<div class='sow-menu__team-list'>" + redPlayers.map(function (p) { return renderQueuePlayerRow(p, lobby); }).join("") + "</div>" +
                    "</div>" +
                    "<div class='sow-menu__team-col sow-menu__team-col--blue'>" +
                        "<div class='sow-menu__team-header'>🔵 BLUE TEAM (" + bluePlayers.length + ")</div>" +
                        "<div class='sow-menu__team-list'>" + bluePlayers.map(function (p) { return renderQueuePlayerRow(p, lobby); }).join("") + "</div>" +
                    "</div>" +
                "</div>";
        } else {
            rosterHtml = "<div class='sow-menu__ffa-roster'>" + players.map(function (p) { return renderQueuePlayerRow(p, lobby); }).join("") + "</div>";
        }

        return "" +
            "<div class='sow-menu__backdrop'></div>" +
            "<div class='sow-menu__shell'>" +
                renderTopbar() +
                "<main class='sow-menu__main sow-menu__main--queue'>" +
                    "<section class='sow-menu__queue-summary-card'>" +
                        "<div class='sow-menu__queue-map-hero' style=\"background-image:url('" + esc(mapThumbUrl) + "')\">" +
                            "<div class='sow-menu__queue-map-overlay'>" +
                                "<span class='sow-menu__mode-chip " + modeClass + "'>" + esc(modeLabel) + "</span>" +
                                "<h3>" + esc(title) + "</h3>" +
                            "</div>" +
                        "</div>" +
                        "<div class='sow-menu__queue-countdown' data-live-countdown></div>" +
                        feedback +
                        "<div class='sow-menu__queue-info-table'>" +
                            (lobby && lobby.host_name ? "<div class='sow-menu__info-row'><span>HOST</span><strong>" + esc(lobby.host_name) + "</strong></div>" : "") +
                            (isCustom && lobby ? "<div class='sow-menu__info-row'><span>ROOM CODE</span><div class='sow-menu__code-box'><strong>" + lobby.id + "</strong><button type='button' data-command='copy_lobby_code' data-lobby-id='" + lobby.id + "'>COPY</button></div></div>" : "") +
                            (lobby && lobby.bot_count > 0 ? "<div class='sow-menu__info-row'><span>TRIBES</span><strong>" + lobby.bot_count + " (" + esc(lobby.bot_difficulty || "Vanilla") + ")</strong></div>" : "") +
                            (lobby && lobby.nation_count > 0 ? "<div class='sow-menu__info-row'><span>NATIONS</span><strong>" + lobby.nation_count + "</strong></div>" : "") +
                            (lobby && lobby.has_password ? "<div class='sow-menu__info-row'><span>ACCESS</span><strong class='sow-menu__lock-tag'>🔒 PASSWORD</strong></div>" : "") +
                        "</div>" +
                        "<div class='sow-menu__queue-action-bar'>" +
                            (isHost ? "<button class='sow-menu__primary sow-menu__queue-start-btn' type='button' data-command='start_private' data-lobby-id='" + lobby.id + "'>START GAME <span>↗</span></button>" : "") +
                            "<button class='sow-menu__danger sow-menu__queue-leave-btn' type='button' data-command='leave_lobby'>LEAVE LOBBY <span>×</span></button>" +
                        "</div>" +
                    "</section>" +
                    "<section class='sow-menu__queue-players-card'>" +
                        "<div class='sow-menu__queue-players-head'>" +
                            "<p class='sow-menu__panel-label'>COMBAT ROSTER</p>" +
                            "<span class='sow-menu__player-count-badge'>" + (lobby ? (lobby.num_players || 0) + " / " + (lobby.max_players || 8) + " PLAYERS" : "—") + "</span>" +
                        "</div>" +
                        "<div class='sow-menu__queue-roster-wrap'>" + rosterHtml + "</div>" +
                    "</section>" +
                "</main>" +
                renderFooter("LIVE ROOM") +
            "</div>";
    }

    function renderLeaderPicker() {
        var leaders = state && Array.isArray(state.leaders) ? state.leaders : [];
        var activeId = tempSelectedLeader || (state && state.selected_leader) || "Caesar";
        var activeLeader = leaderById(activeId);
        var reign = LEADER_REIGNS[activeLeader.id] || "";
        var heroPortrait = asset("gameplay/avatars/" + activeLeader.slug + ".webp");

        var listHtml = leaders.map(function (leader) {
            var isSelected = leader.id === activeId;
            var avatarUrl = asset("gameplay/avatars/" + leader.slug + ".webp");
            return "" +
                "<button class='sow-menu__leader-card" + (isSelected ? " selected" : "") + "' type='button' data-command='preview_leader' data-leader-id='" + esc(leader.id) + "'>" +
                    "<div class='sow-menu__leader-card-avatar' style=\"background-image:url('" + esc(avatarUrl) + "')\">" +
                        (isSelected ? "<span class='sow-menu__leader-card-check'>✓</span>" : "") +
                    "</div>" +
                    "<div class='sow-menu__leader-card-info'>" +
                        "<strong>" + esc(leader.name) + "</strong>" +
                        "<small>" + esc(leader.civilization) + "</small>" +
                    "</div>" +
                "</button>";
        }).join("");

        return "" +
            "<div class='sow-menu__overlay'>" +
                "<section class='sow-menu__modal sow-menu__leader-modal'>" +
                    "<div class='sow-menu__modal-head'>" +
                        "<div>" +
                            "<p class='sow-menu__panel-label'>LEADERS OF HISTORY</p>" +
                            "<h2>SELECT LEADER</h2>" +
                        "</div>" +
                        "<button class='sow-menu__icon-button' type='button' data-command='close_leader_picker' aria-label='Close'>×</button>" +
                    "</div>" +
                    "<div class='sow-menu__leader-layout'>" +
                        "<div class='sow-menu__leader-showcase'>" +
                            "<div class='sow-menu__leader-hero-frame' style=\"background-image:url('" + esc(heroPortrait) + "')\"></div>" +
                            "<div class='sow-menu__leader-hero-details'>" +
                                "<div class='sow-menu__leader-hero-meta'>" +
                                    "<span class='sow-menu__civ-badge'>" + esc(activeLeader.civilization) + "</span>" +
                                    (reign ? "<span class='sow-menu__reign-badge'>" + esc(reign) + "</span>" : "") +
                                "</div>" +
                                "<h3>" + esc(activeLeader.name) + "</h3>" +
                                "<div class='sow-menu__perk-card'>" +
                                    "<div class='sow-menu__perk-title'>⚡ SPECIAL TRAIT &amp; PERK</div>" +
                                    "<p class='sow-menu__perk-desc'>" + esc(activeLeader.perk || "Enhanced military & empire bonuses.") + "</p>" +
                                "</div>" +
                                "<button class='sow-menu__primary sow-menu__leader-confirm-btn' type='button' data-command='confirm_leader' data-leader-id='" + esc(activeLeader.id) + "'>" +
                                    "CONFIRM " + esc(activeLeader.name.toUpperCase()) + " <span>✓</span>" +
                                "</button>" +
                            "</div>" +
                        "</div>" +
                        "<div class='sow-menu__leader-grid-wrap'>" +
                            "<p class='sow-menu__panel-sublabel'>AVAILABLE CIVILIZATIONS (" + leaders.length + ")</p>" +
                            "<div class='sow-menu__leader-grid'>" + listHtml + "</div>" +
                        "</div>" +
                    "</div>" +
                "</section>" +
            "</div>";
    }

    function renderSettings() {
        var settings = state.settings || {};
        var fullscreen = document.fullscreenElement ? "EXIT FULLSCREEN" : "FULLSCREEN";
        var vol = settings.music_volume == null ? 0.8 : settings.music_volume;
        var volPct = Math.round(vol * 100);
        return "" +
            "<div class='sow-menu__overlay'>" +
                "<section class='sow-menu__modal sow-menu__settings-modal'>" +
                    "<div class='sow-menu__modal-head'>" +
                        "<div>" +
                            "<p class='sow-menu__panel-label'>SYSTEM CONFIGURATION</p>" +
                            "<h2>SETTINGS</h2>" +
                        "</div>" +
                        "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='Close'>×</button>" +
                    "</div>" +
                    "<div class='sow-menu__form-grid'>" +
                        "<label class='sow-menu__form-field sow-menu__form-field--wide'>" +
                            "<span>MASTER AUDIO</span>" +
                            "<select class='sow-menu__select' name='mute_all' data-setting='mute'>" +
                                "<option value='on' " + (!settings.mute_all ? "selected" : "") + ">AUDIO ENABLED (ON)</option>" +
                                "<option value='off' " + (settings.mute_all ? "selected" : "") + ">MUTED (OFF)</option>" +
                            "</select>" +
                        "</label>" +
                        "<label class='sow-menu__form-field sow-menu__form-field--wide'>" +
                            "<div class='sow-menu__slider-label'><span>MUSIC VOLUME</span><b data-val-for='music_vol'>" + volPct + "%</b></div>" +
                            "<input class='sow-menu__field' type='range' name='music_volume' min='0' max='1' step='0.05' value='" + esc(vol) + "' data-setting='music_volume'>" +
                        "</label>" +
                        "<label class='sow-menu__form-field sow-menu__form-field--wide'>" +
                            "<span>MOTION &amp; ANIMATION</span>" +
                            "<select class='sow-menu__select' name='reduced_motion' data-setting='reduced_motion'>" +
                                "<option value='full' " + (!settings.reduced_motion ? "selected" : "") + ">FULL KINETIC DYNAMICS</option>" +
                                "<option value='reduced' " + (settings.reduced_motion ? "selected" : "") + ">REDUCED MOTION</option>" +
                            "</select>" +
                        "</label>" +
                    "</div>" +
                    "<button class='sow-menu__secondary sow-menu__fullscreen-btn' type='button' data-command='toggle_fullscreen'>" +
                        "⛶ " + fullscreen +
                    "</button>" +
                    "<div class='sow-menu__modal-actions'>" +
                        "<button class='sow-menu__primary' type='button' data-command='toggle_settings'>DONE <span>✓</span></button>" +
                    "</div>" +
                "</section>" +
            "</div>";
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
        root.dataset.screen = screen;
        root.dataset.ready = typeof window.SOW_menu_command === "function" ? "true" : "false";
        root.hidden = state.phase !== "MainMenu";

        var activeEl = document.activeElement;
        var isTyping = activeEl && root.contains(activeEl) && (activeEl.tagName === "INPUT" || activeEl.tagName === "TEXTAREA" || activeEl.tagName === "SELECT");
        var activeRole = activeEl && activeEl.dataset ? activeEl.dataset.role : null;
        var activeName = activeEl ? activeEl.name : null;
        var activeVal = isTyping ? activeEl.value : null;
        var selStart = isTyping && typeof activeEl.selectionStart === "number" ? activeEl.selectionStart : null;
        var selEnd = isTyping && typeof activeEl.selectionEnd === "number" ? activeEl.selectionEnd : null;

        if (screen === "home") root.innerHTML = renderHome();
        else if (screen === "browser") root.innerHTML = renderBrowser();
        else if (screen === "create") root.innerHTML = renderCreate();
        else if (screen === "queue") root.innerHTML = renderQueue();
        else root.innerHTML = "";

        if (isTyping) {
            var restored = null;
            if (activeRole) {
                restored = root.querySelector("[data-role='" + activeRole + "']");
            } else if (activeName) {
                restored = root.querySelector("[name='" + activeName + "']");
            }
            if (restored) {
                if (activeVal != null) restored.value = activeVal;
                try {
                    restored.focus();
                    if (selStart != null && selEnd != null) {
                        restored.setSelectionRange(selStart, selEnd);
                    }
                } catch (e) {}
            }
        }
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
            tempSelectedLeader = state ? state.selected_leader : "Caesar";
            settingsOpen = false;
            render();
            return;
        }
        if (command === "close_leader_picker") {
            leaderPickerOpen = false;
            tempSelectedLeader = null;
            render();
            return;
        }
        if (command === "preview_leader") {
            tempSelectedLeader = target.dataset.leaderId;
            render();
            return;
        }
        if (command === "confirm_leader") {
            var leaderId = target.dataset.leaderId || tempSelectedLeader;
            if (leaderId) {
                send("set_leader", { leader_id: leaderId });
                leaderPickerOpen = false;
                tempSelectedLeader = null;
                render();
            }
            return;
        }
        if (command === "set_session_mode") {
            createOffline = target.dataset.mode === "offline";
            render();
            return;
        }
        if (command === "set_create_mode") {
            if (!createDraft) createDraft = cloneConfig();
            createDraft.game_mode = target.dataset.mode;
            render();
            return;
        }
        if (command === "set_create_diff") {
            if (!createDraft) createDraft = cloneConfig();
            createDraft.bot_difficulty = target.dataset.diff;
            render();
            return;
        }
        if (command === "set_create_spawn") {
            if (!createDraft) createDraft = cloneConfig();
            createDraft.random_spawn = target.dataset.spawn === "true";
            render();
            return;
        }
        if (command === "set_create_private") {
            createPrivate = target.dataset.private === "true";
            render();
            return;
        }
        if (command === "randomize_seed") {
            if (!createDraft) createDraft = cloneConfig();
            createDraft.seed = Math.floor(Math.random() * 9999) + 1;
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
        if (event.key === "Enter") {
            var input = event.target;
            if (input && input.dataset && input.dataset.role === "display-name") {
                event.preventDefault();
                input.blur();
                return;
            }
        }
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
        var input = event.target;
        if (input.dataset && input.dataset.role === "display-name") {
            displayNameDraft = input.value;
        }
        if (input.dataset && input.dataset.role === "browser-search") {
            browserSearchQuery = input.value;
            var publicPanel = root.querySelector(".sow-menu__public");
            if (publicPanel) {
                var lobbies = publicLobbies(false);
                if (browserSearchQuery) {
                    var q = browserSearchQuery.toLowerCase().trim();
                    lobbies = lobbies.filter(function (l) {
                        return (l.map_name && l.map_name.toLowerCase().indexOf(q) !== -1) ||
                               (l.host_name && l.host_name.toLowerCase().indexOf(q) !== -1) ||
                               (l.game_mode && l.game_mode.toLowerCase().indexOf(q) !== -1);
                    });
                }
                var cards = lobbies.map(renderLobbyCard).join("");
                if (!cards) cards = "<div class='sow-menu__empty'>No public games match your search.</div>";
                var lobbiesContainer = publicPanel.querySelector(".sow-menu__lobbies");
                if (lobbiesContainer) lobbiesContainer.innerHTML = cards;
                var label = publicPanel.querySelector(".sow-menu__panel-label");
                if (label) label.textContent = "PUBLIC GAMES (" + lobbies.length + ")";
            }
        }
        var createForm = input.closest("form[data-form='create']");
        if (createForm) {
            syncCreateDraft(createForm);
            var valBadge = createForm.querySelector("[data-val-for='" + input.name + "']");
            if (valBadge) valBadge.textContent = input.value;
        }
        if (input.dataset && input.dataset.setting === "music_volume") {
            var musicValBadge = root.querySelector("[data-val-for='music_vol']");
            if (musicValBadge) musicValBadge.textContent = Math.round(Number(input.value) * 100) + "%";
        }
        if (input.name === "password" && input.closest("form[data-form='password']")) {
            passwordDraft = input.value;
        }
    });

    root.addEventListener("focusout", function (event) {
        var input = event.target;
        if (input.dataset.role !== "display-name" || state.name_locked) return;
        var name = input.value.trim();
        displayNameDraft = null;
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

    // ─────────────────────────────────────────────────────────
    // HUD CONTROLLER (DOM Overlay for In-Game RTS Matches)
    // ─────────────────────────────────────────────────────────
    var hudRoot = document.getElementById("sow-hud");
    var leaderboardOpen = false;
    var inboxOpen = false;
    var transferOpen = false;
    var betrayalOpen = false;
    var pinEmoji = false;
    var surrenderModalOpen = false;
    var emojiPickerOpen = false;
    var utilitiesOpen = false;
    var hudInitialized = false;
    var hudRefs = null;
    var leaderboardRows = Object.create(null);

    var EMOJIS = [
        "😀", "😎", "😏", "😂", "🤣", "😋", "😉", "😜", "😍", "🥰", "🥳", "🥺", "😇", "🤩", "👍",
        "❤️", "😮", "🤔", "🧐", "🙄", "🤯", "🤡", "💩", "🤫", "😠", "😡", "🤬", "😤", "🥵", "🥶",
        "🤢", "🤮", "⚔️", "🛡️", "🏹", "💣", "💥", "💀", "👑", "💪", "🔥", "👀", "🏳️", "🤝", "💔",
        "🔌", "⭐", "🐺"
    ];

    function ensureHudDom() {
        if (!hudRoot || hudInitialized) return;
        hudInitialized = true;
        hudRoot.innerHTML = ''
            + '<header class="sow-hud__topbar">'
            + '  <div class="sow-hud__status-left">'
            + '    <div class="sow-hud__resource sow-hud__gold" title="Gold Treasury"><span class="sow-hud__icon">🪙</span> <b data-role="gold">0</b></div>'
            + '    <div class="sow-hud__resource sow-hud__troops" title="Troop Pool"><span class="sow-hud__icon">🌾</span> <b data-role="troops">0</b></div>'
            + '    <div class="sow-hud__resource sow-hud__prod" title="Production"><span class="sow-hud__icon">⚙️</span> <b data-role="prod">+0/s</b></div>'
            + '  </div>'
            + '  <div class="sow-hud__status-right">'
            + '    <button class="sow-hud__utility-toggle" type="button" data-command="toggle_utilities" aria-expanded="false" aria-label="Match utilities">•••</button>'
            + '    <div class="sow-hud__utilities" id="sow-hud-utilities">'
            + '      <span class="sow-hud__fps" id="sow-hud-fps">60 FPS</span>'
            + '      <button class="sow-hud__btn sow-hud__btn-ghost" type="button" data-command="toggle_inbox" title="Alliance and resource requests">📩 <span id="sow-hud-inbox-count">0</span></button>'
            + '      <button class="sow-hud__btn sow-hud__btn-ghost" type="button" data-command="toggle_leaderboard" title="Toggle Conquest Rankings">🏆 RANKINGS</button>'
            + '      <button class="sow-hud__btn sow-hud__btn-danger" type="button" data-command="prompt_surrender" title="Leave Match">✕ EXIT</button>'
            + '    </div>'
            + '  </div>'
            + '</header>'
            + '<div class="sow-hud__hover-card hidden" id="sow-hud-hover-card">'
            + '  <div class="sow-hud__hover-header"><span id="sow-hud-hover-avatar">👑</span> <b id="sow-hud-hover-name">Territory</b></div>'
            + '  <div class="sow-hud__hover-stats">'
            + '    <span><b id="sow-hud-hover-pct">0%</b> Land</span>'
            + '    <span><b id="sow-hud-hover-troops">0</b> ⚔</span>'
            + '    <span><b id="sow-hud-hover-gold">0</b> 🪙</span>'
            + '  </div>'
            + '  <div class="sow-hud__hover-buildings" id="sow-hud-hover-blds"></div>'
            + '</div>'
            + '<aside class="sow-hud__left-rail" id="sow-hud-left-rail">'
            + '  <div class="sow-hud__rail-card">'
            + '    <div class="sow-hud__slider-vertical-wrap">'
            + '      <input type="range" min="5" max="100" value="50" step="5" class="sow-hud__range-vertical" id="sow-hud-slider">'
            + '    </div>'
            + '  </div>'
            + '</aside>'
            + '<aside class="sow-hud__right-rail" id="sow-hud-right-rail">'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="zoom_in" title="Zoom In (➕)">➕</button>'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="zoom_out" title="Zoom Out (➖)">➖</button>'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="center_camera" title="Center Capital (🏠)">🏠</button>'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="toggle_emoji" title="Express Emote (😀)">😀</button>'
            + '</aside>'
            + '<div class="sow-hud__emoji-popout hidden" id="sow-hud-emoji-popout">'
            + '  <div class="sow-hud__emoji-header">'
            + '    <span>EXPRESS REACTION</span>'
            + '    <button type="button" class="sow-hud__pin-btn" data-command="toggle_pin_emoji" aria-pressed="false">PIN</button>'
            + '    <button type="button" class="sow-hud__close-btn" data-command="toggle_emoji">✕</button>'
            + '  </div>'
            + '  <div class="sow-hud__emoji-grid" id="sow-hud-emoji-grid"></div>'
            + '</div>'
            + '<footer class="sow-hud__dock" id="sow-hud-dock">'
            + '  <div class="sow-hud__dock-inner">'
            + '    <div class="sow-hud__dock-tabs" role="tablist" aria-label="Match panels">'
            + '      <button type="button" class="sow-hud__dock-tab active" data-command="set_bottom_tab" data-tab="controls">⚔</button>'
            + '      <button type="button" class="sow-hud__dock-tab" data-command="set_bottom_tab" data-tab="battle_log">📜</button>'
            + '      <button type="button" class="sow-hud__dock-tab" data-command="set_bottom_tab" data-tab="event_log">📋</button>'
            + '    </div>'
            + '    <button type="button" class="sow-hud__deploy-btn" data-command="spawn_troops" id="sow-hud-deploy-btn">'
            + '      <span>DEPLOY REINFORCEMENTS</span>'
            + '      <small id="sow-hud-deploy-timer">READY</small>'
            + '    </button>'
            + '    <div class="sow-hud__buildings-strip" id="sow-hud-buildings-strip">'
            + '      <button type="button" class="sow-hud__bld-btn" data-command="build_structure" data-kind="City" id="sow-hud-bld-city">'
            + '        <span class="sow-hud__bld-icon">🏛️</span>'
            + '        <b class="sow-hud__bld-name">City</b>'
            + '        <small class="sow-hud__bld-cost" id="sow-hud-cost-city">100g</small>'
            + '      </button>'
            + '      <button type="button" class="sow-hud__bld-btn" data-command="build_structure" data-kind="Factory" id="sow-hud-bld-factory">'
            + '        <span class="sow-hud__bld-icon">🏭</span>'
            + '        <b class="sow-hud__bld-name">Factory</b>'
            + '        <small class="sow-hud__bld-cost" id="sow-hud-cost-factory">200g</small>'
            + '      </button>'
            + '      <button type="button" class="sow-hud__bld-btn" data-command="build_structure" data-kind="Port" id="sow-hud-bld-port">'
            + '        <span class="sow-hud__bld-icon">⚓</span>'
            + '        <b class="sow-hud__bld-name">Port</b>'
            + '        <small class="sow-hud__bld-cost" id="sow-hud-cost-port">150g</small>'
            + '      </button>'
            + '      <button type="button" class="sow-hud__bld-btn" data-command="build_structure" data-kind="Bunker" id="sow-hud-bld-bunker">'
            + '        <span class="sow-hud__bld-icon">🛡️</span>'
            + '        <b class="sow-hud__bld-name">Bunker</b>'
            + '        <small class="sow-hud__bld-cost" id="sow-hud-cost-bunker">75g</small>'
            + '      </button>'
            + '      <button type="button" class="sow-hud__cancel-btn hidden" data-command="cancel_placement" id="sow-hud-cancel-placement">✕ Cancel</button>'
            + '    </div>'
            + '  </div>'
            + '</footer>'
            + '<aside class="sow-hud__leaderboard hidden" id="sow-hud-leaderboard">'
            + '  <div class="sow-hud__panel-header">'
            + '    <h3>CONQUEST RANKINGS</h3>'
            + '    <button class="sow-hud__close-btn" type="button" data-command="toggle_leaderboard">✕</button>'
            + '  </div>'
            + '  <div class="sow-hud__leaderboard-rows" id="sow-hud-lb-rows"></div>'
            + '</aside>'
            + '<aside class="sow-hud__panel sow-hud__inbox hidden" id="sow-hud-inbox">'
            + '  <div class="sow-hud__panel-header"><h3>INBOX</h3><button class="sow-hud__close-btn" type="button" data-command="toggle_inbox">✕</button></div>'
            + '  <div class="sow-hud__panel-rows" id="sow-hud-inbox-rows"></div>'
            + '</aside>'
            + '<aside class="sow-hud__panel sow-hud__log-panel hidden" id="sow-hud-log-panel">'
            + '  <div class="sow-hud__panel-header"><h3 id="sow-hud-log-title">BATTLE LOG</h3><button class="sow-hud__close-btn" type="button" data-command="set_bottom_tab" data-tab="controls">✕</button></div>'
            + '  <div class="sow-hud__panel-rows" id="sow-hud-log-rows"></div>'
            + '  <button class="sow-hud__clear-log hidden" id="sow-hud-clear-log" type="button" data-command="clear_event_log">CLEAR EVENT LOG</button>'
            + '</aside>'
            + '<aside class="sow-hud__panel sow-hud__transfer hidden" id="sow-hud-transfer">'
            + '  <div class="sow-hud__panel-header"><h3>RESOURCE TRANSFER</h3><button class="sow-hud__close-btn" type="button" data-command="close_transfer">✕</button></div>'
            + '  <p id="sow-hud-transfer-target"></p>'
            + '  <label>GOLD<input id="sow-hud-transfer-gold" type="number" min="0" step="1" value="0"></label>'
            + '  <label>TROOPS<input id="sow-hud-transfer-troops" type="number" min="0" step="1" value="0"></label>'
            + '  <div class="sow-hud__panel-actions"><button type="button" data-command="send_resources">SEND</button><button type="button" data-command="request_resources">REQUEST</button></div>'
            + '</aside>'
            + '<div class="sow-hud__modal-backdrop hidden" id="sow-hud-betrayal-modal">'
            + '  <div class="sow-hud__modal-card"><h3>BREAK ALLIANCE?</h3><p id="sow-hud-betrayal-copy">Attacking this ally may turn other allies against you.</p><div class="sow-hud__panel-actions"><button type="button" data-command="cancel_betrayal">KEEP ALLIANCE</button><button class="sow-hud__btn-danger" type="button" data-command="confirm_betrayal">ATTACK</button></div></div>'
            + '</div>'
            + '<div class="sow-hud__modal-backdrop hidden" id="sow-hud-surrender-modal">'
            + '  <div class="sow-hud__modal-card">'
            + '    <h3 style="margin:0 0 10px;font-size:20px;color:var(--sow-red);">Leave Match?</h3>'
            + '    <p style="color:var(--sow-muted);font-size:14px;line-height:1.5;">Leave this match and return to the command screen?</p>'
            + '    <div style="display:flex;justify-content:center;gap:12px;margin-top:20px;">'
            + '      <button class="sow-hud__btn sow-hud__btn-ghost" type="button" data-command="close_surrender_modal">CANCEL</button>'
            + '      <button class="sow-hud__btn sow-hud__btn-danger" type="button" data-command="confirm_surrender">LEAVE MATCH</button>'
            + '    </div>'
            + '  </div>'
            + '</div>'
            + '<div class="sow-hud__endgame-backdrop hidden" id="sow-hud-endgame-modal">'
            + '  <div class="sow-hud__endgame-card">'
            + '    <div class="sow-hud__endgame-banner" id="sow-hud-endgame-banner">VICTORY</div>'
            + '    <h2 style="margin:0 0 12px;font-size:24px;" id="sow-hud-endgame-title">The World is Yours</h2>'
            + '    <p style="color:var(--sow-muted);font-size:14px;margin-bottom:24px;" id="sow-hud-endgame-desc">Your conquest is complete.</p>'
            + '    <div class="sow-hud__endgame-stats" id="sow-hud-endgame-stats"></div>'
            + '    <button class="sow-hud__btn" type="button" data-command="confirm_endgame_leave" style="background:var(--sow-gold);color:#0d0f13;font-weight:800;padding:12px 28px;">RETURN TO COMMAND</button>'
            + '  </div>'
            + '</div>';

        var emojiGrid = document.getElementById("sow-hud-emoji-grid");
        if (emojiGrid) {
            EMOJIS.forEach(function (emoji) {
                var cell = document.createElement("button");
                cell.type = "button";
                cell.className = "sow-hud__emoji-cell";
                cell.textContent = emoji;
                cell.dataset.command = "express_emoji";
                cell.dataset.emoji = emoji;
                emojiGrid.appendChild(cell);
            });
        }

        hudRefs = {
            gold: hudRoot.querySelector('[data-role="gold"]'),
            troops: hudRoot.querySelector('[data-role="troops"]'),
            prod: hudRoot.querySelector('[data-role="prod"]'),
            fps: document.getElementById("sow-hud-fps"),
            utilities: document.getElementById("sow-hud-utilities"),
            utilityToggle: hudRoot.querySelector("[data-command='toggle_utilities']"),
            inboxCount: document.getElementById("sow-hud-inbox-count"),
            hoverCard: document.getElementById("sow-hud-hover-card"),
            hoverAvatar: document.getElementById("sow-hud-hover-avatar"),
            hoverName: document.getElementById("sow-hud-hover-name"),
            hoverPct: document.getElementById("sow-hud-hover-pct"),
            hoverTroops: document.getElementById("sow-hud-hover-troops"),
            hoverGold: document.getElementById("sow-hud-hover-gold"),
            hoverBlds: document.getElementById("sow-hud-hover-blds"),
            leftRail: document.getElementById("sow-hud-left-rail"),
            slider: document.getElementById("sow-hud-slider"),
            rightRail: document.getElementById("sow-hud-right-rail"),
            emojiPopout: document.getElementById("sow-hud-emoji-popout"),
            pinEmoji: hudRoot.querySelector("[data-command='toggle_pin_emoji']"),
            dockTabs: Array.prototype.slice.call(hudRoot.querySelectorAll(".sow-hud__dock-tab")),
            dockTabsWrap: hudRoot.querySelector(".sow-hud__dock-tabs"),
            deployBtn: document.getElementById("sow-hud-deploy-btn"),
            deployTimer: document.getElementById("sow-hud-deploy-timer"),
            bldStrip: document.getElementById("sow-hud-buildings-strip"),
            bldCity: document.getElementById("sow-hud-bld-city"),
            bldFactory: document.getElementById("sow-hud-bld-factory"),
            bldPort: document.getElementById("sow-hud-bld-port"),
            bldBunker: document.getElementById("sow-hud-bld-bunker"),
            costCity: document.getElementById("sow-hud-cost-city"),
            costFactory: document.getElementById("sow-hud-cost-factory"),
            costPort: document.getElementById("sow-hud-cost-port"),
            costBunker: document.getElementById("sow-hud-cost-bunker"),
            cancelPlacement: document.getElementById("sow-hud-cancel-placement"),
            leaderboard: document.getElementById("sow-hud-leaderboard"),
            rows: document.getElementById("sow-hud-lb-rows"),
            inbox: document.getElementById("sow-hud-inbox"),
            inboxRows: document.getElementById("sow-hud-inbox-rows"),
            logPanel: document.getElementById("sow-hud-log-panel"),
            logTitle: document.getElementById("sow-hud-log-title"),
            logRows: document.getElementById("sow-hud-log-rows"),
            clearLog: document.getElementById("sow-hud-clear-log"),
            transfer: document.getElementById("sow-hud-transfer"),
            transferTarget: document.getElementById("sow-hud-transfer-target"),
            transferGold: document.getElementById("sow-hud-transfer-gold"),
            transferTroops: document.getElementById("sow-hud-transfer-troops"),
            betrayal: document.getElementById("sow-hud-betrayal-modal"),
            betrayalCopy: document.getElementById("sow-hud-betrayal-copy"),
            surrender: document.getElementById("sow-hud-surrender-modal"),
            endgame: document.getElementById("sow-hud-endgame-modal"),
            endgameBanner: document.getElementById("sow-hud-endgame-banner"),
            endgameTitle: document.getElementById("sow-hud-endgame-title"),
            endgameDesc: document.getElementById("sow-hud-endgame-desc"),
            endgameStats: document.getElementById("sow-hud-endgame-stats"),
            notifications: document.createElement("div")
        };

        hudRefs.notifications.className = "sow-hud__notifications";
        hudRefs.notifications.setAttribute("aria-live", "polite");
        hudRoot.appendChild(hudRefs.notifications);

        if (hudRefs.slider) {
            hudRefs.slider.addEventListener("input", function (e) {
                var val = parseFloat(e.target.value);
                var ratio = val / 100.0;
                send("set_attack_ratio", { ratio: ratio });
            });
        }
    }

    function updateLeaderboard(players) {
        if (!hudRefs || !hudRefs.rows) return;
        if (!Array.isArray(players)) return;
        var nextRows = Object.create(null);
        var fragment = document.createDocumentFragment();
        (players || []).forEach(function (player, idx) {
            var key = String(player.id);
            var row = leaderboardRows[key];
            if (!row) {
                var card = document.createElement("div");
                var left = document.createElement("div");
                var rank = document.createElement("span");
                var status = document.createElement("span");
                var name = document.createElement("b");
                var right = document.createElement("div");
                var territory = document.createElement("span");
                var troops = document.createElement("span");
                var transfer = document.createElement("button");
                card.className = "sow-hud__player-card";
                card.dataset.command = "focus_player";
                card.dataset.playerId = String(player.id);
                left.style.cssText = "display:flex;align-items:center;gap:6px;";
                right.style.cssText = "display:flex;gap:10px;font-weight:700;";
                transfer.type = "button";
                transfer.className = "sow-hud__row-action";
                transfer.dataset.command = "open_transfer";
                transfer.dataset.playerId = String(player.id);
                transfer.textContent = "GIFT";
                rank.style.cssText = "font-size:10px;color:var(--sow-muted);font-weight:800;";
                territory.style.color = "var(--sow-gold)";
                troops.style.color = "#86efac";
                left.appendChild(rank);
                left.appendChild(status);
                left.appendChild(name);
                right.appendChild(territory);
                right.appendChild(troops);
                right.appendChild(transfer);
                card.appendChild(left);
                card.appendChild(right);
                row = { card: card, rank: rank, status: status, name: name, territory: territory, troops: troops, key: "" };
            }

            var rowKey = [idx, player.name, player.troops, player.tile_count, player.is_alive, player.is_me].join("|");
            if (row.key !== rowKey) {
                row.key = rowKey;
                row.card.classList.toggle("is-me", !!player.is_me);
                row.card.classList.toggle("is-dead", !player.is_alive);
                row.rank.textContent = "#" + (idx + 1);
                row.status.textContent = player.is_alive ? (idx === 0 ? "👑" : "🛡️") : "💀";
                row.name.textContent = player.name || "Player";
                row.territory.textContent = Math.round((player.territory_pct || 0) * 100) + "%";
                row.troops.textContent = (player.troops > 1000 ? (player.troops / 1000).toFixed(1) + "k" : Math.floor(player.troops)) + " ⚔";
            }
            fragment.appendChild(row.card);
            nextRows[key] = row;
        });
        hudRefs.rows.replaceChildren(fragment);
        leaderboardRows = nextRows;
    }

    function appendPanelRows(container, rows) {
        if (!container || !Array.isArray(rows)) return;
        var fragment = document.createDocumentFragment();
        rows.forEach(function (row) { fragment.appendChild(row); });
        container.replaceChildren(fragment);
    }

    function panelRow(text) {
        var row = document.createElement("div");
        row.className = "sow-hud__panel-row";
        row.textContent = text;
        return row;
    }

    function renderInbox(requests) {
        if (!hudRefs || !Array.isArray(requests)) return;
        var rows = requests.map(function (request) {
            var row = panelRow(request.kind === "resources"
                ? (request.name || "Player") + " requests " + Math.floor(request.gold || 0) + " gold / " + Math.floor(request.troops || 0) + " troops"
                : (request.name || "Player") + " requests an alliance");
            var actions = document.createElement("span");
            actions.className = "sow-hud__panel-actions";
            [request.kind === "resources" ? "accept_resource_request" : "accept_alliance", request.kind === "resources" ? "reject_resource_request" : "reject_alliance"].forEach(function (command) {
                var button = document.createElement("button");
                button.type = "button";
                button.textContent = command.indexOf("reject") >= 0 ? "REJECT" : "ACCEPT";
                button.dataset.command = command;
                button.dataset.playerId = String(request.requester_id);
                actions.appendChild(button);
            });
            row.appendChild(actions);
            return row;
        });
        if (!rows.length) rows.push(panelRow("No pending requests."));
        appendPanelRows(hudRefs.inboxRows, rows);
    }

    function renderBattleLog(entries) {
        if (!hudRefs || !Array.isArray(entries)) return;
        var rows = entries.map(function (entry) {
            var row = panelRow((entry.kind === "incoming" ? "⚔ INCOMING" : entry.kind === "navy" ? "⛴ NAVY" : "🛡 OUTGOING") + " · " + Math.floor(entry.troops || 0) + " troops");
            var button = document.createElement("button");
            button.type = "button";
            button.textContent = entry.retreating ? "" : (entry.kind === "navy" ? "RECALL" : "CANCEL");
            button.dataset.command = entry.kind === "navy" ? "recall_fleet" : "cancel_attack";
            button.dataset[entry.kind === "navy" ? "fleetId" : "attackId"] = String(entry.id);
            button.disabled = Boolean(entry.retreating);
            row.appendChild(button);
            return row;
        });
        if (!rows.length) rows.push(panelRow("No active dispatches."));
        appendPanelRows(hudRefs.logRows, rows);
    }

    function renderEventLog(entries) {
        if (!hudRefs || !Array.isArray(entries)) return;
        var rows = entries.map(function (entry) { return panelRow(entry.message || "Event"); });
        if (!rows.length) rows.push(panelRow("No events yet."));
        appendPanelRows(hudRefs.logRows, rows);
    }

    function renderNotifications(entries) {
        if (!hudRefs || !hudRefs.notifications || !Array.isArray(entries)) return;
        var visible = entries.slice(-3);
        var key = visible.map(function (entry) { return entry.message || ""; }).join("\u001f");
        if (hudRefs.notifications.dataset.key === key) return;
        hudRefs.notifications.dataset.key = key;
        hudRefs.notifications.replaceChildren.apply(hudRefs.notifications, visible.map(function (entry) {
            var node = document.createElement("div");
            node.className = "sow-hud__notification";
            node.textContent = entry.message || "Event";
            return node;
        }));
    }

    function renderHud() {
        if (!hudRoot) return;
        if (!state || state.phase !== "Playing" || !state.hud) {
            hudRoot.hidden = true;
            leaderboardOpen = false;
            inboxOpen = false;
            transferOpen = false;
            betrayalOpen = false;
            emojiPickerOpen = false;
            utilitiesOpen = false;
            hudRoot.dataset.overlayOpen = "false";
            leaderboardRows = Object.create(null);
            if (hudRefs && hudRefs.rows) hudRefs.rows.replaceChildren();
            if (hudRefs && hudRefs.notifications) {
                hudRefs.notifications.dataset.key = "";
                hudRefs.notifications.replaceChildren();
            }
            return;
        }
        ensureHudDom();
        hudRoot.hidden = false;

        var hud = state.hud;
        var gold = Math.floor(hud.gold || 0);
        var troops = Math.floor(hud.troops || 0);
        var maxTroops = Math.floor(hud.max_troops || 0);
        var prod = Math.floor(hud.troop_rate || 0);
        var currentRatio = hud.attack_ratio || 0.5;
        var spawnSecs = hud.spawn_timer_secs;
        var isDeploying = spawnSecs != null && spawnSecs > 0;
        var bottomTab = hud.bottom_tab || "controls";

        if (hudRefs.gold && hudRefs.gold.dataset.val !== String(gold)) {
            hudRefs.gold.textContent = gold.toLocaleString();
            hudRefs.gold.dataset.val = String(gold);
        }

        if (hudRefs.troops) {
            var troopText = maxTroops > 0 ? troops.toLocaleString() + ' / ' + maxTroops.toLocaleString() : troops.toLocaleString();
            if (hudRefs.troops.dataset.val !== troopText) {
                hudRefs.troops.textContent = troopText;
                hudRefs.troops.dataset.val = troopText;
            }
        }

        if (hudRefs.prod && hudRefs.prod.dataset.val !== String(prod)) {
            hudRefs.prod.textContent = '+' + prod + '/s';
            hudRefs.prod.dataset.val = String(prod);
        }

        if (hudRefs.fps) {
            var fpsVal = hud.fps || 60;
            var pingVal = hud.ping || 0;
            hudRefs.fps.textContent = fpsVal + ' FPS' + (pingVal > 0 ? ' | ' + pingVal + 'ms' : '');
        }
        if (hudRefs.inboxCount) hudRefs.inboxCount.textContent = String(hud.inbox_count || 0);

        // Hover Card
        if (hudRefs.hoverCard) {
            var hov = hud.hovered;
            if (hov) {
                hudRefs.hoverCard.classList.remove("hidden");
                if (hudRefs.hoverName) hudRefs.hoverName.textContent = hov.name || "Territory";
                if (hudRefs.hoverPct) hudRefs.hoverPct.textContent = Math.round((hov.territory_pct || 0) * 100) + "%";
                if (hudRefs.hoverTroops) hudRefs.hoverTroops.textContent = (hov.troops > 1000 ? (hov.troops / 1000).toFixed(1) + "k" : Math.floor(hov.troops || 0)) + " ⚔";
                if (hudRefs.hoverGold) hudRefs.hoverGold.textContent = Math.floor(hov.gold || 0) + " 🪙";
                if (hudRefs.hoverBlds) {
                    var bldText = [];
                    if (hov.cities > 0) bldText.push("🏛️ x" + hov.cities);
                    if (hov.factories > 0) bldText.push("🏭 x" + hov.factories);
                    if (hov.ports > 0) bldText.push("⚓ x" + hov.ports);
                    if (hov.bunkers > 0) bldText.push("🛡️ x" + hov.bunkers);
                    hudRefs.hoverBlds.textContent = bldText.join(" ");
                }
            } else {
                hudRefs.hoverCard.classList.add("hidden");
            }
        }

        // Left Rail Slider
        if (hudRefs.slider && document.activeElement !== hudRefs.slider) {
            hudRefs.slider.value = Math.round(currentRatio * 100);
        }

        // Bottom Dock: Phase Transformation
        if (hudRefs.deployBtn) {
            hudRefs.deployBtn.style.display = isDeploying ? "flex" : "none";
            if (isDeploying && hudRefs.deployTimer) {
                hudRefs.deployTimer.textContent = spawnSecs.toFixed(1) + 's';
            }
        }
        if (hudRefs.dockTabsWrap) hudRefs.dockTabsWrap.style.display = "flex";
        if (hudRefs.bldStrip) {
            hudRefs.bldStrip.style.display = isDeploying ? "none" : "flex";
            var selBld = hud.selected_building;
            if (hudRefs.bldCity) hudRefs.bldCity.classList.toggle("active", selBld === "City");
            if (hudRefs.bldFactory) hudRefs.bldFactory.classList.toggle("active", selBld === "Factory");
            if (hudRefs.bldPort) hudRefs.bldPort.classList.toggle("active", selBld === "Port");
            if (hudRefs.bldBunker) hudRefs.bldBunker.classList.toggle("active", selBld === "Bunker");
            if (hudRefs.cancelPlacement) {
                hudRefs.cancelPlacement.classList.toggle("hidden", !selBld && !hud.selected_nuke);
            }
            if (hud.building_costs) {
                if (hudRefs.costCity) hudRefs.costCity.textContent = Math.floor(hud.building_costs.city) + 'g';
                if (hudRefs.costFactory) hudRefs.costFactory.textContent = Math.floor(hud.building_costs.factory) + 'g';
                if (hudRefs.costPort) hudRefs.costPort.textContent = Math.floor(hud.building_costs.port) + 'g';
                if (hudRefs.costBunker) hudRefs.costBunker.textContent = Math.floor(hud.building_costs.bunker) + 'g';
            }
        }

        // Emoji Popout
        if (hudRefs.emojiPopout) {
            hudRefs.emojiPopout.classList.toggle("hidden", !emojiPickerOpen);
        }
        if (hudRefs.utilities) hudRefs.utilities.classList.toggle("open", utilitiesOpen);
        if (hudRefs.utilityToggle) hudRefs.utilityToggle.setAttribute("aria-expanded", String(utilitiesOpen));
        if (hudRefs.pinEmoji) {
            pinEmoji = Boolean(hud.pin_emoji);
            hudRefs.pinEmoji.classList.toggle("active", pinEmoji);
            hudRefs.pinEmoji.setAttribute("aria-pressed", String(pinEmoji));
        }
        if (hudRefs.dockTabs) {
            hudRefs.dockTabs.forEach(function (tab) {
                tab.classList.toggle("active", tab.dataset.tab === bottomTab);
            });
        }
        if (hudRefs.inbox) {
            hudRefs.inbox.classList.toggle("hidden", !inboxOpen);
            if (inboxOpen) renderInbox(hud.inbox);
        }
        if (hudRefs.logPanel) {
            var showBattleLog = bottomTab === "battle_log";
            var showEventLog = bottomTab === "event_log";
            hudRefs.logPanel.classList.toggle("hidden", !showBattleLog && !showEventLog);
            if (hudRefs.logTitle) hudRefs.logTitle.textContent = showEventLog ? "EVENT LOG" : "BATTLE LOG";
            if (hudRefs.clearLog) hudRefs.clearLog.classList.toggle("hidden", !showEventLog);
            if (showBattleLog) renderBattleLog(hud.battle_log);
            if (showEventLog) renderEventLog(hud.event_log);
        }
        if (hudRefs.transfer) {
            transferOpen = Boolean(hud.transfer) || transferOpen;
            hudRefs.transfer.classList.toggle("hidden", !transferOpen || !hud.transfer);
            if (hud.transfer) {
                hudRefs.transfer.dataset.targetId = String(hud.transfer.target_id);
                if (hudRefs.transferTarget) hudRefs.transferTarget.textContent = "Target: " + (hud.transfer.target_name || "Player");
            }
        }
        if (hudRefs.betrayal) {
            betrayalOpen = Boolean(hud.betrayal);
            hudRefs.betrayal.classList.toggle("hidden", !betrayalOpen);
            if (betrayalOpen && hudRefs.betrayalCopy) {
                hudRefs.betrayalCopy.textContent = "Break alliance with " + (hud.betrayal.ally_name || "this player") + " and continue the attack?";
            }
        }

        renderNotifications(hud.notifications);

        // Leaderboard
        if (hudRefs.leaderboard) {
            hudRefs.leaderboard.classList.toggle("hidden", !leaderboardOpen);
        }
        if (leaderboardOpen) {
            updateLeaderboard(hud.leaderboard);
        }

        // Surrender Modal
        if (hudRefs.surrender) {
            hudRefs.surrender.classList.toggle("hidden", !surrenderModalOpen);
        }

        // Endgame Screen
        var isOver = Boolean(hud.match_over);
        var isWinner = Boolean(hud.is_winner);
        if (hudRefs.endgame) {
            hudRefs.endgame.classList.toggle("hidden", !isOver);
            if (isOver) {
                if (hudRefs.endgameBanner) hudRefs.endgameBanner.textContent = isWinner ? 'VICTORY' : 'DEFEAT';
                if (hudRefs.endgameTitle) hudRefs.endgameTitle.textContent = isWinner ? 'The World is Yours' : 'Your Empire Has Fallen';
                if (hudRefs.endgameDesc) hudRefs.endgameDesc.textContent = isWinner ? 'Your conquest is complete. All realms have bowed to your authority.' : (hud.winner_name ? hud.winner_name + ' has unified the world.' : 'Your armies have been vanquished in glorious battle.');
                if (hudRefs.endgameStats) {
                    var kda = hud.player_kda || {};
                    var rewards = hud.rewards || {};
                    hudRefs.endgameStats.textContent = 'K/D/A ' + [kda.kills || 0, kda.deaths || 0, kda.assists || 0].join(' / ') +
                        '   ·   +' + (rewards.xp || 0) + ' XP   ·   +' + (rewards.leader_xp || 0) + ' LEADER XP   ·   +' + (rewards.laurels || 0) + ' LAURELS';
                }
            }
        }
        hudRoot.dataset.overlayOpen = String(Boolean(
            leaderboardOpen || inboxOpen || transferOpen || betrayalOpen ||
            surrenderModalOpen || emojiPickerOpen || bottomTab !== "controls" || isOver
        ));
    }

    if (hudRoot) {
        hudRoot.addEventListener("click", function (event) {
            var btn = event.target.closest("[data-command]");
            if (!btn) return;
            var cmd = btn.dataset.command;
            if (cmd === "toggle_utilities") {
                utilitiesOpen = !utilitiesOpen;
                renderHud();
            } else if (cmd === "toggle_leaderboard") {
                leaderboardOpen = !leaderboardOpen;
                utilitiesOpen = false;
                send("toggle_leaderboard");
                renderHud();
            } else if (cmd === "toggle_inbox") {
                inboxOpen = !inboxOpen;
                utilitiesOpen = false;
                send("toggle_inbox");
                renderHud();
            } else if (cmd === "toggle_pin_emoji") {
                pinEmoji = !pinEmoji;
                send("set_emoji_pinned", { pinned: pinEmoji });
                renderHud();
            } else if (cmd === "set_bottom_tab") {
                utilitiesOpen = false;
                send("set_bottom_tab", { tab: btn.dataset.tab || "controls" });
                renderHud();
            } else if (cmd === "open_transfer") {
                if (leaderboardOpen) {
                    leaderboardOpen = false;
                    send("toggle_leaderboard");
                }
                transferOpen = true;
                send("open_transfer", { target_player_id: Number(btn.dataset.playerId) });
                renderHud();
            } else if (cmd === "close_transfer") {
                transferOpen = false;
                send("close_transfer");
                renderHud();
            } else if (cmd === "send_resources" || cmd === "request_resources") {
                var transfer = hudRefs && hudRefs.transfer;
                var targetId = transfer ? Number(transfer.dataset.targetId) : 0;
                var gold = hudRefs && hudRefs.transferGold ? Number(hudRefs.transferGold.value) : 0;
                var transferTroops = hudRefs && hudRefs.transferTroops ? Number(hudRefs.transferTroops.value) : 0;
                if (targetId > 0) send(cmd, { target_player_id: targetId, gold: gold, troops: transferTroops });
                transferOpen = false;
                renderHud();
            } else if (cmd === "accept_alliance" || cmd === "reject_alliance" || cmd === "accept_resource_request" || cmd === "reject_resource_request") {
                var requesterId = Number(btn.dataset.playerId);
                if (requesterId > 0) send(cmd, { target_player_id: requesterId });
            } else if (cmd === "clear_event_log") {
                send("clear_event_log");
            } else if (cmd === "cancel_betrayal") {
                betrayalOpen = false;
                send("cancel_betrayal");
                renderHud();
            } else if (cmd === "confirm_betrayal") {
                betrayalOpen = false;
                send("confirm_betrayal");
                renderHud();
            } else if (cmd === "cancel_attack" || cmd === "recall_fleet") {
                var id = Number(cmd === "cancel_attack" ? btn.dataset.attackId : btn.dataset.fleetId);
                if (id > 0) send(cmd, cmd === "cancel_attack" ? { attack_id: id } : { fleet_id: id });
            } else if (cmd === "prompt_surrender") {
                utilitiesOpen = false;
                surrenderModalOpen = true;
                renderHud();
            } else if (cmd === "close_surrender_modal") {
                surrenderModalOpen = false;
                renderHud();
            } else if (cmd === "confirm_surrender") {
                surrenderModalOpen = false;
                send("leave_lobby");
                renderHud();
            } else if (cmd === "toggle_emoji") {
                utilitiesOpen = false;
                emojiPickerOpen = !emojiPickerOpen;
                renderHud();
            } else if (cmd === "express_emoji") {
                var emoji = btn.dataset.emoji || "😀";
                send("express_emoji", { emoji: emoji, pinned: pinEmoji });
                emojiPickerOpen = false;
                renderHud();
            } else if (cmd === "zoom_in") {
                send("zoom_in");
            } else if (cmd === "zoom_out") {
                send("zoom_out");
            } else if (cmd === "center_camera") {
                send("center_camera");
            } else if (cmd === "cancel_placement") {
                send("cancel_placement");
            } else if (cmd === "focus_player") {
                var pid = parseInt(btn.dataset.playerId, 10);
                if (!isNaN(pid)) {
                    send("focus_player", { player_id: pid });
                }
            } else if (cmd === "spawn_troops") {
                send("spawn_troops");
            } else if (cmd === "build_structure") {
                var kind = btn.dataset.kind || "City";
                send("build_structure", { kind: kind });
            } else if (cmd === "confirm_endgame_leave") {
                send("leave_lobby");
            }
        });
    }

    function handleStateUpdate(raw) {
        if (typeof raw !== "string" || raw === lastRaw) {
            updateDynamic();
            return;
        }
        lastRaw = raw;
        var previousHud = state && state.phase === "Playing" ? state.hud : null;
        try {
            state = JSON.parse(raw);
        } catch (error) {
            console.warn("[WEB MENU] invalid state:", error);
            return;
        }
        if (state.phase === "Playing") {
            state.hud = state.hud || previousHud;
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
        renderHud();
    }

    window.SOW_onStateUpdate = handleStateUpdate;

    function poll() {
        // Rust calls SOW_onStateUpdate for every changed payload. The fallback poll is
        // only useful for menu countdown text; it must not run during a match.
        if (state && state.phase === "Playing") return;
        var raw = window.SOW_MENU_STATE;
        handleStateUpdate(raw);
    }

    root.hidden = true;
    if (hudRoot) hudRoot.hidden = true;
    pollTimer = window.setInterval(poll, 80);
})();
