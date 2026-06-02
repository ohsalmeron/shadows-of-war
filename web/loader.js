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

    function assetBase() {
        const b = window.SOW_LOADER_BASE;
        if (!b) return './';
        return b.endsWith('/') ? b : b + '/';
    }

    function assetPath(file) {
        return assetBase() + 'assets/ui/' + file;
    }

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
        return assetUrl(
            isMobile() ? assetPath('sow-splash-mobile.webp') : assetPath('sow-splash-desktop.webp')
        );
    }

    function stageSize() {
        const stage = document.getElementById('game-stage');
        if (stage) {
            const r = stage.getBoundingClientRect();
            return { w: r.width, h: r.height };
        }
        return { w: window.innerWidth, h: window.innerHeight };
    }

    function layout() {
        if (!root) return;
        const mode = layoutMode();
        const cfg = layoutConfig(mode);
        const { w: screenW, h: screenH } = stageSize();
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

    function loaderHost() {
        return document.getElementById('game-stage') || document.body;
    }

    function buildDom() {
        root = document.getElementById('web-loader');
        if (!root) {
            root = document.createElement('div');
            root.id = 'web-loader';
            loaderHost().appendChild(root);
        }

        root.innerHTML = `
            <img id="splash-bg" class="splash-bg" alt="" decoding="async" fetchpriority="high" src="${splashSrc()}">
            <img id="splash-desktop" alt="" decoding="async" fetchpriority="high" src="${assetUrl(assetPath('sow-splash-desktop.webp'))}" hidden>
            <img id="splash-mobile" alt="" decoding="async" fetchpriority="high" src="${assetUrl(assetPath('sow-splash-mobile.webp'))}" hidden>
            <div id="loader-bar-wrap" class="loader-bar-wrap">
                <img id="loader-bar-empty" class="loader-bar-empty" alt="" decoding="async" fetchpriority="high" src="${assetUrl(assetPath('loader_empty.webp'))}">
                <div id="loader-bar-fill" class="loader-bar-fill">
                    <img id="loader-bar-full" class="loader-bar-full" alt="" decoding="async" fetchpriority="low" src="${assetUrl(assetPath('loader_full.webp'))}">
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
        wireTextureExportOnLoad(root);
        startProgress();
    }

    function wireTextureExportOnLoad(loaderRoot) {
        const ids = [
            'splash-bg',
            'splash-desktop',
            'splash-mobile',
            'loader-bar-empty',
            'loader-bar-full',
        ];
        for (const id of ids) {
            const img = loaderRoot.querySelector('#' + id);
            if (!img) continue;
            img.addEventListener('load', exportWebLoaderTextures);
            if (img.complete) {
                exportWebLoaderTextures();
            }
        }
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

    function rgbaFromImg(img) {
        if (!img || !img.complete || !img.naturalWidth || !img.naturalHeight) {
            return null;
        }
        const w = img.naturalWidth;
        const h = img.naturalHeight;
        const canvas = document.createElement('canvas');
        canvas.width = w;
        canvas.height = h;
        const ctx = canvas.getContext('2d', { willReadFrequently: true });
        ctx.drawImage(img, 0, 0);
        const data = ctx.getImageData(0, 0, w, h);
        return {
            width: w,
            height: h,
            rgba: new Uint8Array(data.data.buffer, data.data.byteOffset, data.data.byteLength),
        };
    }

    /** Snapshot boot loader images for egui enter/exit splashes (called from WASM before fade). */
    function exportWebLoaderTextures() {
        if (!root) {
            return window.__SOW_LOADER_TEXTURES__ || null;
        }

        const desktopEl = document.getElementById('splash-desktop');
        const mobileEl = document.getElementById('splash-mobile');
        const emptyEl = document.getElementById('loader-bar-empty');
        const fullEl = document.getElementById('loader-bar-full');

        let splashDesktop = rgbaFromImg(desktopEl);
        let splashMobile = rgbaFromImg(mobileEl);

        // Fallback: visible splash-bg may be decoded before hidden preload siblings.
        if (!splashDesktop && splashBg && splashBg.src.includes('sow-splash-desktop')) {
            splashDesktop = rgbaFromImg(splashBg);
        }
        if (!splashMobile && splashBg && splashBg.src.includes('sow-splash-mobile')) {
            splashMobile = rgbaFromImg(splashBg);
        }

        const out = window.__SOW_LOADER_TEXTURES__ || {};
        const loaderEmpty = rgbaFromImg(emptyEl);
        const loaderFull = rgbaFromImg(fullEl);

        if (splashDesktop) out.splash_desktop = splashDesktop;
        if (splashMobile) out.splash_mobile = splashMobile;
        if (loaderEmpty) out.loader_empty = loaderEmpty;
        if (loaderFull) out.loader_full = loaderFull;

        if (Object.keys(out).length === 0) {
            return null;
        }

        window.__SOW_LOADER_TEXTURES__ = out;
        return out;
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

    function initWebLoader() {
        buildDom();
    }

    window.hideWebLoader = finish;
    window.exportWebLoaderTextures = exportWebLoaderTextures;
    window.SOW_initWebLoader = initWebLoader;

    // CrazyGames /play/ shell: #web-loader exists in HTML and auto-starts.
    if (document.getElementById('web-loader')) {
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', initWebLoader);
        } else {
            initWebLoader();
        }
    }
})();
