/**
 * CrazyGames / Poki portal hooks for WASM builds.
 * Loaded before the Rust module; no-op when SDKs are absent (self-hosted shadowsofwar.io).
 */
(function () {
  function sdk(name) {
    try {
      return window[name];
    } catch {
      return undefined;
    }
  }

  window.SOW_portalGameplayStart = function () {
    if (typeof PokiSDK !== "undefined" && PokiSDK.gameplayStart) {
      PokiSDK.gameplayStart();
    }
    if (window.CrazyGames && window.CrazyGames.SDK && window.CrazyGames.SDK.game) {
      window.CrazyGames.SDK.game.play();
    }
  };

  window.SOW_portalGameplayStop = function () {
    if (typeof PokiSDK !== "undefined" && PokiSDK.gameplayStop) {
      PokiSDK.gameplayStop();
    }
    if (window.CrazyGames && window.CrazyGames.SDK && window.CrazyGames.SDK.game) {
      window.CrazyGames.SDK.game.pause();
    }
  };

  window.SOW_portalLoadStart = function () {
    if (typeof PokiSDK !== "undefined" && PokiSDK.loadStart) {
      PokiSDK.loadStart();
    }
  };

  window.SOW_portalLoadStop = function () {
    if (typeof PokiSDK !== "undefined" && PokiSDK.loadStop) {
      PokiSDK.loadStop();
    }
    if (window.CrazyGames && window.CrazyGames.SDK && window.CrazyGames.SDK.game) {
      window.CrazyGames.SDK.game.loadingStop();
    }
  };

  window.SOW_portalLoadStart();
})();
