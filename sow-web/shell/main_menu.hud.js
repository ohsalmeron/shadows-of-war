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
    var devSidebarOpen = false;
    var settingsOpen = false;

    var hudInitialized = false;
    var hudRefs = null;
    var leaderboardRows = Object.create(null);
    var leaderboardRenderKey = "";
    var inboxRenderKey = "";

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
            + '    <button class="sow-hud__icon-pill" type="button" data-command="toggle_tutorial_objectives" id="sow-hud-quests-btn" aria-label="Quests" title="Quests">📜</button>'
            + '    <button class="sow-hud__icon-pill" type="button" data-command="toggle_leaderboard" aria-label="Rankings" title="Rankings">🏆</button>'
            + '    <button class="sow-hud__icon-pill hidden" type="button" data-command="toggle_dev_sidebar" id="sow-hud-dev-btn" aria-label="Dev Tools" title="Dev Tools">🛠</button>'
            + '  </div>'
            + '  <div class="sow-hud__status-right">'
            + '    <span class="sow-hud__fps" id="sow-hud-fps">60 FPS</span>'
            + '    <button class="sow-hud__icon-pill sow-hud__inbox-pill" type="button" data-command="toggle_inbox" aria-label="Inbox" title="Inbox">📩 <span class="sow-hud__inbox-badge" id="sow-hud-inbox-count">0</span></button>'
            + '    <button class="sow-hud__icon-pill" type="button" data-command="toggle_settings" aria-label="Settings" title="Settings">⚙</button>'
            + '    <button class="sow-hud__icon-pill sow-hud__exit-pill" type="button" data-command="prompt_surrender" aria-label="Leave Match" title="Leave Match">✕</button>'
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
            + '  <button type="button" class="sow-hud__icon-btn" data-command="zoom_in" aria-label="Zoom in" title="Zoom in">➕</button>'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="zoom_out" aria-label="Zoom out" title="Zoom out">➖</button>'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="center_camera" aria-label="Center camera" title="Center camera">🏠</button>'
            + '  <button type="button" class="sow-hud__icon-btn" data-command="toggle_emoji" aria-label="Emojis" title="Emojis">😀</button>'
            + '</aside>'
            + '<div class="sow-hud__emoji-popout hidden" id="sow-hud-emoji-popout">'
            + '  <div class="sow-hud__emoji-header">'
            + '    <span>EXPRESS REACTION</span>'
            + '    <button type="button" class="sow-hud__pin-btn" data-command="toggle_pin_emoji" aria-pressed="false">PIN</button>'
            + '    <button type="button" class="sow-hud__close-btn" data-command="toggle_emoji">✕</button>'
            + '  </div>'
            + '  <div class="sow-hud__emoji-grid" id="sow-hud-emoji-grid"></div>'
            + '</div>'
            + '<aside class="sow-hud__dev-sidebar hidden" id="sow-hud-dev-sidebar">'
            + '  <div class="sow-hud__dev-header"><b>Dev Tools</b><button type="button" class="sow-hud__close-btn" data-command="toggle_dev_sidebar">✕</button></div>'
            + '  <div class="sow-hud__dev-body">'
            + '    <div class="sow-hud__dev-section"><b>Map &amp; Borders</b>'
            + '      <label class="sow-hud__dev-row">Border Thk <input type="range" min="0" max="1" step="0.01" value="0.5" data-dev="thickness"></label>'
            + '      <label class="sow-hud__dev-row">Border Drk <input type="range" min="0" max="1" step="0.01" value="0.5" data-dev="darkness"></label>'
            + '      <label class="sow-hud__dev-row">Shore Thk  <input type="range" min="0" max="1" step="0.01" value="0.5" data-dev="shore_thickness"></label>'
            + '      <label class="sow-hud__dev-row">Conquest Dur <input type="range" min="0.1" max="10" step="0.1" value="1.5" data-dev="conquest_duration"></label>'
            + '      <label class="sow-hud__dev-row">Opacity <input type="range" min="0" max="1" step="0.01" value="1" data-dev="territory_opacity"></label>'
            + '      <button type="button" class="sow-hud__dev-reset" data-command="reset_dev_config">RESET</button>'
            + '    </div>'
            + '  </div>'
            + '</aside>'
            + '<aside class="sow-hud__settings hidden" id="sow-hud-settings">'
            + '  <div class="sow-hud__panel-header"><h3>SETTINGS</h3><button class="sow-hud__close-btn" type="button" data-command="toggle_settings" aria-label="Close settings">✕</button></div>'
            + '  <label class="sow-hud__setting-row"><span>Sound</span><input type="checkbox" data-hud-setting="mute_all"></label>'
            + '  <label class="sow-hud__setting-row"><span>Music</span><input type="range" min="0" max="1" step="0.05" data-hud-setting="music_volume"></label>'
            + '  <label class="sow-hud__setting-row"><span>Reduced motion</span><input type="checkbox" data-hud-setting="reduced_motion"></label>'
            + '</aside>'
            + '<footer class="sow-hud__dock" id="sow-hud-dock">'
            + '  <div class="sow-hud__dock-resources">'
            + '    <div class="sow-hud__resource sow-hud__gold" title="Gold Treasury"><span class="sow-hud__icon">🪙</span> <b data-role="gold">0</b></div>'
            + '    <div class="sow-hud__resource sow-hud__troops" title="Troop Pool"><span class="sow-hud__icon">🌾</span> <b data-role="troops">0</b></div>'
            + '    <div class="sow-hud__resource sow-hud__prod" title="Production"><span class="sow-hud__icon">⚙️</span> <b data-role="prod">+0/s</b></div>'
            + '  </div>'
            + '  <div class="sow-hud__dock-inner">'
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
            + '    <h3>RANKINGS</h3>'
            + '    <button class="sow-hud__close-btn" type="button" data-command="toggle_leaderboard">✕</button>'
            + '  </div>'
            + '  <div class="sow-hud__leaderboard-rows" id="sow-hud-lb-rows"></div>'
            + '</aside>'
            + '<aside class="sow-hud__panel sow-hud__inbox hidden" id="sow-hud-inbox">'
            + '  <div class="sow-hud__panel-header"><h3>INBOX</h3><button class="sow-hud__close-btn" type="button" data-command="toggle_inbox">✕</button></div>'
            + '  <div class="sow-hud__panel-rows" id="sow-hud-inbox-rows"></div>'
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
            + '  <section class="sow-hud__endgame-card" role="dialog" aria-modal="true" aria-labelledby="sow-hud-endgame-title">'
            + '    <div class="sow-hud__endgame-hero">'
            + '      <div class="sow-hud__endgame-hero-copy">'
            + '        <div class="sow-hud__endgame-kicker"><span class="sow-hud__endgame-icon" id="sow-hud-endgame-icon" aria-hidden="true">⚔</span><span>MATCH RESULT</span></div>'
            + '        <h2 class="sow-hud__endgame-banner" id="sow-hud-endgame-banner">DEFEAT</h2>'
            + '        <h3 class="sow-hud__endgame-title" id="sow-hud-endgame-title">MATCH LOST</h3>'
            + '        <p class="sow-hud__endgame-desc" id="sow-hud-endgame-desc">The match has ended.</p>'
            + '      </div>'
            + '      <div class="sow-hud__endgame-portrait-wrap">'
            + '        <img class="sow-hud__endgame-portrait" id="sow-hud-endgame-portrait" src="" alt="Leader Portrait" />'
            + '      </div>'
            + '    </div>'
            + '    <div class="sow-hud__endgame-stats" id="sow-hud-endgame-stats">'
            + '      <div class="sow-hud__endgame-stat"><span class="sow-hud__endgame-stat-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M6 4l14 14M18 4L4 18M5 5l3 3M19 5l-3 3M5 19l3-3M19 19l-3-3"/></svg></span><span class="sow-hud__endgame-stat-label">K / D / A</span><b id="sow-hud-endgame-kda">0 / 0 / 0</b></div>'
            + '      <div class="sow-hud__endgame-stat"><span class="sow-hud__endgame-stat-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m12 3 2.1 5.1L19 10l-4.9 1.9L12 17l-2.1-5.1L5 10l4.9-1.9z"/></svg></span><span class="sow-hud__endgame-stat-label">XP</span><b id="sow-hud-endgame-xp">+0</b></div>'
            + '      <div class="sow-hud__endgame-stat"><span class="sow-hud__endgame-stat-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m4 7 4 4 4-6 4 6 4-4-2 10H6zM6 20h12"/></svg></span><span class="sow-hud__endgame-stat-label">LEADER XP</span><b id="sow-hud-endgame-leader-xp">+0</b></div>'
            + '      <div class="sow-hud__endgame-stat"><span class="sow-hud__endgame-stat-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M8 20c-3-2-5-5-5-9 3 1 5 3 6 6M16 20c3-2 5-5 5-9-3 1-5 3-6 6M9 22h6"/></svg></span><span class="sow-hud__endgame-stat-label">LAURELS</span><b id="sow-hud-endgame-laurels">+0</b></div>'
            + '    </div>'
            + '    <div class="sow-hud__endgame-store" id="sow-hud-endgame-store" aria-label="Featured skin offer">'
            + '      <span class="sow-hud__endgame-store-icon" aria-hidden="true">✦</span>'
            + '      <span class="sow-hud__endgame-store-copy"><b id="sow-hud-endgame-store-name">FEATURED SKIN</b><small id="sow-hud-endgame-store-copy">ORIGINAL SOW COSMETIC</small></span>'
            + '      <button class="sow-hud__endgame-store-state" type="button" data-command="open_store" id="sow-hud-endgame-store-action">VIEW STORE</button>'
            + '    </div>'
            + '    <div class="sow-hud__endgame-actions">'
            + '      <button class="sow-hud__endgame-secondary hidden" type="button" data-command="continue_observing" id="sow-hud-endgame-observe"><span aria-hidden="true">◉</span> CONTINUE AS OBSERVER</button>'
            + '      <button class="sow-hud__endgame-primary" type="button" data-command="confirm_endgame_leave"><span aria-hidden="true">⌂</span> BACK TO MENU</button>'
            + '    </div>'
            + '  </section>'
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
            inboxCount: document.getElementById("sow-hud-inbox-count"),
            questsBtn: document.getElementById("sow-hud-quests-btn"),
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
            devSidebar: document.getElementById("sow-hud-dev-sidebar"),
            devBtn: document.getElementById("sow-hud-dev-btn"),
            settings: document.getElementById("sow-hud-settings"),
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
            transfer: document.getElementById("sow-hud-transfer"),
            transferTarget: document.getElementById("sow-hud-transfer-target"),
            transferGold: document.getElementById("sow-hud-transfer-gold"),
            transferTroops: document.getElementById("sow-hud-transfer-troops"),
            betrayal: document.getElementById("sow-hud-betrayal-modal"),
            betrayalCopy: document.getElementById("sow-hud-betrayal-copy"),
            surrender: document.getElementById("sow-hud-surrender-modal"),
            endgame: document.getElementById("sow-hud-endgame-modal"),
            endgameCard: document.querySelector("#sow-hud-endgame-modal .sow-hud__endgame-card"),
            endgameIcon: document.getElementById("sow-hud-endgame-icon"),
            endgameBanner: document.getElementById("sow-hud-endgame-banner"),
            endgameTitle: document.getElementById("sow-hud-endgame-title"),
            endgameDesc: document.getElementById("sow-hud-endgame-desc"),
            endgamePortrait: document.getElementById("sow-hud-endgame-portrait"),
            endgameStats: document.getElementById("sow-hud-endgame-stats"),
            endgameKda: document.getElementById("sow-hud-endgame-kda"),
            endgameXp: document.getElementById("sow-hud-endgame-xp"),
            endgameLeaderXp: document.getElementById("sow-hud-endgame-leader-xp"),
            endgameLaurels: document.getElementById("sow-hud-endgame-laurels"),
            endgameStore: document.getElementById("sow-hud-endgame-store"),
            endgameStoreName: document.getElementById("sow-hud-endgame-store-name"),
            endgameStoreCopy: document.getElementById("sow-hud-endgame-store-copy"),
            endgameStoreAction: document.getElementById("sow-hud-endgame-store-action"),
            endgameObserve: document.getElementById("sow-hud-endgame-observe"),
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
        hudRoot.addEventListener("input", function (event) {
            var input = event.target.closest("[data-dev]");
            if (!input) return;
            send("set_dev_config", { field: input.dataset.dev, value: Number(input.value) });
        });
        hudRoot.addEventListener("change", function (event) {
            var input = event.target.closest("[data-hud-setting]");
            if (!input) return;
            var setting = input.dataset.hudSetting;
            if (setting === "mute_all") send("set_mute", { value: !input.checked });
            if (setting === "music_volume") send("set_music_volume", { value: Number(input.value) });
            if (setting === "reduced_motion") send("set_reduced_motion", { value: input.checked });
        });
    }

    function updateLeaderboard(players) {
        if (!hudRefs || !hudRefs.rows) return;
        if (!Array.isArray(players)) return;
        var renderKey = players.map(function (player, idx) {
            return [idx, player.id, player.name, player.troops, player.tile_count, player.territory_pct, player.is_alive, player.is_me].join("|");
        }).join("\u001e");
        if (renderKey === leaderboardRenderKey) return;
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
        leaderboardRenderKey = renderKey;
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
        var renderKey = requests.map(function (request) {
            return [request.kind, request.requester_id, request.name, request.gold, request.troops, request.active].join("|");
        }).join("\u001e");
        if (renderKey === inboxRenderKey) return;
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
        inboxRenderKey = renderKey;
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
            devSidebarOpen = false;
            settingsOpen = false;

            hudRoot.dataset.overlayOpen = "false";
            leaderboardRows = Object.create(null);
            leaderboardRenderKey = "";
            inboxRenderKey = "";
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
        var quests = hud.quests || {};
        var devTools = hud.dev_tools || {};
        if (hudRefs.questsBtn) {
            hudRefs.questsBtn.classList.toggle("hidden", !quests.available);
            hudRefs.questsBtn.classList.toggle("active", Boolean(quests.open));
        }
        if (hudRefs.devBtn) {
            hudRefs.devBtn.classList.toggle("hidden", !devTools.available);
        }
        if (devTools.config && hudRefs.devSidebar) {
            hudRefs.devSidebar.querySelectorAll("[data-dev]").forEach(function (input) {
                if (document.activeElement === input) return;
                var value = devTools.config[input.dataset.dev];
                if (value != null) input.value = value;
            });
        }
        var gold = Math.floor(hud.gold || 0);
        var troops = Math.floor(hud.troops || 0);
        var maxTroops = Math.floor(hud.max_troops || 0);
        var prod = Math.floor(hud.troop_rate || 0);
        var currentRatio = hud.attack_ratio || 0.5;
        var spawnSecs = hud.spawn_timer_secs;
        var isDeploying = spawnSecs != null && spawnSecs > 0;

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
        if (hudRefs.devSidebar) {
            hudRefs.devSidebar.classList.toggle("hidden", !devSidebarOpen);
        }
        if (hudRefs.devBtn) {
            hudRefs.devBtn.classList.toggle("active", devSidebarOpen);
        }
        if (hudRefs.pinEmoji) {
            pinEmoji = Boolean(hud.pin_emoji);
            hudRefs.pinEmoji.classList.toggle("active", pinEmoji);
            hudRefs.pinEmoji.setAttribute("aria-pressed", String(pinEmoji));
        }

        if (hudRefs.settings) {
            hudRefs.settings.classList.toggle("hidden", !settingsOpen);
            if (settingsOpen && state.settings) {
                var settings = state.settings;
                var muteInput = hudRefs.settings.querySelector('[data-hud-setting="mute_all"]');
                var musicInput = hudRefs.settings.querySelector('[data-hud-setting="music_volume"]');
                var motionInput = hudRefs.settings.querySelector('[data-hud-setting="reduced_motion"]');
                if (muteInput && document.activeElement !== muteInput) muteInput.checked = !settings.mute_all;
                if (musicInput && document.activeElement !== musicInput) musicInput.value = settings.music_volume == null ? 0.8 : settings.music_volume;
                if (motionInput && document.activeElement !== motionInput) motionInput.checked = Boolean(settings.reduced_motion);
            }
        }

        if (hudRefs.inbox) {
            hudRefs.inbox.classList.toggle("hidden", !inboxOpen);
            if (inboxOpen) renderInbox(hud.inbox);
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
                var kda = hud.player_kda || {};
                var rewards = hud.rewards || {};
                var result = isWinner ? "victory" : "defeat";
                var kdaText = [kda.kills || 0, kda.deaths || 0, kda.assists || 0].join(" / ");
                if (hudRefs.endgameCard) hudRefs.endgameCard.dataset.result = result;
                if (hudRefs.endgameIcon) hudRefs.endgameIcon.textContent = isWinner ? "♛" : "⚔";
                if (hudRefs.endgameBanner) hudRefs.endgameBanner.textContent = isWinner ? "VICTORY" : "DEFEAT";
                if (hudRefs.endgameTitle) hudRefs.endgameTitle.textContent = isWinner ? "MATCH WON" : "MATCH LOST";
                if (hudRefs.endgameDesc) hudRefs.endgameDesc.textContent = isWinner ? "Map control secured." : (hud.winner_name ? "Winner: " + hud.winner_name : "Your empire was eliminated.");
                var activeLeaderId = (hud && hud.player_leader) || (state && state.selected_leader);
                var activeLeader = leaderById(activeLeaderId);
                if (hudRefs.endgamePortrait) {
                    hudRefs.endgamePortrait.src = asset("shell/leaders/" + activeLeader.slug + "_desktop.webp");
                    hudRefs.endgamePortrait.alt = activeLeader.name || "Leader Portrait";
                }
                if (hudRefs.endgameKda) hudRefs.endgameKda.textContent = kdaText;
                if (hudRefs.endgameXp) hudRefs.endgameXp.textContent = "+" + (rewards.xp || 0);
                if (hudRefs.endgameLeaderXp) hudRefs.endgameLeaderXp.textContent = "+" + (rewards.leader_xp || 0);
                if (hudRefs.endgameLaurels) hudRefs.endgameLaurels.textContent = "+" + (rewards.laurels || 0);
                var featuredSkin = hud.featured_skin;
                if (hudRefs.endgameStore) hudRefs.endgameStore.classList.toggle("hidden", !featuredSkin);
                if (featuredSkin) {
                    if (hudRefs.endgameStoreName) hudRefs.endgameStoreName.textContent = featuredSkin.name || "FEATURED SKIN";
                    if (hudRefs.endgameStoreCopy) hudRefs.endgameStoreCopy.textContent = (featuredSkin.cost_gems || 0) + " GEMS · ORIGINAL SOW COSMETIC";
                }
                if (hudRefs.endgameObserve) hudRefs.endgameObserve.classList.toggle("hidden", isWinner || Boolean(hud.winner_name));
            }
        }
        hudRoot.dataset.overlayOpen = String(Boolean(
            leaderboardOpen || inboxOpen || transferOpen || betrayalOpen ||
            surrenderModalOpen || emojiPickerOpen || isOver
            || devSidebarOpen || settingsOpen
        ));
    }

    if (hudRoot) {
        hudRoot.addEventListener("click", function (event) {
            var btn = event.target.closest("[data-command]");
            if (!btn) return;
            var cmd = btn.dataset.command;
            if (cmd === "toggle_dev_sidebar") {
                devSidebarOpen = !devSidebarOpen;
                send("toggle_dev_sidebar");
                renderHud();
            } else if (cmd === "toggle_tutorial_objectives") {
                send("toggle_tutorial_objectives");
                renderHud();
            } else if (cmd === "toggle_leaderboard") {
                leaderboardOpen = !leaderboardOpen;
                send("toggle_leaderboard");
                renderHud();
            } else if (cmd === "toggle_inbox") {
                inboxOpen = !inboxOpen;
                send("toggle_inbox");
                renderHud();
            } else if (cmd === "toggle_settings") {
                settingsOpen = !settingsOpen;
                renderHud();
            } else if (cmd === "reset_dev_config") {
                send("reset_dev_config");
            } else if (cmd === "toggle_pin_emoji") {
                pinEmoji = !pinEmoji;
                send("set_emoji_pinned", { pinned: pinEmoji });
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
            } else if (cmd === "open_store") {
                window.SOW_open_store_after_match = true;
                send("leave_lobby");
            } else if (cmd === "continue_observing") {
                send("continue_observing");
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
            if (state.hud && state.hud.dev_tools && typeof state.hud.dev_tools.open === "boolean") {
                devSidebarOpen = state.hud.dev_tools.open;
            }
        }
        if (state.phase === "MainMenu" && window.SOW_open_store_after_match) {
            window.SOW_open_store_after_match = false;
            mobileStoreOpen = true;
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

    function syncInitialState() {
        var raw = window.SOW_MENU_STATE;
        handleStateUpdate(raw);
    }

    root.hidden = true;
    if (hudRoot) hudRoot.hidden = true;
    syncInitialState();
})();
