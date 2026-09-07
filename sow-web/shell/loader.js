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

    function assetPathVariants(file) {
        const bootBase = window.SOW_BOOT_UI_BASE;
        if (bootBase && typeof bootBase === 'string') {
            const root = bootBase.replace(/\/$/, '');
            return [root + '/' + file];
        }
        const base = window.SOW_ASSETS_URL;
        if (base && typeof base === 'string') {
            const root = base.replace(/\/$/, '');
            return [root + '/shell/loader/' + file];
        }
        // Strict endpoints: a missing SOW_ASSETS_URL is a packaging bug —
        // fail loudly instead of guessing a CDN URL (guessing once loaded
        // the wrong origin).
        throw new Error('SOW_ASSETS_URL is not set; cannot resolve boot UI asset: ' + file);
    }

    function wireAssetFallback(img, file) {
        const variants = assetPathVariants(file);
        let attempt = 0;
        img.onerror = function () {
            attempt += 1;
            if (attempt < variants.length) {
                setImgSrc(img, assetUrl(variants[attempt]));
            }
        };
    }

    function assetUrl(path) {
        const v = window.SOW_BUILD_TS;
        if (v && v !== '__BUILD_TS__') {
            return path + '?v=' + encodeURIComponent(v);
        }
        return path;
    }

    function isCrossOriginAssetUrl(url) {
        try {
            const resolved = new URL(url, window.location.href);
            return resolved.origin !== window.location.origin;
        } catch {
            return false;
        }
    }

    /** Set src after crossOrigin so CDN boot art can be read in canvas (portal iframe). */
    function setImgSrc(img, url) {
        if (!img || !url) {
            return;
        }
        if (isCrossOriginAssetUrl(url)) {
            img.crossOrigin = 'anonymous';
        } else {
            img.removeAttribute('crossorigin');
        }
        img.src = url;
    }

    let root = null;
    let barFill = null;
    let barFull = null;
    let loaderText = null;
    let finishing = false;
    let rafId = 0;
    let reportedProgress = null;

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

    function stageSize() {
        const vv = window.visualViewport;
        if (vv && vv.width > 0 && vv.height > 0) {
            return {
                w: vv.width,
                h: vv.height,
                top: vv.offsetTop,
                left: vv.offsetLeft,
            };
        }
        return {
            w: window.innerWidth,
            h: window.innerHeight,
            top: 0,
            left: 0,
        };
    }

    function layout() {
        if (!root) return;
        const mode = layoutMode();
        const cfg = layoutConfig(mode);
        const { w: screenW, h: screenH, top, left } = stageSize();

        root.style.top = top + 'px';
        root.style.left = left + 'px';
        root.style.width = screenW + 'px';
        root.style.height = screenH + 'px';
        const barWidth = barWidthFor(mode, screenW);
        const barHeight = barWidth / BAR_ASPECT;
        const bottomPadding = Math.max(screenH * cfg.bottomRatio, cfg.bottomMinPx);

        root.dataset.layout = mode;

        const barWrapEl = document.getElementById('loader-bar-wrap');
        if (barWrapEl) {
            barWrapEl.style.width = barWidth + 'px';
            barWrapEl.style.height = barHeight + 'px';
            barWrapEl.style.bottom = 'max(' + bottomPadding + 'px, var(--sow-sab))';
        }
        if (barFull) {
            barFull.style.width = barWidth + 'px';
        }
        if (loaderText) {
            loaderText.style.fontSize = cfg.textSizePx + 'px';
            loaderText.style.marginTop = cfg.textNudgePx + 'px';
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
        const durationMs = 2500;

        function tick(now) {
            if (!barFill || finishing) return;
            const t = Math.min(1, (now - t0) / durationMs);
            const eased = 1 - Math.pow(1 - t, 2);
            const progress = reportedProgress == null ? eased * 88 : reportedProgress * 100;
            barFill.style.width = Math.min(100, progress).toFixed(1) + '%';
            rafId = requestAnimationFrame(tick);
        }

        barFill.style.transition = 'none';
        barFill.style.width = '0%';
        reportedProgress = null;
        rafId = requestAnimationFrame(tick);
    }

    function buildDom() {
        root = document.getElementById('web-loader');
        if (!root) {
            return;
        }

        const splashBg = document.getElementById('splash-bg');
        barFill = document.getElementById('loader-bar-fill');
        barFull = document.getElementById('loader-bar-full');
        loaderText = document.getElementById('loader-text');

        for (const img of root.querySelectorAll('img[src]')) {
            const src = img.getAttribute('src');
            if (src && !src.startsWith('data:')) {
                setImgSrc(img, src);
            }
        }
        for (const [id, file] of [
            ['splash-bg', isMobile() ? 'sow-splash-mobile.webp' : 'sow-splash-desktop.webp'],
            ['loader-bar-empty', 'loader_empty.webp'],
            ['loader-bar-full', 'loader_full.webp'],
        ]) {
            const img = document.getElementById(id);
            if (img) wireAssetFallback(img, file);
        }

        layout();
        window.addEventListener('resize', layout);
        window.addEventListener('orientationchange', layout);
        if (window.visualViewport) {
            window.visualViewport.addEventListener('resize', layout);
            window.visualViewport.addEventListener('scroll', layout);
        }
        startProgress();
    }

    function teardown(expectedRoot) {
        if (expectedRoot && root !== expectedRoot) return;
        stopProgress();
        window.removeEventListener('resize', layout);
        window.removeEventListener('orientationchange', layout);
        if (window.visualViewport) {
            window.visualViewport.removeEventListener('resize', layout);
            window.visualViewport.removeEventListener('scroll', layout);
        }
        if (root && root.parentNode) {
            root.parentNode.removeChild(root);
        }
        root = null;
        barFill = null;
        barFull = null;
        loaderText = null;
    }

    function sowAnalyticsEnvelope(name) {
        let session;
        try {
            session = sessionStorage.getItem('sow_analytics_session');
            if (!session) {
                session = (crypto.randomUUID ? crypto.randomUUID() : String(Date.now()) + Math.random().toString(16).slice(2));
                sessionStorage.setItem('sow_analytics_session', session);
            }
        } catch (_) {
            session = String(Date.now());
        }
        return {
            v: 1,
            name: name,
            ts_ms: Date.now(),
            session_id: session,
            portal: window.SOW_PORTAL || 'site',
            platform: 'web',
            build: window.SOW_BUILD_TS && window.SOW_BUILD_TS !== '__BUILD_TS__' ? window.SOW_BUILD_TS : undefined,
            locale: navigator.language || '',
        };
    }

    function sowTrack(name) {
        try {
            const base = String(window.SOW_DATABASE_URL || '/api').replace(/\/$/, '');
            fetch(base + '/event', {
                method: 'POST',
                keepalive: true,
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ events: [sowAnalyticsEnvelope(name)] }),
            }).catch(() => {});
        } catch (_) {}
    }

    function finish() {
        if (!root || finishing) return;
        const closingRoot = root;
        finishing = true;
        sowTrack('shell_loaded');
        stopProgress();
        root.style.pointerEvents = 'none';

        if (barFill) {
            barFill.style.transition = 'width 150ms ease-out';
            barFill.style.width = '100%';
        }

        root.style.transition = `opacity ${FADEOUT_MS}ms ease-out`;
        setTimeout(() => {
            if (root !== closingRoot) return;
            root.style.opacity = '0';
            setTimeout(() => teardown(closingRoot), FADEOUT_MS + 30);
        }, 160);
    }

    function sync(state) {
        if (!state) return;
        const active = state.phase !== 'MainMenu' && state.phase !== 'Playing';
        if (!active) {
            finish();
            return;
        }

        if (!root) {
            finishing = false;
            buildDom();
        }
        if (finishing) finishing = false;
        const progress = Number(state.loader_progress);
        if (Number.isFinite(progress)) {
            reportedProgress = Math.max(0, Math.min(1, progress));
            if (barFill) barFill.style.width = (reportedProgress * 100).toFixed(1) + '%';
        }
        if (loaderText) {
            const status = typeof state.loader_status === 'string' ? state.loader_status.trim() : '';
            loaderText.textContent = status || 'Loading...';
        }
        if (root) root.setAttribute('aria-busy', 'true');
    }

    function initWebLoader() {
        buildDom();
    }

    window.hideWebLoader = finish;
    window.SOW_initWebLoader = initWebLoader;
    window.SOW_syncWebLoader = sync;

    // Game shell (play subdomain / portal): #web-loader in index.html auto-starts.
    if (document.getElementById('web-loader')) {
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', initWebLoader);
        } else {
            initWebLoader();
        }
    }
})();
