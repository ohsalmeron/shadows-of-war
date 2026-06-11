/**
 * CrazyGames / Poki portal hooks. No-op when SDKs are absent (self-hosted).
 */
(function () {
  function crazyGameApi() {
    return window.CrazyGames && window.CrazyGames.SDK && window.CrazyGames.SDK.game;
  }

  function isCrazyGamesHost() {
    var h = window.location.hostname || "";
    if (/crazygames\.com$/i.test(h) || /dev-crazygames\.com$/i.test(h)) {
      return true;
    }
    var ref = document.referrer || "";
    return /crazygames/i.test(ref);
  }

  function isSiteEmbed() {
    return window.SOW_PORTAL === "site";
  }

  function isOnCrazyGames() {
    if (isSiteEmbed()) {
      return false;
    }
    if (window.SOW_PORTAL === "crazygames") {
      return true;
    }
    if (isCrazyGamesHost()) {
      return true;
    }
    try {
      if (window.self !== window.top) {
        return (window.top.location.hostname || "").includes("crazygames");
      }
      return false;
    } catch (e) {
      if (document.referrer && document.referrer.includes("crazygames")) {
        return true;
      }
      return window.self !== window.top;
    }
  }

  function crazyGamesSdkReady() {
    var sdk = window.CrazyGames && window.CrazyGames.SDK;
    if (!sdk) {
      return false;
    }
    var env = sdk.environment;
    return env === "local" || env === "crazygames";
  }

  function isOnPoki() {
    if (isSiteEmbed()) {
      return false;
    }
    if (window.SOW_PORTAL === "poki") {
      return true;
    }
    var h = window.location.hostname || "";
    if (/poki\.com$/i.test(h) || /poki-gdn\.com$/i.test(h) || /poki\.io$/i.test(h)) {
      return true;
    }
    var ref = document.referrer || "";
    return /poki\.com/i.test(ref) || /poki-gdn\.com/i.test(ref);
  }

  function portalAdPause() {
    window.SOW_adPlaying = true;
    document.querySelectorAll("audio,video").forEach(function (el) {
      el.dataset.sowWasPaused = el.paused ? "1" : "0";
      el.pause();
      el.muted = true;
    });
  }

  function portalAdResume() {
    window.SOW_adPlaying = false;
    document.querySelectorAll("audio,video").forEach(function (el) {
      el.muted = false;
      if (el.dataset.sowWasPaused !== "1") {
        el.play().catch(function () {});
      }
      delete el.dataset.sowWasPaused;
    });
  }

  function parseInviteLobbyId(inviteParams) {
    if (!inviteParams || typeof inviteParams !== "object") {
      return null;
    }
    var raw = inviteParams.lobbyId;
    if (raw === undefined || raw === null || raw === "") {
      return null;
    }
    var id = parseInt(String(raw), 10);
    return isNaN(id) ? null : id;
  }

  function queueInviteLobbyId(lobbyId) {
    if (lobbyId !== null && lobbyId !== undefined) {
      window.SOW_PENDING_INVITE_LOBBY_ID = lobbyId;
    }
  }

  function installJoinRoomListener() {
    if (!crazyGamesSdkReady()) {
      return;
    }
    var game = crazyGameApi();
    if (!game || !game.addJoinRoomListener) {
      return;
    }
    game.addJoinRoomListener(function (inviteParams) {
      var lobbyId = parseInviteLobbyId(inviteParams);
      if (lobbyId !== null) {
        console.log("CrazyGames join room listener: lobby", lobbyId);
        queueInviteLobbyId(lobbyId);
      }
    });
  }

  function applyGameSettings(settings) {
    if (!settings) {
      return;
    }
    window.SOW_PORTAL_MUTE_AUDIO = !!settings.muteAudio;
    if (settings.muteAudio) {
      window.SOW_portalMuteGameAudio();
    }
    window.SOW_DISABLE_CHAT = !!settings.disableChat;
  }

  var SOW_PROGRESS_KEY = "sow_player_progress";

  window.SOW_PLATFORM_IDENTITY = null;
  window.SOW_PENDING_INVITE_LOBBY_ID = null;
  window.SOW_HOST_PRIVATE_PENDING = false;
  window.SOW_PORTAL_MUTE_AUDIO = false;
  window.SOW_DISABLE_CHAT = false;
  window.SOW_PORTAL_PROGRESS_JSON = null;

  window.SOW_isSiteEmbed = isSiteEmbed;
  window.SOW_isOnCrazyGames = isOnCrazyGames;
  window.SOW_isOnPoki = isOnPoki;
  window.SOW_portalAdPause = portalAdPause;
  window.SOW_portalAdResume = portalAdResume;

  window.SOW_portalMuteGameAudio = function () {
    document.querySelectorAll("audio,video").forEach(function (el) {
      el.muted = true;
    });
  };

  window.SOW_portalUnmuteGameAudio = function () {
    if (window.SOW_PORTAL_MUTE_AUDIO || window.SOW_adPlaying) {
      return;
    }
    document.querySelectorAll("audio,video").forEach(function (el) {
      el.muted = false;
    });
  };

  window.SOW_portalUpdateRoom = function (jsonStr) {
    if (!crazyGamesSdkReady()) {
      return;
    }
    var game = crazyGameApi();
    if (!game || !game.updateRoom) {
      return;
    }
    try {
      var payload = JSON.parse(jsonStr);
      game.updateRoom(payload);
    } catch (e) {
      console.warn("SOW_portalUpdateRoom parse failed:", e);
    }
  };

  window.SOW_portalLeftRoom = function () {
    if (!crazyGamesSdkReady()) {
      return;
    }
    var game = crazyGameApi();
    if (game && game.leftRoom) {
      game.leftRoom();
    }
  };

  window.SOW_portalInviteLink = function (jsonStr) {
    if (!crazyGamesSdkReady()) {
      return null;
    }
    var game = crazyGameApi();
    if (!game || !game.inviteLink) {
      return null;
    }
    try {
      var payload = JSON.parse(jsonStr);
      return game.inviteLink(payload) || null;
    } catch (e) {
      console.warn("SOW_portalInviteLink failed:", e);
      return null;
    }
  };

  window.SOW_portalClearHostPrivatePending = function () {
    window.SOW_HOST_PRIVATE_PENDING = false;
  };

  window.SOW_portalClearPendingInvite = function () {
    window.SOW_PENDING_INVITE_LOBBY_ID = null;
  };

  window.SOW_AUTH_CHANGED = false;
  window.SOW_portalClearAuthChanged = function () {
    window.SOW_AUTH_CHANGED = false;
  };

  async function crazyGamesIdentityFromUser(user) {
    if (!user || !user.username) {
      return null;
    }
    var token = null;
    try {
      if (window.CrazyGames.SDK.user.getUserToken) {
        token = await window.CrazyGames.SDK.user.getUserToken();
      }
    } catch (e) {
      console.warn("CrazyGames getUserToken failed:", e);
    }
    return {
      provider: "crazygames",
      displayName: user.username,
      externalId: user.userId || user.id || null,
      avatarUrl: user.profilePictureUrl || null,
      nameLocked: true,
      token: token,
    };
  }

  window.SOW_portalSaveData = function (jsonStr) {
    if (!crazyGamesSdkReady()) {
      return;
    }
    var data = window.CrazyGames.SDK.data;
    if (!data || !data.setItem) {
      return;
    }
    try {
      var payload = JSON.parse(jsonStr);
      data.setItem(payload.key, payload.value);
    } catch (e) {
      console.warn("SOW_portalSaveData failed:", e);
    }
  };

  window.SOW_portalLoadData = async function (key) {
    if (!crazyGamesSdkReady()) {
      return null;
    }
    var data = window.CrazyGames.SDK.data;
    if (!data || !data.getItem) {
      return null;
    }
    try {
      return await data.getItem(key);
    } catch (e) {
      console.warn("SOW_portalLoadData failed:", e);
      return null;
    }
  };

  window.SOW_portalShowAuthPrompt = async function () {
    if (!crazyGamesSdkReady() || !window.CrazyGames.SDK.user || !window.CrazyGames.SDK.user.showAuthPrompt) {
      return;
    }
    try {
      var user = await window.CrazyGames.SDK.user.showAuthPrompt();
      var identity = await crazyGamesIdentityFromUser(user);
      if (identity) {
        window.SOW_PLATFORM_IDENTITY = identity;
        window.SOW_AUTH_CHANGED = true;
        console.log("Auth prompt successful login:", user.username);
      }
    } catch (e) {
      console.warn("Auth prompt failed or cancelled:", e);
    }
  };

  window.SOW_portalConsumeBootIntent = function () {
    window.SOW_PENDING_INVITE_LOBBY_ID = null;
    window.SOW_HOST_PRIVATE_PENDING = false;
    if (!crazyGamesSdkReady()) {
      return;
    }
    var game = crazyGameApi();
    if (!game) {
      return;
    }
    var coldInvite = parseInviteLobbyId(game.inviteParams);
    if (coldInvite !== null) {
      console.log("CrazyGames cold-start invite lobby:", coldInvite);
      queueInviteLobbyId(coldInvite);
      return;
    }
    if (game.isInstantMultiplayer) {
      console.log("CrazyGames instant multiplayer: host private lobby");
      window.SOW_HOST_PRIVATE_PENDING = true;
    }
  };

  window.SOW_initPortalSdk = async function () {
    if (!isOnCrazyGames()) {
      return;
    }
    if (!(window.CrazyGames && window.CrazyGames.SDK && window.CrazyGames.SDK.init)) {
      console.warn("CrazyGames SDK script not loaded");
      return;
    }
    try {
      await window.CrazyGames.SDK.init();
      var env = window.CrazyGames.SDK.environment;
      console.log("CrazyGames SDK init OK (env=" + env + ")");

      if (crazyGamesSdkReady() && window.CrazyGames.SDK.user) {
        try {
          if (window.CrazyGames.SDK.user.isUserAccountAvailable) {
            var user = await window.CrazyGames.SDK.user.getUser();
            var identity = await crazyGamesIdentityFromUser(user);
            if (identity) {
              window.SOW_PLATFORM_IDENTITY = identity;
              window.SOW_AUTH_CHANGED = true;
              console.log("CrazyGames user:", user.username);
            }
          }
          if (window.CrazyGames.SDK.user.addAuthListener) {
            window.CrazyGames.SDK.user.addAuthListener(async function (u) {
              var identity = await crazyGamesIdentityFromUser(u);
              if (identity) {
                window.SOW_PLATFORM_IDENTITY = identity;
                window.SOW_AUTH_CHANGED = true;
                console.log("CrazyGames user changed via AuthListener:", u.username);
              }
            });
          }
        } catch (e) {
          console.warn("CrazyGames getUser failed:", e);
        }
      }

      if (crazyGamesSdkReady() && window.CrazyGames.SDK.ad && window.CrazyGames.SDK.ad.hasAdblock) {
        try {
          window.SOW_hasAdblock = await window.CrazyGames.SDK.ad.hasAdblock();
          console.log("CrazyGames adblock check:", window.SOW_hasAdblock);
        } catch (e) {
          console.warn("CrazyGames hasAdblock failed:", e);
        }
      }

      if (crazyGamesSdkReady()) {
        var game = crazyGameApi();
        if (game && game.settings) {
          applyGameSettings(game.settings);
        }
        if (game && game.addSettingsChangeListener) {
          game.addSettingsChangeListener(applyGameSettings);
        }
        installJoinRoomListener();
        window.SOW_portalConsumeBootIntent();
      }

      if (crazyGamesSdkReady() && window.CrazyGames.SDK.data) {
        try {
          var saved = await window.SOW_portalLoadData(SOW_PROGRESS_KEY);
          if (saved) {
            window.SOW_PORTAL_PROGRESS_JSON =
              typeof saved === "string" ? saved : JSON.stringify(saved);
            console.log("CrazyGames progress loaded");
          }
        } catch (e) {
          console.warn("CrazyGames progress load failed:", e);
        }
      }
    } catch (e) {
      console.warn("CrazyGames SDK init failed:", e);
    }
  };

  window.SOW_portalGameplayStart = function () {
    if (isSiteEmbed() || window.SOW_adPlaying) {
      return;
    }
    if (typeof PokiSDK !== "undefined" && PokiSDK.gameplayStart) {
      PokiSDK.gameplayStart();
    }
    if (!crazyGamesSdkReady()) {
      return;
    }
    const game = crazyGameApi();
    if (game) {
      if (game.gameplayStart) {
        game.gameplayStart();
      } else if (game.play) {
        game.play();
      }
    }
  };

  function portalAdsEnabled() {
    return window.SOW_ENABLE_PORTAL_ADS === true;
  }

  function requestCrazyGamesMidgameAd() {
    if (!portalAdsEnabled() || !crazyGamesSdkReady() || window.SOW_adPlaying) {
      return;
    }
    var ad = window.CrazyGames.SDK.ad;
    if (!ad || !ad.requestAd) {
      return;
    }
    var callbacks = {
      adStarted: function () {
        portalAdPause();
      },
      adFinished: function () {
        portalAdResume();
      },
      adError: function (error) {
        console.warn("CrazyGames midgame ad error:", error);
        portalAdResume();
      },
    };
    try {
      ad.requestAd("midgame", callbacks);
    } catch (e) {
      console.warn("CrazyGames requestAd failed:", e);
    }
  }

  window.SOW_portalGameplayStop = function () {
    if (typeof PokiSDK !== "undefined" && PokiSDK.gameplayStop) {
      PokiSDK.gameplayStop();
    }
    if (crazyGamesSdkReady()) {
      const game = crazyGameApi();
      if (game) {
        if (game.gameplayStop) {
          game.gameplayStop();
        } else if (game.pause) {
          game.pause();
        }
      }
      requestCrazyGamesMidgameAd();
    }
  };

  window.SOW_portalLoadStart = function () {
    if (isSiteEmbed()) {
      return;
    }
    if (typeof PokiSDK !== "undefined" && PokiSDK.loadStart) {
      PokiSDK.loadStart();
    }
    if (!crazyGamesSdkReady()) {
      return;
    }
    const game = crazyGameApi();
    if (!game) {
      return;
    }
    if (game.loadingStart) {
      game.loadingStart();
    } else if (game.sdkGameLoadingStart) {
      game.sdkGameLoadingStart();
    }
  };

  window.SOW_portalLoadStop = function () {
    if (typeof PokiSDK !== "undefined" && PokiSDK.loadStop) {
      PokiSDK.loadStop();
    }
    if (!crazyGamesSdkReady()) {
      return;
    }
    const game = crazyGameApi();
    if (!game) {
      return;
    }
    if (game.loadingStop) {
      game.loadingStop();
    } else if (game.sdkGameLoadingStop) {
      game.sdkGameLoadingStop();
    }
  };

  window.SOW_portalHappytime = function () {
    if (!crazyGamesSdkReady()) {
      return;
    }
    const game = crazyGameApi();
    if (game && game.happytime) {
      try {
        game.happytime();
      } catch (e) {
        console.warn("CrazyGames happytime trigger failed:", e);
      }
    }
  };

  document.addEventListener(
    "keydown",
    function (e) {
      if (isSiteEmbed() || !document.fullscreenElement) {
        return;
      }
      if ((e.ctrlKey || e.metaKey) && (e.key === "w" || e.key === "W")) {
        e.preventDefault();
      }
    },
    true
  );
})();
