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
        return asset("cdn/leaders/" + leader.slug + "_desktop.webp");
    }

    function avatarImage() {
        var leader = leaderById(state && state.selected_leader);
        return asset("cdn/avatars/" + leader.slug + ".webp");
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
            countdown: lobby.is_counting_down,
            timer: Math.ceil(lobby.timer_secs || 0),
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
            private_game: state.custom_game_is_private,
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
                "<div class='sow-menu__brand'>SHADOWS OF WAR <small>MMORTS</small></div>" +
                "<div class='sow-menu__identity'>" +
                    "<button class='sow-menu__avatar' type='button' data-command='open_leader_picker' " +
                        "aria-label='Select leader' style=\"background-image:url('" + esc(avatarImage()) + "')\"></button>" +
                    "<div class='sow-menu__profile'>" +
                        "<input data-role='display-name' value=\"" + esc(name) + "\" maxlength='20' " +
                            (state.name_locked ? "readonly" : "") + " aria-label='Display name'>" +
                        "<small>" + esc(leader.name) + " · " + esc(leader.civilization) + "</small>" +
                    "</div>" +
                    "<div class='sow-menu__progress' data-progression>LV " + esc(state.level) +
                        " · " + esc(state.xp) + " XP · ✦ " + esc(state.laurels) + "</div>" +
                    "<div class='sow-menu__top-actions'>" +
                        "<button class='sow-menu__signin' type='button' data-command='sign_in'>" + signIn + "</button>" +
                        "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='Settings'>⚙</button>" +
                    "</div>" +
                "</div>" +
            "</header>";
    }

    function renderCommandPanel() {
        var error = state.error ? "<div class='sow-menu__status sow-menu__status--error'>" + esc(state.error) + "</div>" : "";
        var notice = state.notice ? "<div class='sow-menu__status sow-menu__status--notice'>" +
            esc({ host_left: "Host left the lobby", kicked: "You were removed from the lobby", banned: "You are banned from this lobby", connection_lost: "Connection lost" }[state.notice] || state.notice) +
            "</div>" : "";
        return "" +
            "<section class='sow-menu__command'>" +
                "<p class='sow-menu__eyebrow'>COMMAND CENTER</p>" +
                "<h1>SHADOWS<br><em>OF WAR</em></h1>" +
                "<p class='sow-menu__tagline'>A browser MMORTS of territory, diplomacy, and decisive betrayal. Pick a leader, enter a live map, and make the world remember your name.</p>" +
                "<button class='sow-menu__primary' type='button' data-command='quick_match'>ENTER THE WAR <span>↗</span></button>" +
                "<button class='sow-menu__secondary' type='button' data-command='open_browser'>BROWSE PUBLIC GAMES <span>→</span></button>" +
                "<form class='sow-menu__join' data-form='join'>" +
                    "<input name='code' inputmode='numeric' autocomplete='off' placeholder='LOBBY CODE' aria-label='Lobby code'>" +
                    "<button type='submit'>JOIN</button>" +
                "</form>" +
                "<button class='sow-menu__secondary' type='button' data-command='open_create'>CREATE A GAME <span>+</span></button>" +
                "<div class='sow-menu__status' data-connection data-connected='false'>CONNECTING TO WAR ROOM</div>" +
                error + notice +
            "</section>";
    }

    function publicLobbies() {
        return (state.lobbies || []).filter(function (lobby) {
            if (lobby.kind !== "Matchmaking" && lobby.kind !== "Custom") return false;
            if (lobby.kind === "Custom" && lobby.has_password) return false;
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

    function renderLobbyCard(lobby) {
        var count = lobby.max_players ? lobby.num_players + "/" + lobby.max_players : lobby.num_players + " PLAYERS";
        var timer = lobby.is_counting_down ? "STARTING " + Math.ceil(lobby.timer_secs) + "s" : count;
        return "" +
            "<article class='sow-menu__lobby' data-command='join_lobby' data-lobby-id='" + lobby.id +
                "' style=\"background-image:url('" + esc(lobbyThumb(lobby)) + "')\">" +
                "<div class='sow-menu__lobby-top'><span>" + esc(lobby.game_mode || "FFA") + "</span><span>" + esc(timer) + "</span></div>" +
                "<h3>" + esc(lobby.map_name || "WORLD MAP") + "</h3>" +
                "<div class='sow-menu__lobby-bottom'><span>" + esc(lobby.host_name || "OPEN LOBBY") + "</span><span>JOIN ↗</span></div>" +
            "</article>";
    }

    function renderPublicPanel() {
        var lobbies = publicLobbies();
        var cards = lobbies.map(renderLobbyCard).join("");
        if (!cards) {
            cards = "<div class='sow-menu__empty'>No public games yet.<br>Start a game and become the first commander on the map.</div>";
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
                        "<div class='sow-menu__leader-copy'><small>COMMANDING " + esc(leader.civilization) + "</small><h2>" + esc(leader.name) +
                            "</h2><p>" + esc(leader.perk) + "</p></div>" +
                        renderPublicPanel() +
                    "</section>" +
                "</main>" +
                renderFooter("PLAY IN BROWSER · NO DOWNLOAD") +
            "</div>" + leaderOverlay + settingsOverlay;
    }

    function renderBrowser() {
        var lobbies = publicLobbies();
        var cards = lobbies.map(renderLobbyCard).join("");
        if (!cards) cards = "<div class='sow-menu__empty'>No public games match this filter.</div>";
        return "" +
            "<div class='sow-menu__backdrop'></div><div class='sow-menu__shell'>" +
                renderTopbar() +
                "<main class='sow-menu__main'>" +
                    "<section class='sow-menu__command'><p class='sow-menu__eyebrow'>GAME BROWSER</p><h1>CHOOSE<br><em>YOUR WAR</em></h1>" +
                        "<p class='sow-menu__tagline'>Join an open lobby already gathering commanders, or create a map with your own rules.</p>" +
                        "<button class='sow-menu__secondary' type='button' data-command='close_overlay'>← BACK TO COMMAND</button>" +
                        "<form class='sow-menu__join' data-form='join'><input name='code' inputmode='numeric' autocomplete='off' placeholder='LOBBY CODE' aria-label='Lobby code'><button type='submit'>JOIN</button></form>" +
                    "</section>" +
                    "<section class='sow-menu__battlefield'><section class='sow-menu__public'><div class='sow-menu__public-head'><p class='sow-menu__panel-label'>PUBLIC GAMES</p><div class='sow-menu__filters'>" +
                        filterButton("all", "ALL") + filterButton("ffa", "FFA") + filterButton("teams", "TEAMS") + filterButton("hvn", "HVN") +
                    "</div></div><div class='sow-menu__lobbies'>" + cards + "</div></section></section>" +
                "</main>" + renderFooter("LIVE LOBBIES") +
            "</div>";
    }

    function cloneConfig() {
        return JSON.parse(JSON.stringify(state.custom_game_config || {}));
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
        var privateGame = state.custom_game_is_private === true;
        return "" +
            "<div class='sow-menu__backdrop'></div><div class='sow-menu__shell'>" + renderTopbar() +
                "<main class='sow-menu__main'><section class='sow-menu__command'><p class='sow-menu__eyebrow'>LOBBY FORGE</p><h1>SET THE<br><em>TERMS</em></h1><p class='sow-menu__tagline'>Shape the battlefield, choose who enters, and decide when the war begins.</p><button class='sow-menu__secondary' type='button' data-command='close_overlay'>← CANCEL</button></section>" +
                    "<section class='sow-menu__battlefield'><form class='sow-menu__modal' data-form='create'><div class='sow-menu__modal-head'><div><p class='sow-menu__panel-label'>CREATE GAME</p><h2>YOUR RULES</h2></div></div>" +
                        "<div class='sow-menu__form-grid'>" +
                            "<label class='sow-menu__form-field sow-menu__form-field--wide'>MAP<select class='sow-menu__select' name='map_name'>" + mapOptions(config) + "</select></label>" +
                            "<label class='sow-menu__form-field'>MODE<select class='sow-menu__select' name='game_mode'>" + modeOptions(config) + "</select></label>" +
                            "<label class='sow-menu__form-field'>MAX PLAYERS<input class='sow-menu__field' name='max_players' type='number' min='2' max='16' value='" + esc(config.max_players || 8) + "'></label>" +
                            "<label class='sow-menu__form-field'>TRIBES<input class='sow-menu__field' name='bot_count' type='number' min='0' max='1000' value='" + esc(config.bot_count || 128) + "'></label>" +
                            "<label class='sow-menu__form-field'>NATIONS<input class='sow-menu__field' name='nation_count' type='number' min='0' max='400' value='" + esc(config.nation_count || 32) + "'></label>" +
                            "<label class='sow-menu__form-field'>DIFFICULTY<select class='sow-menu__select' name='bot_difficulty'><option value='Vanilla' " + (config.bot_difficulty === "Vanilla" ? "selected" : "") + ">VANILLA</option><option value='Terminator' " + (config.bot_difficulty === "Terminator" ? "selected" : "") + ">TERMINATOR</option></select></label>" +
                            "<label class='sow-menu__form-field sow-menu__form-field--wide'>VISIBILITY<select class='sow-menu__select' name='visibility'><option value='public' " + (!privateGame ? "selected" : "") + ">PUBLIC</option><option value='private' " + (privateGame ? "selected" : "") + ">PRIVATE</option></select></label>" +
                            "<label class='sow-menu__form-field sow-menu__form-field--wide'>PASSWORD (OPTIONAL)<input class='sow-menu__field' name='password' type='password' autocomplete='new-password' placeholder='Leave empty for public access'></label>" +
                        "</div><div class='sow-menu__modal-actions'><button class='sow-menu__ghost-button' type='button' data-command='close_overlay'>CANCEL</button><button class='sow-menu__primary' type='submit'>CREATE LOBBY <span>↗</span></button></div></form></section>" +
                "</main>" + renderFooter("CONFIGURATION IS SERVER-VALIDATED") + "</div>";
    }

    function joinedLobby() {
        var id = state.joined_lobby_id || state.pending_lobby_id;
        return (state.lobbies || []).find(function (lobby) { return lobby.id === id; }) || null;
    }

    function renderQueue() {
        var lobby = joinedLobby();
        var title = lobby ? (lobby.map_name || "WORLD MAP") : "FINDING A BATTLE";
        var feedback = state.is_downloading_map ?
            "<div class='sow-menu__queue-feedback'>DOWNLOADING " + esc(state.downloading_map_name || title) + " · " + esc(state.map_download_progress || 0) + "%</div>" :
            (state.error ? "<div class='sow-menu__queue-feedback sow-menu__queue-feedback--error'>" + esc(state.error) + "</div>" : "");
        var roster = lobby && lobby.players ? lobby.players.map(function (player) {
            return "<div class='sow-menu__player'><span>" + esc(player.name) + "</span><small>" + esc(player.team || "COMMANDER") + "</small></div>";
        }).join("") : "<div class='sow-menu__empty'>Waiting for the war room to answer.</div>";
        var hostAction = lobby && state.is_lobby_host && lobby.kind === "Custom" ?
            "<button class='sow-menu__primary' type='button' data-command='start_private' data-lobby-id='" + lobby.id + "'>START GAME <span>↗</span></button>" : "";
        return "" +
            "<div class='sow-menu__backdrop'></div><div class='sow-menu__shell'>" + renderTopbar() +
                "<main class='sow-menu__main'><section class='sow-menu__command'><p class='sow-menu__eyebrow'>WAR ROOM</p><h1>HOLD<br><em>THE LINE</em></h1><p class='sow-menu__tagline'>Your command is registered. The map is assembling its rivals.</p><div class='sow-menu__status' data-connection data-connected='" + state.connected + "'>" + (state.connected ? "LIVE CONNECTION" : "RECONNECTING") + "</div><button class='sow-menu__danger' type='button' data-command='leave_lobby'>LEAVE LOBBY <span>×</span></button></section>" +
                    "<section class='sow-menu__battlefield'><section class='sow-menu__queue'><div class='sow-menu__queue-head'><div><p class='sow-menu__panel-label'>" + esc(lobby ? (lobby.game_mode || "MATCHMAKING") : "MATCHMAKING") + "</p><h2>" + esc(title) + "</h2></div><span data-live-countdown></span></div><div class='sow-menu__queue-status' data-queue-status>ASSEMBLING COMMANDERS<strong>" + (lobby ? esc((lobby.num_players || 0) + "/" + (lobby.max_players || "?")) : "—") + "</strong></div>" + feedback + "<div class='sow-menu__roster'>" + roster + "</div><div class='sow-menu__modal-actions'>" + hostAction + "</div></section></section>" +
                "</main>" + renderFooter("LOBBY STATE IS LIVE") + "</div>";
    }

    function renderLeaderPicker() {
        var leaders = state.leaders || [];
        return "<div class='sow-menu__overlay'><section class='sow-menu__modal'><div class='sow-menu__modal-head'><div><p class='sow-menu__panel-label'>COMMANDERS</p><h2>CHOOSE YOUR LEADER</h2></div><button class='sow-menu__icon-button' type='button' data-command='close_leader_picker'>×</button></div><div class='sow-menu__leader-list'>" +
            leaders.map(function (leader) {
                return "<button class='sow-menu__leader-option' type='button' data-command='set_leader' data-leader-id='" + esc(leader.id) + "' data-selected='" + (leader.id === state.selected_leader) + "'><img src='" + esc(asset("cdn/avatars/" + leader.slug + ".webp")) + "' alt=''><span>" + esc(leader.name) + "</span></button>";
            }).join("") + "</div></section></div>";
    }

    function renderSettings() {
        var settings = state.settings || {};
        return "<div class='sow-menu__overlay'><section class='sow-menu__modal'><div class='sow-menu__modal-head'><div><p class='sow-menu__panel-label'>SYSTEM</p><h2>SETTINGS</h2></div><button class='sow-menu__icon-button' type='button' data-command='toggle_settings'>×</button></div><div class='sow-menu__form-grid'><label class='sow-menu__form-field sow-menu__form-field--wide'>MASTER AUDIO<select class='sow-menu__select' data-setting='mute'><option value='on' " + (!settings.mute_all ? "selected" : "") + ">ON</option><option value='off' " + (settings.mute_all ? "selected" : "") + ">OFF</option></select></label><label class='sow-menu__form-field sow-menu__form-field--wide'>MUSIC VOLUME<input class='sow-menu__field' type='range' min='0' max='1' step='0.05' value='" + esc(settings.music_volume == null ? 0.8 : settings.music_volume) + "' data-setting='music_volume'></label><label class='sow-menu__form-field sow-menu__form-field--wide'>MOTION<select class='sow-menu__select' data-setting='reduced_motion'><option value='full' " + (!settings.reduced_motion ? "selected" : "") + ">FULL</option><option value='reduced' " + (settings.reduced_motion ? "selected" : "") + ">REDUCED</option></select></label></div><div class='sow-menu__modal-actions'><button class='sow-menu__primary' type='button' data-command='toggle_settings'>DONE <span>✓</span></button></div></section></div>";
    }

    function render() {
        if (!state) return;
        var screen = currentScreen();
        if (screen === "create" && previousScreen !== "create") createDraft = cloneConfig();
        if (screen !== "create") createDraft = null;
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
            connection.textContent = state.connected ? "LIVE CONNECTION" : (state.connecting ? "CONNECTING TO WAR ROOM" : "OFFLINE · RETRYING");
        }
        var progression = root.querySelector("[data-progression]");
        if (progression) progression.textContent = "LV " + state.level + " · " + state.xp + " XP · ✦ " + state.laurels;
        var timer = root.querySelector("[data-live-countdown]");
        var lobby = joinedLobby();
        if (timer && lobby) timer.textContent = lobby.is_counting_down ? "STARTING IN " + Math.ceil(lobby.timer_secs) + "s" : "WAITING FOR COMMANDERS";
        var queueStatus = root.querySelector("[data-queue-status]");
        if (queueStatus && lobby) {
            var strong = queueStatus.querySelector("strong");
            if (strong) strong.textContent = (lobby.num_players || 0) + "/" + (lobby.max_players || "?");
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
        if (command === "set_leader") {
            if (send("set_leader", { leader_id: target.dataset.leaderId })) {
                leaderPickerOpen = false;
            }
            return;
        }
        var payload = {};
        if (target.dataset.lobbyId) payload.lobby_id = Number(target.dataset.lobbyId);
        send(command, payload);
    });

    root.addEventListener("submit", function (event) {
        var form = event.target;
        if (form.dataset.form === "join") {
            event.preventDefault();
            var code = form.elements.code.value.trim();
            if (code) send("join_code", { code: code });
        }
        if (form.dataset.form === "create") {
            event.preventDefault();
            var config = createDraft || cloneConfig();
            Array.prototype.forEach.call(form.elements, function (field) {
                if (!field.name) return;
                if (field.name === "max_players" || field.name === "bot_count" || field.name === "nation_count") config[field.name] = Number(field.value);
                else if (field.name === "map_name") {
                    config.map_name = field.value;
                    var map = (state.map_catalog || []).find(function (entry) { return entry.key === field.value; });
                    if (map) {
                        config.map_width = map.width;
                        config.map_height = map.height;
                    }
                }
                else if (field.name !== "visibility" && field.name !== "password") config[field.name] = field.value;
            });
            var privateGame = form.elements.visibility.value === "private";
            send("create_game", { config: config, is_private: privateGame, password: form.elements.password.value || null });
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
        if (!input.dataset.setting) return;
        if (input.dataset.setting === "mute") send("set_mute", { value: input.value === "off" });
        if (input.dataset.setting === "music_volume") send("set_music_volume", { value: Number(input.value) });
        if (input.dataset.setting === "reduced_motion") send("set_reduced_motion", { value: input.value === "reduced" });
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
