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

  /**
   * True when embedded on CrazyGames (iframe, referrer, or package boot var).
   */
  function isOnCrazyGames() {
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

  window.SOW_isOnCrazyGames = isOnCrazyGames;
  window.SOW_isOnPoki = isOnPoki;
  window.SOW_portalAdPause = portalAdPause;
  window.SOW_portalAdResume = portalAdResume;

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
      if (crazyGamesSdkReady() && window.CrazyGames.SDK.ad && window.CrazyGames.SDK.ad.hasAdblock) {
        try {
          window.SOW_hasAdblock = await window.CrazyGames.SDK.ad.hasAdblock();
          console.log("CrazyGames adblock check:", window.SOW_hasAdblock);
        } catch (e) {
          console.warn("CrazyGames hasAdblock failed:", e);
        }
      }
    } catch (e) {
      console.warn("CrazyGames SDK init failed:", e);
    }
  };

  window.SOW_portalGameplayStart = function () {
    if (window.SOW_adPlaying) {
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

  // Midgame ads are for Full Launch only. Basic Launch forbids ads (see bannersDisabledBasicLaunch).
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

  // CrazyGames: avoid Ctrl/Cmd+W closing the tab while the game is fullscreen.
  document.addEventListener(
    "keydown",
    function (e) {
      if (!document.fullscreenElement) {
        return;
      }
      if ((e.ctrlKey || e.metaKey) && (e.key === "w" || e.key === "W")) {
        e.preventDefault();
      }
    },
    true
  );
})();
