/**
 * Marketing-site embed: iframe to play shell (lazy on Play click).
 * Desktop: hero morphs in-place into game. Mobile (≤480px): fullscreen overlay.
 */
(function () {
    'use strict';

    var PLAY_URL = window.SOW_PLAY_URL;
    if (!PLAY_URL) {
        var host = window.location.hostname;
        if (host === '127.0.0.1' || host === 'localhost') {
            PLAY_URL = '/game/index.html';
        } else {
            PLAY_URL = 'https://play.shadowsofwar.io/';
        }
    }

    var desktopLoaded = false;
    var mobileLoaded = false;

    function isMobile() {
        return window.matchMedia('(max-width: 480px)').matches;
    }

    function loadFrame(frame) {
        if (!frame) {
            return;
        }
        var current = frame.getAttribute('src');
        if (current && current !== 'about:blank') {
            return;
        }
        frame.src = PLAY_URL;
    }

    function enterGame() {
        if (isMobile()) {
            var overlay = document.getElementById('game-overlay');
            var mobileFrame = document.getElementById('sow-game-frame-mobile');
            if (!overlay || !mobileFrame) {
                return;
            }
            if (!mobileLoaded) {
                loadFrame(mobileFrame);
                mobileLoaded = true;
            }
            overlay.hidden = false;
            document.body.classList.add('game-active');
            return;
        }

        var stage = document.getElementById('sow-game-stage');
        var desktopFrame = document.getElementById('sow-game-frame');
        if (!stage || !desktopFrame) {
            return;
        }
        if (!desktopLoaded) {
            loadFrame(desktopFrame);
            desktopLoaded = true;
        }
        stage.classList.add('playing');
    }

    function restoreSite() {
        var overlay = document.getElementById('game-overlay');
        var mobileFrame = document.getElementById('sow-game-frame-mobile');
        if (overlay) {
            overlay.hidden = true;
        }
        document.body.classList.remove('game-active');
        if (mobileFrame) {
            mobileFrame.src = 'about:blank';
            mobileLoaded = false;
        }
    }

    window.SOW_enterGame = enterGame;
    window.SOW_restoreSite = restoreSite;

    function bindPlay(el) {
        if (!el) {
            return;
        }
        el.addEventListener('click', function (e) {
            e.preventDefault();
            enterGame();
        });
    }

    function init() {
        bindPlay(document.getElementById('sow-play-btn'));
        bindPlay(document.getElementById('sow-nav-play'));
        var restore = document.getElementById('game-restore-btn');
        if (restore) {
            restore.addEventListener('click', restoreSite);
        }
        if (window.location.hash === '#play') {
            enterGame();
        }
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
