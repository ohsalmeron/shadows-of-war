/**
 * CrazyGames / Poki portal hooks. No-op when SDKs are absent (self-hosted).
 */
(function () {
  function crazyGameApi() {
    return window.CrazyGames && window.CrazyGames.SDK && window.CrazyGames.SDK.game;
  }

  /**
   * Runtime detection (mirrors OpenFront's CrazyGamesSDK.isOnCrazyGames): only treat
   * this as a CrazyGames embed when we are iframed inside a crazygames host. Keeps the
   * shim a no-op on the plain website / PTR even if the SDK script is somehow present.
   */
  function isOnCrazyGames() {
    if (window.SOW_PORTAL === "crazygames") {
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

  window.SOW_isOnCrazyGames = isOnCrazyGames;

  window.SOW_initPortalSdk = async function () {
    if (!isOnCrazyGames()) {
      return;
    }
    if (window.CrazyGames && window.CrazyGames.SDK && window.CrazyGames.SDK.init) {
      try {
        await window.CrazyGames.SDK.init();
      } catch (e) {
        console.warn("CrazyGames SDK init failed:", e);
      }
    }
  };

  window.SOW_portalGameplayStart = function () {
    if (typeof PokiSDK !== "undefined" && PokiSDK.gameplayStart) {
      PokiSDK.gameplayStart();
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

  window.SOW_portalGameplayStop = function () {
    if (typeof PokiSDK !== "undefined" && PokiSDK.gameplayStop) {
      PokiSDK.gameplayStop();
    }
    const game = crazyGameApi();
    if (game) {
      if (game.gameplayStop) {
        game.gameplayStop();
      } else if (game.pause) {
        game.pause();
      }
    }
  };

  window.SOW_portalLoadStart = function () {
    if (typeof PokiSDK !== "undefined" && PokiSDK.loadStart) {
      PokiSDK.loadStart();
    }
    const game = crazyGameApi();
    if (game && game.sdkGameLoadingStart) {
      game.sdkGameLoadingStart();
    }
  };

  window.SOW_portalLoadStop = function () {
    if (typeof PokiSDK !== "undefined" && PokiSDK.loadStop) {
      PokiSDK.loadStop();
    }
    const game = crazyGameApi();
    if (game) {
      if (game.sdkGameLoadingStop) {
        game.sdkGameLoadingStop();
      } else if (game.loadingStop) {
        game.loadingStop();
      }
    }
  };

  window.SOW_portalLoadStart();
})();
