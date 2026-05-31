/**
 * Web boot loader — splash + progress bar until the game calls hideWebLoader().
 */
(function () {
    'use strict';

    const FADEOUT_MS = 250;
    const BAR_ASPECT = 2064 / 512;
    const MOBILE_BREAKPOINT = 600;

    const LAYOUT = {
        portrait: {
            barWidthRatio: 0.76,
            barSidePadPx: 32,
            bottomRatio: 0.06,
            bottomMinPx: 24,
            textSizePx: 11,
            textNudgePx: -1,
        },
        landscape: {
            barMaxWidth: 480,
            barWidthRatio: 0.42,
            bottomRatio: 0.07,
            bottomMinPx: 32,
            textSizePx: 13,
            textNudgePx: -2,
        },
    };

    const ASSETS = {
        splashDesktop: './assets/ui/sow-splash-desktop.webp',
        splashMobile: './assets/ui/sow-splash-mobile.webp',
        loaderEmpty: './assets/ui/loader_empty.webp',
        loaderFull: './assets/ui/loader_full.webp',
    };

    function assetUrl(path) {
        const v = window.SOW_BUILD_TS;
        if (v && v !== '__BUILD_TS__') {
            return path + '?v=' + encodeURIComponent(v);
        }
        return path;
    }

    let root = null;
    let splashBg = null;
    let barFill = null;
    let barFull = null;
    let loaderText = null;
    let finishing = false;
    let rafId = 0;

    function isMobile() {
        return window.innerWidth < MOBILE_BREAKPOINT;
    }

    function layoutMode() {
        if (isMobile() && window.innerWidth <= window.innerHeight) {
            return 'portrait';
        }
        return 'landscape';
    }

    function layoutConfig(mode) {
        return LAYOUT[mode];
    }

    function barWidthFor(mode, screenW) {
        const cfg = layoutConfig(mode);
        if (mode === 'portrait') {
            const inner = Math.max(160, screenW - cfg.barSidePadPx * 2);
            return inner * cfg.barWidthRatio;
        }
        return Math.min(cfg.barMaxWidth, screenW * cfg.barWidthRatio);
    }

    function splashSrc() {
        return assetUrl(isMobile() ? ASSETS.splashMobile : ASSETS.splashDesktop);
    }

    function layout() {
        if (!root) return;
        const mode = layoutMode();
        const cfg = layoutConfig(mode);
        const screenW = window.innerWidth;
        const screenH = window.innerHeight;
        const barWidth = barWidthFor(mode, screenW);
        const barHeight = barWidth / BAR_ASPECT;
        const bottomPadding = Math.max(screenH * cfg.bottomRatio, cfg.bottomMinPx);

        root.dataset.layout = mode;

        const barWrapEl = document.getElementById('loader-bar-wrap');
        if (barWrapEl) {
            barWrapEl.style.width = barWidth + 'px';
            barWrapEl.style.height = barHeight + 'px';
            barWrapEl.style.bottom = bottomPadding + 'px';
        }
        if (barFull) {
            barFull.style.width = barWidth + 'px';
        }
        if (loaderText) {
            loaderText.style.fontSize = cfg.textSizePx + 'px';
            loaderText.style.marginTop = cfg.textNudgePx + 'px';
        }
        if (splashBg && splashBg.src !== new URL(splashSrc(), window.location.href).href) {
            splashBg.src = splashSrc();
        }
    }

    function stopProgress() {
        if (rafId) {
            cancelAnimationFrame(rafId);
            rafId = 0;
        }
    }

    /** Slow creep toward 88% while WASM loads; never claims done until hideWebLoader. */
    function startProgress() {
        if (!barFill) return;
        const t0 = performance.now();
        const durationMs = 12000;

        function tick(now) {
            if (!barFill || finishing) return;
            const t = Math.min(1, (now - t0) / durationMs);
            const eased = 1 - Math.pow(1 - t, 2);
            barFill.style.width = (eased * 88).toFixed(1) + '%';
            rafId = requestAnimationFrame(tick);
        }

        barFill.style.transition = 'none';
        barFill.style.width = '0%';
        rafId = requestAnimationFrame(tick);
    }

    function buildDom() {
        root = document.getElementById('web-loader');
        if (!root) {
            root = document.createElement('div');
            root.id = 'web-loader';
            document.body.appendChild(root);
        }

        root.innerHTML = `
            <img id="splash-bg" class="splash-bg" alt="" decoding="async" fetchpriority="high" src="${splashSrc()}">
            <div id="loader-bar-wrap" class="loader-bar-wrap">
                <img id="loader-bar-empty" class="loader-bar-empty" alt="" decoding="async" fetchpriority="high" src="${assetUrl(ASSETS.loaderEmpty)}">
                <div id="loader-bar-fill" class="loader-bar-fill">
                    <img id="loader-bar-full" class="loader-bar-full" alt="" decoding="async" fetchpriority="low" src="${assetUrl(ASSETS.loaderFull)}">
                </div>
                <p id="loader-text" class="loader-text">Loading...</p>
            </div>
        `;

        splashBg = document.getElementById('splash-bg');
        barFill = document.getElementById('loader-bar-fill');
        barFull = document.getElementById('loader-bar-full');
        loaderText = document.getElementById('loader-text');

        layout();
        window.addEventListener('resize', layout);
        window.addEventListener('orientationchange', layout);
        startProgress();
    }

    function teardown() {
        stopProgress();
        window.removeEventListener('resize', layout);
        window.removeEventListener('orientationchange', layout);
        if (root && root.parentNode) {
            root.parentNode.removeChild(root);
        }
        root = null;
        splashBg = null;
        barFill = null;
        barFull = null;
        loaderText = null;
    }

    function finish() {
        if (!root || finishing) return;
        finishing = true;
        stopProgress();
        root.style.pointerEvents = 'none';

        if (barFill) {
            barFill.style.transition = 'width 150ms ease-out';
            barFill.style.width = '100%';
        }

        root.style.transition = `opacity ${FADEOUT_MS}ms ease-out`;
        setTimeout(() => {
            if (!root) return;
            root.style.opacity = '0';
            setTimeout(teardown, FADEOUT_MS + 30);
        }, 160);
    }

    function init() {
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', init);
            return;
        }
        buildDom();
    }

    window.hideWebLoader = finish;
    init();
})();
