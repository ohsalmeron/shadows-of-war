(function () {
    "use strict";

    var root = document.getElementById("sow-menu");
    if (!root) return;

    var state = null;
    var lastRaw = "";
    var lastRenderKey = "";
    var previousScreen = null;
    var tempSelectedLeader = null;
    var browserSearchQuery = "";
    var heroesSearchQuery = "";
    var heroesRegionFilter = "all";
    var settingsOpen = false;
    var profileOpen = false;
    var profilePublicId = null;
    var profileTab = "overview";
    var profileData = null;
    var profileHistory = [];
    var profileHistoryCursor = 0;
    var profileRatings = null;
    var profileMatchDetail = null;
    var profileSearchResults = [];
    var profileLoading = false;
    var profileError = "";
    var mobileStoreOpen = false;
    var mobileHeroesOpen = false;
    var createDraft = null;
    var createOffline = false;
    var createPrivate = false;
    var createPassword = "";
    var passwordLobbyId = null;
    var passwordDraft = "";
    var pendingCommands = [];
    var pendingHud = null;
    var lastHudRaw = "";

    var LEADER_REGIONS = {
        caesar: "Europe",
        cleopatra: "Africa",
        ragnar: "Europe",
        sun_tzu: "Asia",
        alexander: "Europe",
        genghis_khan: "Asia",
        richard_the_lionheart: "Europe",
        vercingetorix: "Europe",
        boudica: "Europe",
        lady_six_sky: "Americas",
        leonidas: "Europe",
        napoleon: "Europe"
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
        if (profileOpen) return "profile";
        if (mobileHeroesOpen) return "heroes";
        if (mobileStoreOpen) return "store";
        if (state.waiting) return "queue";
        if (state.show_create) return "create";
        if (state.show_browser) return "browser";
        return "home";
    }

    function profileApi(path) {
        var base = String(window.SOW_DATABASE_URL || "/api").replace(/\/$/, "");
        return base + path;
    }

    function loadProfile(id) {
        if (!id || profileLoading) return;
        profileLoading = true;
        profileError = "";
        fetch(profileApi("/profiles/" + encodeURIComponent(id)), {
            headers: { "Accept": "application/json" }
        }).then(function (response) {
            if (!response.ok) throw new Error("profile request failed");
            return response.json();
        }).then(function (data) {
            if (!profileOpen || profilePublicId !== id) return;
            profileData = data;
            profileHistory = Array.isArray(data.recent_matches) ? data.recent_matches.slice() : [];
            profileHistoryCursor = profileHistory.length;
        }).catch(function () {
            if (profileOpen && profilePublicId === id) {
                profileError = "Profile unavailable.";
            }
        }).finally(function () {
            profileLoading = false;
            if (profileOpen && profilePublicId === id) render();
        });
    }

    function loadMoreProfileHistory() {
        if (!profilePublicId || profileLoading) return;
        profileLoading = true;
        fetch(profileApi("/profiles/" + encodeURIComponent(profilePublicId) + "/matches?cursor=" + profileHistoryCursor + "&limit=20"), {
            headers: { "Accept": "application/json" }
        }).then(function (response) {
            if (!response.ok) throw new Error("history request failed");
            return response.json();
        }).then(function (data) {
            var items = Array.isArray(data.items) ? data.items : [];
            profileHistory = profileHistory.concat(items);
            profileHistoryCursor = Number(data.next_cursor || profileHistoryCursor + items.length);
        }).catch(function () {
            profileError = "Match history unavailable.";
        }).finally(function () {
            profileLoading = false;
            if (profileOpen) render();
        });
    }

    function loadProfileRatings() {
        if (!profilePublicId || profileLoading || profileRatings !== null) return;
        profileLoading = true;
        fetch(profileApi("/profiles/" + encodeURIComponent(profilePublicId) + "/seasons"), {
            headers: { "Accept": "application/json" }
        }).then(function (response) {
            if (!response.ok) throw new Error("profile ratings failed");
            return response.json();
        }).then(function (data) {
            if (profileOpen) profileRatings = Array.isArray(data.items) ? data.items : [];
        }).catch(function () {
            if (profileOpen) profileError = "Ranked records unavailable.";
        }).finally(function () {
            profileLoading = false;
            if (profileOpen) render();
        });
    }

    function openProfile(id) {
        var targetId = id || (state && state.public_profile_id);
        if (!targetId) return;
        reportOpen = false;
        reportTarget = null;
        reportSent = false;
        deleteArmed = false;
        try {
            if (typeof window.SOW_isBlockedId === "function" && window.SOW_isBlockedId(targetId)) {
                profileOpen = true;
                profilePublicId = targetId;
                profileTab = "overview";
                profileData = null;
                profileHistory = [];
                profileHistoryCursor = 0;
                profileRatings = null;
                profileMatchDetail = null;
                profileSearchResults = [];
                profileError = "You blocked this player.";
                render();
                return;
            }
        } catch (e) {}
        profileOpen = true;
        profilePublicId = targetId;
        profileTab = "overview";
        profileData = null;
        profileHistory = [];
        profileHistoryCursor = 0;
        profileRatings = null;
        profileMatchDetail = null;
        profileSearchResults = [];
        profileError = "";
        render();
        loadProfile(targetId);
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
        var bust = String(window.SOW_MAPS_CACHE_BUST || window.SOW_BUILD_TS || "");
        return base + "/" + encodeURIComponent((lobby && lobby.map_name) || "world") + "/thumbnail.webp" +
            (bust ? "?v=" + encodeURIComponent(bust) : "");
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
            name: state.player_name,
            locked: state.name_locked,
            leader: state.selected_leader,
            level: state.level,
            xp: state.xp,
            laurels: state.laurels,
            gems: state.gems,
            selected_skin: state.selected_skin,
            skins: (state.store && state.store.skins || []).map(function (skin) {
                return [skin.id, skin.owned, skin.cost_gems];
            }),
            public_profile_id: state.public_profile_id,
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
