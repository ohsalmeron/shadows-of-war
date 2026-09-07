    // ── Conduct & privacy (Terms/Privacy enforcement in-client) ──
    var REPORT_REASONS = [
        ["cheating", "Cheating / automation"],
        ["harassment", "Harassment"],
        ["hate_speech", "Hate speech"],
        ["threats", "Threats"],
        ["spam", "Spam"],
        ["inappropriate_name", "Inappropriate name"],
        ["exploiting", "Exploiting bugs"],
        ["other", "Other (explain below)"]
    ];
    var reportOpen = false;
    var reportTarget = null;
    var reportSent = false;
    var reportBusy = false;
    var deleteArmed = false;
    var deleteBusy = false;
    var ageDeclined = false;
    var ageConfirmed = false;
    try { ageConfirmed = window.localStorage.getItem("sow_age_ok") === "1"; } catch (e) {}

    function selfCreds() {
        var id = null;
        var secret = null;
        try {
            id = window.localStorage.getItem("sow_account_id");
            secret = window.localStorage.getItem("sow_account_secret");
        } catch (e) {}
        return (id && secret) ? { account_id: id, auth_secret: secret } : null;
    }

    function isBlockedPublicId(publicId) {
        try {
            return typeof window.SOW_isBlockedId === "function" && window.SOW_isBlockedId(publicId);
        } catch (e) {
            return false;
        }
    }

    function renderAgeGate() {
        var body = ageDeclined
            ? "<p>You must be 13 or older to play. Players aged 13–17 need a parent or guardian's permission.</p><a class='sow-age-gate__site-link' href='https://shadowsofwar.io/'>Back to site</a>"
            : "<p>You must be 13 or older to play. Players aged 13–17 need a parent or guardian's permission.</p>" +
              "<div class='sow-age-gate__actions'>" +
              "<button type='button' class='sow-age-gate__confirm' data-command='confirm_age'>I AM 13+ / HAVE PERMISSION</button>" +
              "<button type='button' class='sow-age-gate__leave' data-command='decline_age'>LEAVE</button></div>";
        return "<div class='sow-age-gate' role='dialog' aria-modal='true' aria-labelledby='sow-age-gate-title'>" +
            "<section class='sow-age-gate__dialog'><header class='sow-age-gate__header'><div class='sow-age-gate__portrait'><img src='" + esc(asset("gameplay/avatars/lady_six_sky.webp")) + "' alt='Lady Six Sky' width='52' height='52'><span class='sow-age-gate__mark'>13+</span></div><div><span class='sow-age-gate__eyebrow'>ACCESS CHECK</span><h2 id='sow-age-gate-title'>AGE CHECK</h2></div></header>" +
            "<div class='sow-age-gate__body'>" + body + "</div><footer class='sow-age-gate__footer'><a href='https://shadowsofwar.io/terms/' target='_blank' rel='noopener'>Terms</a><span aria-hidden='true'>·</span><a href='https://shadowsofwar.io/privacy/' target='_blank' rel='noopener'>Privacy</a></footer></section></div>";
    }

    function renderTopbar() {
        var leader = leaderById(state.selected_leader);
        var name = displayNameDraft != null ? displayNameDraft : (state.player_name || "ANONYMOUS");
        var signIn = state.name_locked ? "ACCOUNT" : "SIGN IN";
        var accountXp = Math.max(0, Number(state.xp) || 0);
        return "" +
            "<header class='sow-menu__topbar'>" +
                "<div class='sow-menu__identity'>" +
                    "<button class='sow-menu__avatar' type='button' data-command='open_leader_picker' " +
                        "aria-label='Select leader' style=\"background-image:url('" + esc(avatarImage()) + "')\"></button>" +
                    "<div class='sow-menu__profile'>" +
                        "<input data-role='display-name' name='display_name' value=\"" + esc(name) + "\" maxlength='20' " +
                            (state.name_locked ? "readonly" : "") + " aria-label='Display name'>" +
                        "<button class='sow-menu__profile-link' type='button' data-command='open_profile'>" + esc(leader.name) + " · " + esc(leader.civilization) + "</button>" +
                    "</div>" +
                "</div>" +
                "<div class='sow-menu__top-actions'>" +
                    "<div class='sow-menu__progress' data-progression data-command='open_profile' role='button' tabindex='0' title='Open profile' aria-label='Open profile'>" +
                        "<span class='sow-menu__progress-cell sow-menu__level'><small>LV</small><strong data-progression-level-value>" + esc(state.level) + "</strong></span>" +
                        "<span class='sow-menu__progress-cell sow-menu__xp'><span class='sow-menu__xp-value' data-progression-xp-value>" + esc(Math.floor(accountXp)) + " XP</span><span class='sow-menu__xp-track' aria-hidden='true'><i data-progression-xp-fill style='width:" + (accountXp % 100) + "%'></i></span></span>" +
                        "<span class='sow-menu__progress-cell sow-menu__laurels'><svg class='sow-menu__laurel-icon' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.8' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'><path d='M8 20c-3-2-5-5-5-9 3 1 5 3 6 6M16 20c3-2 5-5 5-9-3 1-5 3-6 6M9 22h6'/></svg><strong data-progression-laurels-value>" + esc(state.laurels) + "</strong></span>" +
                    "</div>" +
                    "<button class='sow-menu__signin' type='button' data-command='sign_in'>" + signIn + "</button>" +
                    "<button class='sow-menu__icon-button' type='button' data-command='toggle_settings' aria-label='Settings'>⚙</button>" +
                "</div>" +
            "</header>";
    }

    function renderCommandPanel() {
        return "" +
            "<section class='sow-menu__command'>" +
                "<div class='sow-menu__home-public'>" + renderPublicPanel() + "</div>" +
                "<div class='sow-menu__home-actions'>" +
                    "<button class='sow-menu__primary' type='button' data-command='quick_match'>QUICK MATCH <span>↗</span></button>" +
                    "<button class='sow-menu__secondary' type='button' data-command='open_browser'>LOBBY BROWSER <span>→</span></button>" +
                    "<form class='sow-menu__join' data-form='join'>" +
                        "<input name='code' inputmode='numeric' autocomplete='off' placeholder='LOBBY CODE' aria-label='Lobby code'>" +
                        "<button type='submit'>JOIN</button>" +
                    "</form>" +
                    "<button class='sow-menu__secondary' type='button' data-command='open_create'>CREATE CUSTOM GAME <span>+</span></button>" +
                    "<button class='sow-menu__secondary' type='button' data-command='mobile_nav' data-mobile-screen='store'>STORE <span>↗</span></button>" +
                    renderFeedback() +
                "</div>" +
            "</section>";
    }

    function renderFeedback() {
        var purchaseStatus = "";
        try {
            var purchase = new URLSearchParams(window.location.search).get("purchase");
            var purchaseMessages = {
                success: "Purchase received. Your gems may take a moment to appear.",
                restored: "Purchases restored.",
                cancelled: "Purchase cancelled.",
                error: "Purchase could not be completed."
            };
            if (purchaseMessages[purchase]) {
                purchaseStatus = "<div class='sow-menu__status sow-menu__status--notice'>" + esc(purchaseMessages[purchase]) + "</div>";
            }
        } catch (e) {}
        var error = state.error ? "<div class='sow-menu__status sow-menu__status--error'>" + esc(state.error) + "</div>" : "";
        var notice = state.notice ? "<div class='sow-menu__status sow-menu__status--notice'>" +
            esc({ host_left: "Host left the lobby", kicked: "You were removed from the lobby", banned: "You are banned from this lobby", connection_lost: "Connection lost" }[state.notice] || state.notice) +
            "</div>" : "";
        return purchaseStatus + error + notice;
    }

    function publicLobbies(includeMatchmaking) {
        return (state.lobbies || []).filter(function (lobby) {
            if (lobby.kind === "Matchmaking") {
                if (!includeMatchmaking) return false;
            } else if (lobby.kind !== "Custom" || lobby.is_private) {
                return false;
            }
            return true;
        });
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
                "'>" +
                "<img class='sow-menu__lobby-art' src='" + esc(lobbyThumb(lobby)) + "' alt='' loading='lazy' decoding='async'>" +
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
                "<div class='sow-menu__lobbies'>" + cards + "</div>" +
            "</section>";
    }

    function renderFooter(label) {
        return "<footer class='sow-menu__footer'>" + (label ? "<span>" + esc(label) + "</span>" : "") + "<nav class='sow-menu__footer-links' aria-label='Game links'>" +
            "<a href='/how-to-play/'>HOW TO PLAY</a><a href='/support/'>SUPPORT</a><a href='/terms/'>TERMS</a><a href='/privacy/'>PRIVACY</a><a href='/cookies/'>COOKIES</a>" +
            "<a href='https://discord.gg/d6ZDeChSE' target='_blank' rel='noopener noreferrer'>DISCORD</a><a href='https://t.me/shadowsofwario' target='_blank' rel='noopener noreferrer'>TELEGRAM</a><a href='https://github.com/worldofunreal/shadows-of-war' target='_blank' rel='noopener noreferrer'>GITHUB</a>" +
            "</nav><span>SHADOWSOFWAR.IO</span></footer>";
    }

    function renderMobileNav(active) {
        var items = [
            ["store", "🛒", "Store"],
            ["heroes", "♜", "Heroes"],
            ["battle", "⚔", "Battle"],
            ["profile", "●", "Profile"]
        ];
        return "<nav class='sow-menu__mobile-nav' aria-label='Main menu navigation'>" + items.map(function (item) {
            var selected = active === item[0];
            return "<button type='button' class='sow-menu__mobile-nav-item" + (selected ? " is-active" : "") + "' data-command='mobile_nav' data-mobile-screen='" + item[0] + "'" +
                (selected ? " aria-current='page'" : "") + " aria-label='" + esc(item[2]) + "'><span aria-hidden='true'>" + item[1] + "</span><small>" + item[2] + "</small></button>";
        }).join("") + "</nav>";
    }

    function renderHome() {
        var leader = leaderById(state.selected_leader);
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
                    "</section>" +
                "</main>" +
                renderFooter("") + renderMobileNav("battle") +
            "</div>" + settingsOverlay + renderPasswordModal();
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
                            "<div class='sow-menu__browser-search'>" +
                                "<input data-role='browser-search' type='search' placeholder='Search by map or host name...' value=\"" + esc(browserSearchQuery) + "\">" +
                            "</div>" +
                            "<div class='sow-menu__lobbies'>" + cards + "</div>" +
                        "</section>" +
                    "</section>" +
                "</main>" +
                renderFooter("LOBBY BROWSER") + renderMobileNav("battle") +
                "</div>" + renderPasswordModal();
    }

    function nativePurchaseHref(productId) {
        if (!state || !state.purchase_user_id || !state.native_purchase_scheme) return "";
        return state.native_purchase_scheme + "?product_id=" + encodeURIComponent(productId) +
            "&app_user_id=" + encodeURIComponent(state.purchase_user_id);
    }

    function isAndroidTwa() {
        var referrer = String(document.referrer || "");
        if (/^android-app:\/\/com\.shadowsofwar(?:\/|$)/i.test(referrer)) return true;
        try {
            return new URLSearchParams(window.location.search).get("sow_platform") === "android" &&
                /Android/i.test(navigator.userAgent || "");
        } catch (e) {
            return false;
        }
    }

    function webPurchaseHref(packageId) {
        if (!state || !state.purchase_user_id || !window.SOW_REVENUECAT_WEB_PURCHASE_LINK) return "";
        var href = window.SOW_REVENUECAT_WEB_PURCHASE_LINK.replace(/\/+$/, "") +
            "/" + encodeURIComponent(state.purchase_user_id);
        return packageId ? href + "?package_id=" + encodeURIComponent(packageId) : href;
    }

    function renderWebPurchaseAction() {
        if (isAndroidTwa()) return "";
        var href = webPurchaseHref();
        return href
            ? "<a class='sow-store__buy sow-store__buy--primary' href='" + esc(href) + "' target='_blank' rel='noopener'>BUY ONLINE <span aria-hidden='true'>↗</span></a>"
            : "";
    }

    function renderStoreLeader(leader) {
        var leaderSlug = leader.slug || leader.id || "null";
        var avatarUrl = asset("gameplay/avatars/" + leaderSlug + ".webp");
        var desktopArt = asset("shell/leaders/" + leaderSlug + "_desktop.webp");
        var mobileArt = asset("shell/leaders/" + leaderSlug + "_mobile.webp");
        var status = leader.owned ? "OWNED" : (leader.free_rotation ? "FREE THIS WEEK" : "LOCKED");
        var action = leader.owned || leader.free_rotation
            ? "<span class='sow-store__offer-state'>" + status + "</span>"
            : "<div class='sow-store__buy-row'>" +
              "<button class='sow-store__buy' type='button' data-command='unlock_leader' data-currency='laurels' data-leader-id='" + esc(leader.id) + "'>" + esc(leader.cost_laurels) + " LAURELS</button>" +
              "<button class='sow-store__buy' type='button' data-command='unlock_leader' data-currency='gems' data-leader-id='" + esc(leader.id) + "'>" + esc(leader.cost_gems) + " GEMS</button>" +
              "</div>";
        return "<article class='sow-store__leader-card " + (leader.owned ? "is-owned" : "") + "'>" +
            "<div class='sow-store__leader-visual'><picture><source media='(max-width: 700px)' srcset='" + esc(mobileArt) + "'><img src='" + esc(desktopArt) + "' alt='" + esc(leader.name) + "' width='640' height='360' loading='lazy'></picture>" +
                "<span class='sow-store__leader-avatar'><img src='" + esc(avatarUrl) + "' alt='' width='64' height='64' loading='lazy'></span>" +
                "<span class='sow-store__leader-status'>" + esc(status) + "</span>" +
            "</div><div class='sow-store__leader-body'><div class='sow-store__leader-title'><h3>" + esc(leader.name) + "</h3><span>" + esc(leader.civilization) + "</span></div>" +
            "<p class='sow-store__leader-perk'>" + esc(leader.perk) + "</p><div class='sow-store__leader-action'>" + action + "</div></div></article>";
    }

    function renderStoreBundle(bundle) {
        var href = isAndroidTwa() ? nativePurchaseHref(bundle.product_id) : "";
        var action = isAndroidTwa()
            ? (href
                ? "<a class='sow-store__buy sow-store__buy--primary' href='" + esc(href) + "'>BUY IN APP <span aria-hidden='true'>↗</span></a>"
                : "<button class='sow-store__buy' type='button' disabled>ACCOUNT REQUIRED</button>")
            : "";
        return "<article class='sow-store__bundle'><span class='sow-store__bundle-icon' aria-hidden='true'>✦</span><div><strong>" + esc(bundle.gems) + " GEMS</strong><small>One-time gem bundle</small></div>" + action + "</article>";
    }

    function renderStoreSkin(skin) {
        var equipped = state.selected_skin === skin.id;
        var action;
        if (equipped) {
            action = "<span class='sow-store__offer-state'>EQUIPPED</span>";
        } else if (skin.owned) {
            action = "<button class='sow-store__buy' type='button' data-command='equip_skin' data-skin-id='" + esc(skin.id) + "'>EQUIP</button>";
        } else {
            action = "<button class='sow-store__buy' type='button' data-command='unlock_skin' data-skin-id='" + esc(skin.id) + "'>UNLOCK " + esc(skin.cost_gems) + " GEMS</button>";
        }
        return "<article class='sow-store__skin'><div class='sow-store__skin-art'><img src='" + esc(asset(skin.asset_path)) + "' alt='' width='96' height='96' loading='lazy'></div><div class='sow-store__skin-body'><h3>" + esc(skin.name) + "</h3><p>All leaders</p>" + action + "</div></article>";
    }

    function renderStore() {
        var store = state.store || {};
        var leaders = Array.isArray(store.leaders) ? store.leaders : [];
        var skins = Array.isArray(store.skins) ? store.skins : [];
        var bundles = Array.isArray(store.gem_bundles) ? store.gem_bundles : [];
        return "" +
            "<div class='sow-menu__backdrop sow-store__backdrop'></div>" +
            "<div class='sow-menu__shell sow-menu__store'>" +
                renderTopbar() +
                "<main class='sow-menu__main sow-menu__main--store'><section class='sow-menu__store-slot' data-store-slot aria-label='Store'>" +
                    "<header class='sow-store__heading'><div><p class='sow-store__eyebrow'>STORE</p><h1>Store</h1><p>Leaders, skins and gems.</p></div><div class='sow-store__balances'><span><b>✦</b> " + esc(store.gems || 0) + " <small>GEMS</small></span><span><b>◈</b> " + esc(store.laurels || 0) + " <small>LAURELS</small></span></div></header>" +
                    renderFeedback() +
                    "<section class='sow-store__section' aria-labelledby='sow-store-leaders'><div class='sow-store__section-head'><h2 id='sow-store-leaders'>Leaders</h2><span>Weekly rotation</span></div><div class='sow-store__leader-grid'>" + (leaders.map(renderStoreLeader).join("") || "<p class='sow-menu__empty'>No leaders available.</p>") + "</div><p class='sow-store__fineprint'>Unlocked leaders have distinct gameplay perks — they are not purely cosmetic. Gem unlocks are final sale, no refunds.</p></section>" +
                    "<section class='sow-store__section' aria-labelledby='sow-store-skins'><div class='sow-store__section-head'><h2 id='sow-store-skins'>Skins</h2><span>Cosmetics</span></div><div class='sow-store__skin-grid'>" + (skins.map(renderStoreSkin).join("") || "<p class='sow-menu__empty'>No skins available.</p>") + "</div></section>" +
                    "<section class='sow-store__section' aria-labelledby='sow-store-gems'><div class='sow-store__section-head'><h2 id='sow-store-gems'>Gem bundles</h2>" + renderWebPurchaseAction() + "</div><div class='sow-store__bundle-grid'>" + (bundles.map(renderStoreBundle).join("") || "<p class='sow-menu__empty'>Products are not configured yet.</p>") + "</div>" +
                    "<p class='sow-store__fineprint'>Digital items delivered instantly to your account. <strong>All sales are final — no refunds.</strong> Buying means you consent to immediate delivery and give up the statutory withdrawal right where applicable. See <a href='https://shadowsofwar.io/terms/' target='_blank' rel='noopener'>Terms</a>.</p></section>" +
                "</section></main>" +
                renderFooter("STORE") + renderMobileNav("store") +
            "</div>";
    }

    function renderHeroesCard(leader, activeId) {
        var selected = leader.id === activeId;
        var avatarUrl = asset("gameplay/avatars/" + leader.slug + ".webp");
        return "<button class='sow-heroes__card" + (selected ? " is-selected" : "") + "' type='button' data-command='preview_leader' data-leader-id='" + esc(leader.id) + "' aria-pressed='" + selected + "'>" +
            "<span class='sow-heroes__card-art' style=\"background-image:url('" + esc(avatarUrl) + "')\"></span>" +
            "<span class='sow-heroes__card-copy'><strong>" + esc(leader.name) + "</strong><small>" + esc(leader.civilization) + "</small><em>" + esc(leader.perk || "Command trait") + "</em></span>" +
            (selected ? "<span class='sow-heroes__card-state'>SELECTED</span>" : "") +
            "</button>";
    }

    function heroesRoster(leaders) {
        var query = heroesSearchQuery.trim().toLowerCase();
        return leaders.filter(function (leader) {
            var matchesQuery = !query ||
                String(leader.name || "").toLowerCase().indexOf(query) !== -1 ||
                String(leader.civilization || "").toLowerCase().indexOf(query) !== -1;
            var regionKey = String(leader.slug || leader.id || "").toLowerCase();
            var matchesRegion = heroesRegionFilter === "all" || LEADER_REGIONS[regionKey] === heroesRegionFilter;
            return matchesQuery && matchesRegion;
        });
    }

    function renderHeroesRegionOptions() {
        return ["all", "Europe", "Africa", "Asia", "Americas"].map(function (region) {
            var label = region === "all" ? "All regions" : region;
            return "<option value='" + esc(region) + "'" + (heroesRegionFilter === region ? " selected" : "") + ">" + esc(label) + "</option>";
        }).join("");
    }

    function renderHeroesRoster(activeId) {
        var leaders = state && Array.isArray(state.leaders) ? state.leaders : [];
        var filtered = heroesRoster(leaders);
        var cards = filtered.map(function (leader) { return renderHeroesCard(leader, activeId); }).join("");
        return cards || "<p class='sow-menu__empty sow-heroes__empty'>No leaders found.</p>";
    }

    function renderHeroes() {
        var activeId = tempSelectedLeader || (state && state.selected_leader) || "Caesar";
        var activeLeader = leaderById(activeId);
        var portraitAsset = asset("shell/leaders/" + activeLeader.slug + "_mobile.webp");
        var landscapeAsset = asset("shell/leaders/" + activeLeader.slug + "_desktop.webp");
        return "" +
            "<div class='sow-menu__backdrop sow-heroes__backdrop'></div>" +
            "<div class='sow-menu__shell sow-menu__heroes'>" +
                renderTopbar() +
                "<main class='sow-menu__main sow-menu__main--heroes'><section class='sow-menu__heroes-slot' aria-label='Heroes'>" +
                    "<div class='sow-heroes__workspace'>" +
                        "<section class='sow-heroes__featured' aria-labelledby='sow-heroes-selected'><picture><source media='(max-width: 700px) and (orientation: portrait)' srcset='" + esc(landscapeAsset) + "'><img src='" + esc(portraitAsset) + "' alt='" + esc(activeLeader.name) + "' width='1080' height='1920' fetchpriority='high'></picture><div class='sow-heroes__featured-copy'><p class='sow-heroes__featured-label'>SELECTED</p><h2 id='sow-heroes-selected'>" + esc(activeLeader.name) + "</h2><p class='sow-heroes__civilization'>" + esc(activeLeader.civilization) + "</p><p class='sow-heroes__perk'>" + esc(activeLeader.perk || "Enhanced military & empire bonuses.") + "</p><button class='sow-menu__primary sow-heroes__confirm' type='button' data-command='confirm_leader' data-leader-id='" + esc(activeLeader.id) + "'>CONFIRM " + esc(activeLeader.name.toUpperCase()) + " <span>✓</span></button></div></section>" +
                        "<section class='sow-heroes__roster' aria-label='Leader list'><div class='sow-heroes__section-head'><div class='sow-heroes__filters'><label class='sow-heroes__search'><span class='sow-heroes__sr-only'>Search leaders</span><input data-role='heroes-search' type='search' placeholder='Search leader or civilization' value=\"" + esc(heroesSearchQuery) + "\" autocomplete='off' spellcheck='false'></label><label class='sow-heroes__region'><span class='sow-heroes__sr-only'>Filter leaders by region</span><select data-role='heroes-region' aria-label='Filter leaders by region'>" + renderHeroesRegionOptions() + "</select></label></div></div><div class='sow-heroes__grid' data-heroes-roster aria-live='polite'>" + renderHeroesRoster(activeId) + "</div></section>" +
                    "</div>" +
                "</section></main>" +
                renderFooter("HEROES") + renderMobileNav("heroes") +
            "</div>";
    }

    function profileLeaderCard(leader) {
        var summary = leader || {};
        var leaderInfo = leaderById(summary.leader);
        return "<article class='sow-profile__leader'>" +
            "<img src='" + esc(asset("gameplay/avatars/" + leaderInfo.slug + ".webp")) + "' alt='' width='48' height='48' loading='lazy'>" +
            "<div><strong>" + esc(summary.leader || leaderInfo.name) + "</strong>" +
            "<span>" + esc(summary.matches_played || 0) + " matches · " + esc(Math.round((summary.win_rate || 0) * 100)) + "% wins</span></div>" +
            "<b>LV " + esc(1 + Math.floor((summary.xp || 0) / 100)) + "</b>" +
            "</article>";
    }

    function profileRecentLeaders(matches) {
        var recent = Array.isArray(matches) ? matches.slice(0, 10) : [];
        var knownLeaders = state && Array.isArray(state.leaders) ? state.leaders : [];
        var counts = {};
        recent.forEach(function (match, index) {
            var leaderId = match && match.leader;
            var leaderInfo = knownLeaders.find(function (leader) { return leader.id === leaderId; });
            if (!leaderInfo) return;
            var entry = counts[leaderInfo.id];
            if (!entry) {
                entry = counts[leaderInfo.id] = { leader: leaderInfo, matches: 0, lastIndex: index };
            }
            entry.matches += 1;
            entry.lastIndex = Math.min(entry.lastIndex, index);
        });
        return Object.keys(counts).map(function (id) { return counts[id]; }).sort(function (left, right) {
            return right.matches - left.matches || left.lastIndex - right.lastIndex;
        }).slice(0, 3);
    }

    function profileRecentLeadersPanel(matches) {
        var recent = Array.isArray(matches) ? matches.slice(0, 10) : [];
        var favorites = profileRecentLeaders(recent);
        var cards = favorites.map(function (entry) {
            var leader = entry.leader;
            var share = recent.length ? Math.round((entry.matches / recent.length) * 100) : 0;
            var unit = entry.matches === 1 ? "game" : "games";
            return "<article class='sow-profile__favorite'><img src='" + esc(asset("gameplay/avatars/" + leader.slug + ".webp")) + "' alt='" + esc(leader.name) + " avatar' width='64' height='64' loading='lazy'><div class='sow-profile__favorite-copy'><strong>" + esc(leader.name) + "</strong><span>" + esc(entry.matches) + " " + unit + " · " + esc(share) + "%</span></div></article>";
        }).join("");
        return "<section class='sow-profile__favorites' aria-labelledby='sow-profile-favorites-title'><div class='sow-profile__favorites-head'><h2 id='sow-profile-favorites-title'>Most played leaders</h2><span>Last " + esc(recent.length) + " matches</span></div>" +
            (cards ? "<div class='sow-profile__favorites-track' data-count='" + esc(favorites.length) + "'>" + cards + "</div>" : "<p class='sow-profile__favorites-empty'>No leader data yet.</p>") +
            "</section>";
    }

    function profileMatchRow(match) {
        var result = match.won ? "WIN" : "LOSS";
        var mode = match.mode || "FFA";
        var map = match.map_name || "WORLD MAP";
        var kda = (match.kills || 0) + " / " + (match.deaths || 0) + " / " + (match.assists || 0);
        return "<button type='button' class='sow-profile__match' data-command='open_match' data-match-id='" + esc(match.match_id) + "'>" +
            "<span class='sow-profile__match-result " + (match.won ? "is-win" : "is-loss") + "'>" + result + "</span>" +
            "<span class='sow-profile__match-context'><strong>" + esc(mode) + "</strong><small>" + esc(map) + " · " + esc(match.queue || "MATCHMAKING") + "</small></span>" +
            "<span class='sow-profile__match-kda'><strong>" + esc(match.leader || "—") + "</strong><small>" + esc(kda) + " K/D/A</small></span>" +
            "<span class='sow-profile__match-rating'>" + (match.rating_delta == null ? "—" : (match.rating_delta >= 0 ? "+" : "") + esc(match.rating_delta) + " SR") + "</span>" +
            "</button>";
    }

    function renderProfile() {
        var data = profileData;
        var own = state && state.public_profile_id === profilePublicId;
        var title = own ? "Your profile" : "Player profile";
        var header = data
            ? "<section class='sow-profile__heading' aria-labelledby='sow-profile-title'><div class='sow-profile__identity-card'><div class='sow-profile__heading-top'><span class='sow-profile__kicker'>" + esc(title) + "</span><button type='button' class='sow-profile__back' data-command='close_profile'>← Back</button></div><div class='sow-profile__identity-main'><div class='sow-profile__identity-copy'><h1 id='sow-profile-title'>" + esc(data.display_name) + "</h1><p class='sow-profile__handle'>" + esc(data.handle) + "</p></div><div class='sow-profile__level'><small>LEVEL</small><strong>" + esc(data.level) + "</strong></div></div></div>" + profileRecentLeadersPanel(profileHistory) + "</section>"
            : "<section class='sow-profile__heading sow-profile__heading--loading' aria-live='polite'><div class='sow-profile__identity-card'><div class='sow-profile__heading-top'><span class='sow-profile__kicker'>Player profile</span><button type='button' class='sow-profile__back' data-command='close_profile'>← Back</button></div><h1 id='sow-profile-title'>" + (profileLoading ? "Loading…" : "Profile unavailable") + "</h1><p class='sow-profile__handle'>" + esc(profileError || "Try again or return to the menu.") + "</p></div></section>";
        var tabs = ["overview", "leaders", "history", "ranked"].map(function (tab) {
            var active = profileTab === tab;
            return "<button type='button' role='tab' id='sow-profile-tab-" + tab + "' class='sow-profile__tab" + (active ? " is-active" : "") + "' aria-selected='" + active + "' aria-controls='sow-profile-panel-" + tab + "' data-command='profile_tab' data-profile-tab='" + tab + "'>" + tab.charAt(0).toUpperCase() + tab.slice(1) + "</button>";
        }).join("");
        var playGamesActions = "";
        if (typeof window.SOW_isAndroidTwa === "function" && window.SOW_isAndroidTwa()) {
            playGamesActions = "<section class='sow-profile__section' aria-label='Google Play Games'><div class='sow-profile__section-head'><h2>Google Play Games</h2><span class='sow-profile__section-note'>Android</span></div><div class='sow-profile__actions'><button type='button' class='sow-menu__ghost-button' data-command='open_play_games' data-play-games-section='achievements'>Achievements</button><button type='button' class='sow-menu__ghost-button' data-command='open_play_games' data-play-games-section='leaderboards'>Leaderboards</button></div></section>";
        }
        var content = "";
        if (data && profileTab === "overview") {
            content = "<div id='sow-profile-panel-overview' class='sow-profile__panel-content' role='tabpanel' aria-labelledby='sow-profile-tab-overview'><div class='sow-profile__stats'>" +
                "<div><strong>" + esc(data.matches_played) + "</strong><span>Matches</span></div>" +
                "<div><strong>" + esc(data.wins) + "</strong><span>Wins</span></div>" +
                "<div><strong>" + esc(Math.round((data.win_rate || 0) * 100)) + "%</strong><span>Win rate</span></div>" +
                "<div class='sow-profile__stat-kda'><strong><i>" + esc(data.kills) + "</i><i>" + esc(data.deaths) + "</i><i>" + esc(data.assists) + "</i></strong><span>K / D / A</span></div>" +
                "</div><div class='sow-profile__columns'><section class='sow-profile__section'><div class='sow-profile__section-head'><h2>Recent matches</h2><button type='button' class='sow-profile__text-action' data-command='profile_tab' data-profile-tab='history'>View all</button></div>" +
                (profileHistory.slice(0, 10).map(profileMatchRow).join("") || "<p class='sow-profile__empty'>No completed matches.</p>") +
                "</section><section class='sow-profile__section'><div class='sow-profile__section-head'><h2>Leaders</h2><button type='button' class='sow-profile__text-action' data-command='profile_tab' data-profile-tab='leaders'>View all</button></div>" +
                ((data.leaders || []).slice(0, 4).map(profileLeaderCard).join("") || "<p class='sow-profile__empty'>No leader history.</p>") +
                "</section></div></div>";
        } else if (data && profileTab === "leaders") {
            content = "<section id='sow-profile-panel-leaders' class='sow-profile__section sow-profile__panel-content' role='tabpanel' aria-labelledby='sow-profile-tab-leaders'><div class='sow-profile__section-head'><h2>Leader mastery</h2><span class='sow-profile__section-note'>" + esc((data.leaders || []).length) + " leaders</span></div><div class='sow-profile__leaders'>" +
                ((data.leaders || []).map(profileLeaderCard).join("") || "<p class='sow-profile__empty'>No leader history.</p>") +
                "</div></section>";
        } else if (profileTab === "history") {
            content = "<section id='sow-profile-panel-history' class='sow-profile__section sow-profile__panel-content' role='tabpanel' aria-labelledby='sow-profile-tab-history'><div class='sow-profile__section-head'><h2>Match history</h2><span class='sow-profile__section-note'>" + esc(data ? data.matches_played : 0) + " matches</span></div><div class='sow-profile__history'>" +
                (profileHistory.map(profileMatchRow).join("") || "<p class='sow-profile__empty'>No completed matches.</p>") +
                "</div>" +
                (profilePublicId ? "<button type='button' class='sow-profile__load-more' data-command='load_profile_more'" + (profileLoading ? " disabled" : "") + ">" + (profileLoading ? "Loading…" : "Load more matches") + "</button>" : "") +
                "</section>";
        } else if (data && profileTab === "ranked") {
            var ratings = profileRatings === null
                ? "<p class='sow-profile__empty'>" + (profileLoading ? "Loading ranked records…" : "No ranked records.") + "</p>"
                : (profileRatings.map(function (rating) {
                    return "<article class='sow-profile__rating'><div><strong>" + esc(rating.season_name) + "</strong><span>" + esc(rating.queue) + " · " + esc(rating.mode) + "</span></div><b>" + esc(rating.tier) + (rating.division ? " " + esc(rating.division) : "") + "</b><strong>" + esc(rating.score) + " SR</strong><small>" + esc(rating.games_played) + " games · " + esc(rating.wins) + " wins · peak " + esc(rating.peak_score) + "</small></article>";
                }).join("") || "<p class='sow-profile__empty'>No ranked records.</p>");
            content = "<section id='sow-profile-panel-ranked' class='sow-profile__section sow-profile__panel-content' role='tabpanel' aria-labelledby='sow-profile-tab-ranked'><div class='sow-profile__section-head'><h2>Ranked</h2><span class='sow-profile__section-note'>Season records</span></div><div class='sow-profile__ratings'>" + ratings + "</div></section>";
        } else if (!data) {
            content = "<section class='sow-profile__state' aria-live='polite'><strong>" + esc(profileLoading ? "Loading profile…" : "Profile unavailable") + "</strong><span>" + esc(profileError || "Try again or return to the menu.") + "</span>" + (profileLoading ? "" : "<button type='button' class='sow-profile__load-more' data-command='retry_profile'>Try again</button>") + "</section>";
        }
        var search = "<section class='sow-profile__directory'><div><h2>Find a player</h2><p>Search public player profiles by name or handle.</p></div><form class='sow-profile__search' data-form='profile-search'><label class='sow-profile__sr-only' for='sow-profile-search-input'>Player name or handle</label><input id='sow-profile-search-input' name='q' type='search' autocomplete='off' placeholder='Name or handle…' aria-label='Player name or handle'><button type='submit'>Search</button></form></section>";
        var searchResults = profileSearchResults.length
            ? "<div class='sow-profile__search-results' aria-live='polite'>" + profileSearchResults.map(function (summary) {
                return "<button type='button' class='sow-profile__search-result' data-command='open_public_profile' data-profile-id='" + esc(summary.public_id) + "'><strong>" + esc(summary.display_name) + "</strong><span>" + esc(summary.handle) + " · LV " + esc(summary.level) + "</span></button>";
            }).join("") + "</div>"
            : "";
        var detail = profileMatchDetail
            ? "<div class='sow-profile__detail-backdrop'><section class='sow-profile__detail' role='dialog' aria-modal='true' aria-labelledby='sow-profile-detail-title' tabindex='-1'><button type='button' class='sow-profile__detail-close' data-command='close_match' aria-label='Close match details'>×</button><span class='sow-profile__kicker'>Match details</span><h2 id='sow-profile-detail-title'>" + esc(profileMatchDetail.mode || "MATCH") + "</h2><p>" + esc(profileMatchDetail.map_name || "WORLD") + " · " + esc(profileMatchDetail.queue || "MATCHMAKING") + "</p><div class='sow-profile__detail-players'>" + (profileMatchDetail.participants || []).map(function (participant) {
                return "<div><strong>" + esc(participant.handle || participant.public_id) + "</strong><span>" + esc(participant.leader || "—") + " · " + esc(participant.kills || 0) + " / " + esc(participant.deaths || 0) + " / " + esc(participant.assists || 0) + "</span><b>" + (participant.won ? "WIN" : "LOSS") + "</b></div>";
            }).join("") + "</div></section></div>"
            : "";
        return "<div class='sow-menu__backdrop'></div><div class='sow-menu__shell sow-profile'>" +
            renderTopbar() +
            "<main class='sow-menu__main sow-profile__main'><section class='sow-profile__page'>" + header +
            "<nav class='sow-profile__tabs' role='tablist' aria-label='Profile sections'>" + tabs + "</nav>" + playGamesActions + content + renderModeration(own) + search + searchResults +
            "</section></main>" +
            renderFooter("PROFILE") + renderMobileNav("profile") + detail + "</div>";
    }

    // Conduct + privacy controls on profiles. Own profile: self-service
    // erasure (double-confirm, ownership-proofed server-side). Others:
    // report with closed-reason dropdown + free text for Other; filing
    // also blocks the account client-side and notifies moderation.
    function renderModeration(own) {
        if (own) {
            if (!selfCreds()) return "";
            var delAction = deleteBusy
                ? "<p class='sow-profile__empty'>Deleting account…</p>"
                : deleteArmed
                    ? "<p><strong>This is permanent.</strong> Your account, profile, items, and history will be erased. Click again to confirm.</p><button type='button' class='sow-menu__danger' data-command='confirm_delete'>YES, DELETE MY ACCOUNT</button> <button type='button' class='sow-menu__ghost-button' data-command='cancel_delete'>CANCEL</button>"
                    : "<button type='button' class='sow-menu__ghost-button' data-command='request_delete'>Delete my account…</button>";
            return "<section class='sow-profile__section' aria-label='Danger zone'><div class='sow-profile__section-head'><h2>Danger zone</h2></div>" + delAction + "</section>";
        }
        if (!profilePublicId || !selfCreds()) return "";
        if (reportSent) {
            return "<section class='sow-profile__section' aria-label='Report filed'><div class='sow-profile__section-head'><h2>Report filed</h2></div><p class='sow-profile__empty'>Thanks — our moderation team will review this player. They are now blocked for you.</p></section>";
        }
        if (isBlockedPublicId(profilePublicId)) {
            return "<section class='sow-profile__section' aria-label='Blocked'><div class='sow-profile__section-head'><h2>Blocked</h2></div><p class='sow-profile__empty'>You blocked this player when you reported them.</p></section>";
        }
        if (!reportOpen || reportTarget !== profilePublicId) {
            return "<section class='sow-profile__section' aria-label='Conduct'><div class='sow-profile__section-head'><h2>Conduct</h2></div><button type='button' class='sow-menu__ghost-button' data-command='open_report' data-profile-id='" + esc(profilePublicId) + "'>Report player…</button></section>";
        }
        var options = REPORT_REASONS.map(function (r) {
            return "<option value='" + esc(r[0]) + "'>" + esc(r[1]) + "</option>";
        }).join("");
        return "<section class='sow-profile__section' aria-label='Report player'><div class='sow-profile__section-head'><h2>Report player</h2></div>" +
            "<form data-form='report'>" +
            "<label for='sow-report-reason'>Reason</label>" +
            "<select id='sow-report-reason' name='reason'>" + options + "</select>" +
            "<label for='sow-report-details'>Details (required for Other)</label>" +
            "<textarea id='sow-report-details' name='details' rows='3' maxlength='500' placeholder='What happened?'></textarea>" +
            "<p class='sow-profile__empty'>Filing also blocks this player for you and notifies our moderation team. False reports violate the Terms.</p>" +
            "<button type='submit'" + (reportBusy ? " disabled" : "") + ">" + (reportBusy ? "Sending…" : "Send report & block") + "</button> " +
            "<button type='button' class='sow-menu__ghost-button' data-command='cancel_report'>Cancel</button>" +
            "</form></section>";
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
                                        (isSp ? "START SIMULATION" : "CREATE LOBBY") + " <span>↗</span>" +
                                    "</button>" +
                                    "<button class='sow-menu__ghost-button' type='button' data-command='close_overlay'>CANCEL</button>" +
                                "</div>" +
                            "</div>" +
                        "</form>" +
                    "</section>" +
                "</main>" +
                renderFooter("CREATE GAME") + renderMobileNav("battle") +
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
                            "<p class='sow-menu__panel-label'>PLAYERS</p>" +
                            "<span class='sow-menu__player-count-badge'>" + (lobby ? (lobby.num_players || 0) + " / " + (lobby.max_players || 8) + " PLAYERS" : "—") + "</span>" +
                        "</div>" +
                        "<div class='sow-menu__queue-roster-wrap'>" + rosterHtml + "</div>" +
                    "</section>" +
                "</main>" +
                renderFooter("LOBBY") + renderMobileNav("battle") +
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
                                "<option value='full' " + (!settings.reduced_motion ? "selected" : "") + ">FULL</option>" +
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
        var sameScreen = previousScreen === screen;
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
        var scrollTop = null;
        if (sameScreen) {
            var currentMain = root.querySelector(".sow-menu__main");
            if (currentMain) scrollTop = currentMain.scrollTop;
        }

        if (screen === "home") root.innerHTML = renderHome();
        else if (screen === "browser") root.innerHTML = renderBrowser();
        else if (screen === "create") root.innerHTML = renderCreate();
        else if (screen === "queue") root.innerHTML = renderQueue();
        else if (screen === "profile") root.innerHTML = renderProfile();
        else if (screen === "heroes") root.innerHTML = renderHeroes();
        else if (screen === "store") root.innerHTML = renderStore();
        else root.innerHTML = "";

        // 13+ eligibility gate (Terms). Blocks the whole menu until confirmed;
        // the choice persists per device. Under-13s must leave.
        if (!ageConfirmed) root.innerHTML += renderAgeGate();

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
        if (sameScreen && scrollTop !== null) {
            var nextMain = root.querySelector(".sow-menu__main");
            if (nextMain) nextMain.scrollTop = scrollTop;
        }
    }

    function updateDynamic() {
        if (!state || root.hidden) return;
        var progression = root.querySelector("[data-progression]");
        if (progression) {
            var progressionXp = Math.max(0, Number(state.xp) || 0);
            var progressionLevel = Math.max(1, Number(state.level) || 1);
            var levelValue = progression.querySelector("[data-progression-level-value]");
            var xpValue = progression.querySelector("[data-progression-xp-value]");
            var xpFill = progression.querySelector("[data-progression-xp-fill]");
            var laurelsValue = progression.querySelector("[data-progression-laurels-value]");
            if (levelValue) levelValue.textContent = progressionLevel;
            if (xpValue) xpValue.textContent = Math.floor(progressionXp) + " XP";
            if (xpFill) xpFill.style.width = (progressionXp % 100) + "%";
            if (laurelsValue) laurelsValue.textContent = Math.max(0, Number(state.laurels) || 0);
        }
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
        if (command === "mobile_nav") {
            var mobileScreen = target.dataset.mobileScreen || "battle";
            settingsOpen = false;
            passwordLobbyId = null;
            passwordDraft = "";
            tempSelectedLeader = null;
            if (mobileScreen === "heroes") {
                profileOpen = false;
                profilePublicId = null;
                profileMatchDetail = null;
                mobileStoreOpen = false;
                mobileHeroesOpen = true;
                heroesSearchQuery = "";
                heroesRegionFilter = "all";
                tempSelectedLeader = state ? state.selected_leader : "Caesar";
                render();
                return;
            }
            if (mobileScreen === "profile") {
                mobileStoreOpen = false;
                if (!profileOpen || profilePublicId !== (state && state.public_profile_id)) {
                    openProfile(null);
                } else {
                    render();
                }
                return;
            }
            if (mobileScreen === "store") {
                profileOpen = false;
                mobileHeroesOpen = false;
                mobileStoreOpen = true;
                render();
                return;
            }
            profileOpen = false;
            profilePublicId = null;
            profileMatchDetail = null;
            mobileHeroesOpen = false;
            mobileStoreOpen = false;
            if (state.show_browser || state.show_create) {
                send("close_overlay");
            } else {
                render();
            }
            return;
        }
        if (command === "open_profile" || command === "open_public_profile") {
            mobileStoreOpen = false;
            mobileHeroesOpen = false;
            openProfile(target.dataset.profileId || null);
            return;
        }
        if (command === "close_profile") {
            profileOpen = false;
            mobileStoreOpen = false;
            mobileHeroesOpen = false;
            profilePublicId = null;
            profileData = null;
            profileSearchResults = [];
            profileRatings = null;
            profileMatchDetail = null;
            render();
            return;
        }
        if (command === "profile_tab") {
            profileTab = target.dataset.profileTab || "overview";
            if (profileTab === "history" && profileHistory.length === 0) {
                loadMoreProfileHistory();
            } else if (profileTab === "ranked") {
                loadProfileRatings();
            } else {
                render();
            }
            return;
        }
        if (command === "open_play_games") {
            if (typeof window.SOW_openAndroidPlayGames === "function") {
                window.SOW_openAndroidPlayGames(target.dataset.playGamesSection || "");
            }
            return;
        }
        if (command === "load_profile_more") {
            loadMoreProfileHistory();
            return;
        }
        if (command === "retry_profile") {
            profileData = null;
            profileHistory = [];
            profileHistoryCursor = 0;
            profileRatings = null;
            profileError = "";
            render();
            loadProfile(profilePublicId);
            return;
        }
        if (command === "open_match") {
            var matchId = target.dataset.matchId;
            if (!matchId || profileLoading) return;
            profileLoading = true;
            fetch(profileApi("/matches/" + encodeURIComponent(matchId)), {
                headers: { "Accept": "application/json" }
            }).then(function (response) {
                if (!response.ok) throw new Error("match detail failed");
                return response.json();
            }).then(function (detail) {
                profileMatchDetail = detail;
            }).catch(function () {
                profileError = "Match details unavailable.";
            }).finally(function () {
                profileLoading = false;
                render();
            });
            return;
        }
        if (command === "close_match") {
            profileMatchDetail = null;
            render();
            return;
        }
        if (command === "confirm_age") {
            ageConfirmed = true;
            ageDeclined = false;
            try { window.localStorage.setItem("sow_age_ok", "1"); } catch (e) {}
            render();
            return;
        }
        if (command === "decline_age") {
            ageDeclined = true;
            render();
            return;
        }
        if (command === "open_report") {
            reportOpen = true;
            reportTarget = target.dataset.profileId || profilePublicId;
            reportSent = false;
            render();
            return;
        }
        if (command === "cancel_report") {
            reportOpen = false;
            reportTarget = null;
            render();
            return;
        }
        if (command === "request_delete") {
            deleteArmed = true;
            render();
            return;
        }
        if (command === "cancel_delete") {
            deleteArmed = false;
            render();
            return;
        }
        if (command === "confirm_delete") {
            var delCreds = selfCreds();
            if (!delCreds || deleteBusy) {
                deleteArmed = false;
                render();
                return;
            }
            deleteBusy = true;
            render();
            fetch(profileApi("/profile/anonymous/delete"), {
                method: "POST",
                headers: { "Content-Type": "application/json", "Accept": "application/json" },
                body: JSON.stringify({ account_id: delCreds.account_id, auth_secret: delCreds.auth_secret })
            }).then(function (response) {
                if (!response.ok) throw new Error("delete failed");
                try {
                    ["sow_account_id", "sow_account_secret", "sow_player_progress"].forEach(function (k) {
                        window.localStorage.removeItem(k);
                    });
                } catch (e) {}
                window.location.reload();
            }).catch(function () {
                deleteBusy = false;
                deleteArmed = false;
                state.error = "Account deletion unavailable. Try again or email hello@shadowsofwar.io.";
                render();
            });
            return;
        }
        if (command === "open_leader_picker") {
            profileOpen = false;
            profilePublicId = null;
            profileMatchDetail = null;
            profileSearchResults = [];
            mobileStoreOpen = false;
            mobileHeroesOpen = true;
            heroesSearchQuery = "";
            heroesRegionFilter = "all";
            tempSelectedLeader = state ? state.selected_leader : "Caesar";
            settingsOpen = false;
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
                mobileHeroesOpen = false;
                heroesSearchQuery = "";
                heroesRegionFilter = "all";
                tempSelectedLeader = null;
                render();
            }
            return;
        }
        if (command === "unlock_leader") {
            var unlockLeaderId = target.dataset.leaderId;
            var unlockCurrency = target.dataset.currency || "laurels";
            var unlockAccountId = state && state.purchase_user_id;
            var unlockSecret = null;
            try { unlockSecret = window.localStorage.getItem("sow_account_secret"); } catch (e) {}
            if (!unlockLeaderId || !unlockAccountId || !unlockSecret) {
                state.error = "Account setup is required before unlocking a leader.";
                render();
                return;
            }
            target.disabled = true;
            fetch(profileApi("/store/leaders/unlock"), {
                method: "POST",
                headers: { "Content-Type": "application/json", "Accept": "application/json" },
                body: JSON.stringify({
                    public_id: unlockAccountId,
                    auth_secret: unlockSecret,
                    leader_id: unlockLeaderId,
                    currency: unlockCurrency
                })
            }).then(function (response) {
                if (!response.ok) throw new Error("unlock failed");
                window.location.reload();
            }).catch(function () {
                state.error = "Leader unlock unavailable.";
                render();
            });
            return;
        }
        if (command === "unlock_skin" || command === "equip_skin") {
            var skinId = target.dataset.skinId;
            var skinAccountId = state && state.purchase_user_id;
            var skinSecret = null;
            try { skinSecret = window.localStorage.getItem("sow_account_secret"); } catch (e) {}
            if (!skinId || !skinAccountId || !skinSecret) {
                state.error = "Account setup is required before changing skins.";
                render();
                return;
            }
            target.disabled = true;
            fetch(profileApi(command === "unlock_skin" ? "/store/skins/unlock" : "/store/skins/equip"), {
                method: "POST",
                headers: { "Content-Type": "application/json", "Accept": "application/json" },
                body: JSON.stringify({
                    public_id: skinAccountId,
                    auth_secret: skinSecret,
                    skin_id: skinId
                })
            }).then(function (response) {
                if (!response.ok) throw new Error("skin action failed");
                window.location.reload();
            }).catch(function () {
                state.error = command === "unlock_skin" ? "Skin unlock unavailable." : "Skin equip unavailable.";
                render();
            });
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
                mobileHeroesOpen = false;
            }
            return;
        }
        var payload = {};
        if (target.dataset.lobbyId) payload.lobby_id = Number(target.dataset.lobbyId);
        if (target.dataset.playerId) payload.target_player_id = Number(target.dataset.playerId);
        send(command, payload);
    });

    root.addEventListener("keydown", function (event) {
        if (event.key === "Escape" && profileMatchDetail) {
            profileMatchDetail = null;
            render();
            return;
        }
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
        if (form.dataset.form === "profile-search") {
            event.preventDefault();
            var query = form.elements.q.value.trim();
            fetch(profileApi("/profiles/search?q=" + encodeURIComponent(query) + "&limit=20"), {
                headers: { "Accept": "application/json" }
            }).then(function (response) {
                if (!response.ok) throw new Error("profile search failed");
                return response.json();
            }).then(function (data) {
                profileSearchResults = Array.isArray(data.items) ? data.items : [];
                profileSearchResults = profileSearchResults.filter(function (s) {
                    return !isBlockedPublicId(s && s.public_id);
                });
                profileError = "";
            }).catch(function () {
                profileSearchResults = [];
                profileError = "Profile search unavailable.";
            }).finally(function () {
                render();
            });
            return;
        }
        if (form.dataset.form === "report") {
            event.preventDefault();
            if (reportBusy) return;
            var repCreds = selfCreds();
            var reason = form.elements.reason ? form.elements.reason.value : "";
            var details = form.elements.details ? form.elements.details.value.trim() : "";
            if (!repCreds || !reportTarget) {
                reportOpen = false;
                render();
                return;
            }
            if (reason === "other" && !details) {
                profileError = "Details are required for reason Other.";
                render();
                return;
            }
            reportBusy = true;
            render();
            fetch(profileApi("/profile/anonymous/report"), {
                method: "POST",
                headers: { "Content-Type": "application/json", "Accept": "application/json" },
                body: JSON.stringify({
                    account_id: repCreds.account_id,
                    auth_secret: repCreds.auth_secret,
                    reported_public_id: reportTarget,
                    reason: reason,
                    details: details || null
                })
            }).then(function (response) {
                if (!response.ok) throw new Error("report failed");
                return response.json();
            }).then(function () {
                reportSent = true;
                reportOpen = false;
                try {
                    var list = window.SOW_BLOCKED_IDS || [];
                    if (list.indexOf(reportTarget) === -1) list.push(reportTarget);
                    window.SOW_BLOCKED_IDS = list;
                } catch (e) {}
            }).catch(function () {
                profileError = "Report unavailable. Try again or email hello@shadowsofwar.io.";
            }).finally(function () {
                reportBusy = false;
                render();
            });
            return;
        }
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
            }
        }
        if (input.dataset && input.dataset.role === "heroes-search") {
            heroesSearchQuery = input.value;
            var heroesRosterContainer = root.querySelector("[data-heroes-roster]");
            if (heroesRosterContainer) {
                var activeHeroesId = tempSelectedLeader || (state && state.selected_leader) || "Caesar";
                heroesRosterContainer.innerHTML = renderHeroesRoster(activeHeroesId);
                var heroesRosterPanel = root.querySelector(".sow-heroes__roster");
                if (heroesRosterPanel) heroesRosterPanel.scrollTop = 0;
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
        if (input.dataset && input.dataset.role === "heroes-region") {
            heroesRegionFilter = input.value || "all";
            var heroesRosterContainer = root.querySelector("[data-heroes-roster]");
            if (heroesRosterContainer) {
                var activeHeroesId = tempSelectedLeader || (state && state.selected_leader) || "Caesar";
                heroesRosterContainer.innerHTML = renderHeroesRoster(activeHeroesId);
                var heroesRosterPanel = root.querySelector(".sow-heroes__roster");
                if (heroesRosterPanel) heroesRosterPanel.scrollTop = 0;
            }
            return;
        }
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
