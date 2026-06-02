/**
 * Site boot — lazy-load game WASM on Play click (shadowsofwar.io / only).
 * Reads build filenames from #sow-game-manifest (SSR-injected JSON).
 */
(function () {
    'use strict';

    var booting = false;

    function manifest() {
        var el = document.getElementById('sow-game-manifest');
        if (!el || !el.textContent) return null;
        try {
            return JSON.parse(el.textContent);
        } catch (_) {
            return null;
        }
    }

    function loadScript(src) {
        return new Promise(function (resolve, reject) {
            var s = document.createElement('script');
            s.src = src;
            s.onload = resolve;
            s.onerror = reject;
            document.head.appendChild(s);
        });
    }

    function prefetchGameJs() {
        var m = manifest();
        if (!m || !m.js || document.querySelector('link[data-sow-prefetch-js]')) return;
        var link = document.createElement('link');
        link.rel = 'prefetch';
        link.as = 'script';
        link.href = '/play/' + m.js;
        link.setAttribute('data-sow-prefetch-js', '');
        document.head.appendChild(link);
    }

    async function bootGame() {
        if (booting) return;
        var m = manifest();
        if (!m || !m.js || !m.wasm) {
            console.error('SOW: game manifest missing — run ./scripts/sow.sh package first');
            return;
        }
        booting = true;

        var stage = document.getElementById('game-stage');
        if (!stage) return;

        var overlay = document.getElementById('game-play-overlay');
        if (overlay) overlay.hidden = true;
        stage.classList.add('game-stage--active');

        if (!document.getElementById('blade')) {
            var canvas = document.createElement('canvas');
            canvas.id = 'blade';
            canvas.setAttribute('oncontextmenu', 'return false;');
            stage.appendChild(canvas);
        }

        if (!document.getElementById('web-loader')) {
            var loader = document.createElement('div');
            loader.id = 'web-loader';
            loader.setAttribute('aria-live', 'polite');
            loader.setAttribute('aria-busy', 'true');
            stage.appendChild(loader);
        }

        if (!document.getElementById('version')) {
            var ver = document.createElement('div');
            ver.id = 'version';
            ver.textContent = 'v' + (m.version || '');
            stage.appendChild(ver);
        }

        window.SOW_BUILD_TS = m.build_ts;
        window.SOW_LOADER_BASE = '/play/';
        window.SOW_MAPS_URL = window.SOW_MAPS_URL || 'https://shadowsofwar.io/maps';
        window.SOW_ASSETS_URL = window.SOW_ASSETS_URL || 'https://shadowsofwar.io/assets';

        await loadScript('/play/loader.js');
        if (typeof window.SOW_initWebLoader === 'function') {
            window.SOW_initWebLoader();
        }

        if ('serviceWorker' in navigator) {
            navigator.serviceWorker.register('/sw.js', { scope: '/' }).catch(function (err) {
                console.warn('Service worker registration failed:', err);
            });
        }

        try {
            var mod = await import('/play/' + m.js);
            await mod.default({ module_or_path: '/play/' + m.wasm });
        } catch (e) {
            if (
                e.message !==
                "Using exceptions for control flow, don't mind me. This isn't actually an error!"
            ) {
                booting = false;
                throw e;
            }
        }
    }

    document.addEventListener('DOMContentLoaded', function () {
        var btn = document.getElementById('game-play-btn');
        if (!btn) return;
        btn.addEventListener('click', function (e) {
            e.preventDefault();
            bootGame();
        });
        btn.addEventListener('mouseenter', prefetchGameJs);
        btn.addEventListener('focus', prefetchGameJs);
    });
})();
