(() => {
  const $ = selector => document.querySelector(selector);
  const $$ = selector => [...document.querySelectorAll(selector)];

  // First-party, vendor-free landing funnel telemetry. The game shell owns
  // gameplay events; this page only measures entry and the Play now CTA.
  const siteSessionId = (() => {
    try {
      const key = 'sow_site_session_id';
      const existing = sessionStorage.getItem(key);
      if (existing) return existing;
      const created = globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(16).slice(2)}`;
      sessionStorage.setItem(key, created);
      return created;
    } catch (_) {
      return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
    }
  })();

  function siteTrack(name, props) {
    const event = {
      v: 1,
      name,
      ts_ms: Date.now(),
      session_id: siteSessionId,
      portal: 'site',
      platform: 'web',
      build: 'site',
      locale: (navigator.language || 'en').slice(0, 32),
    };
    if (props && typeof props === 'object') event.props = props;
    fetch('/api/event', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ events: [event] }),
      keepalive: true,
    }).catch(() => {});
  }

  const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');

  const cards = $$('.leader-card[data-leader-id]');
  const leaders = cards.map(c => ({
    id: c.dataset.leaderId,
    name: c.dataset.name,
    civ: c.dataset.civ,
    code: c.dataset.code,
    ability: c.dataset.ability,
    description: c.dataset.description,
    image: c.dataset.image
  }));

  const asset = (leader, mobile = false) => `/assets/shell/leaders/${leader.image}_${mobile ? 'mobile' : 'desktop'}.webp`;
  const avatar = leader => `/assets/gameplay/avatars/${leader.image}.webp`;

  function setTheme(theme) {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem('sow-theme', theme);
    const isLight = theme === 'light';
    $$('[data-theme-toggle]').forEach(toggle => {
      toggle.querySelector('.theme-icon').textContent = isLight ? '☾' : '☼';
      toggle.querySelector('.theme-label').textContent = isLight ? 'Dark mode' : 'Light mode';
      toggle.setAttribute('aria-label', isLight ? 'Switch to dark mode' : 'Switch to light mode');
    });
    const themeColor = $('meta[name="theme-color"]');
    if (themeColor) themeColor.content = isLight ? '#f5f0e6' : '#0a0a0e';
  }

  function initTheme() {
    const saved = localStorage.getItem('sow-theme');
    const preferred = window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
    setTheme(saved || preferred);
    $$('[data-theme-toggle]').forEach(toggle => toggle.addEventListener('click', () => {
      setTheme(document.documentElement.dataset.theme === 'light' ? 'dark' : 'light');
    }));
  }

  function updateLeader(index) {
    const leader = leaders[index];
    if (!leader) return;

    // Trigger subtle glitch burst on hero frame
    const glitch = $('.hologram-glitch');
    if (glitch && !prefersReducedMotion.matches) {
      glitch.style.opacity = '0.5';
      setTimeout(() => { glitch.style.opacity = ''; }, 120);
    }

    const heroImage = $('[data-hero-image]');
    const mobileImage = $('[data-hero-mobile]');
    if (heroImage) {
      heroImage.src = asset(leader);
      heroImage.alt = `${leader.name} leader artwork`;
    }
    if (mobileImage) mobileImage.srcset = asset(leader, true);
    const heroName = $('[data-hero-name]');
    if (heroName) heroName.textContent = leader.name;
    const heroCiv = $('[data-hero-civ]');
    if (heroCiv) heroCiv.textContent = leader.civ;
    const heroCode = $('[data-hero-code]');
    if (heroCode) heroCode.textContent = `${leader.code} / ${String(index + 1).padStart(2, '0')}`;
    const heroIndex = $('.hero-frame-index');
    if (heroIndex) heroIndex.innerHTML = `${String(index + 1).padStart(2, '0')} <span>/</span> 12`;
    const detailImage = $('[data-detail-image]');
    if (detailImage) {
      detailImage.src = asset(leader);
      detailImage.srcset = `${asset(leader, true)} 600w, ${asset(leader)} 1200w`;
      detailImage.alt = `${leader.name} artwork`;
    }
    const detailCode = $('[data-detail-code]');
    if (detailCode) detailCode.textContent = `${leader.code} · ${String(index + 1).padStart(2, '0')}`;
    const detailName = $('[data-detail-name]');
    if (detailName) detailName.textContent = leader.name;
    const detailCiv = $('[data-detail-civ]');
    if (detailCiv) detailCiv.textContent = leader.civ;
    const detailAbility = $('[data-detail-ability]');
    if (detailAbility) detailAbility.textContent = leader.ability;
    const detailDesc = $('[data-detail-description]');
    if (detailDesc) {
      detailDesc.textContent = leader.description;
    }

    $$('.leader-chip').forEach((item, itemIndex) => {
      const active = itemIndex === index;
      item.classList.toggle('is-active', active);
      item.setAttribute('aria-pressed', String(active));
    });
    cards.forEach((item, itemIndex) => {
      const active = itemIndex === index;
      item.classList.toggle('is-active', active);
      item.setAttribute('aria-pressed', String(active));
    });
  }

  function renderLeaderRail() {
    const rail = $('[data-leader-rail]');
    if (!rail || !leaders.length) return;
    rail.innerHTML = '';
    leaders.forEach((leader, index) => {
      const button = document.createElement('button');
      button.className = `leader-chip${index === 0 ? ' is-active' : ''}`;
      button.type = 'button';
      button.setAttribute('aria-pressed', index === 0 ? 'true' : 'false');
      button.title = leader.name;
      button.setAttribute('aria-label', `Select ${leader.name}`);
      button.innerHTML = `<span class="sheen" aria-hidden="true"></span><img src="${avatar(leader)}" alt=""><span>${String(index + 1).padStart(2, '0')}</span>`;
      button.addEventListener('click', () => updateLeader(index));
      rail.appendChild(button);
    });
  }

  function bindLeaderGrid() {
    cards.forEach((card, index) => {
      const sheen = document.createElement('span');
      sheen.className = 'sheen';
      sheen.setAttribute('aria-hidden', 'true');
      card.appendChild(sheen);

      card.addEventListener('click', () => {
        updateLeader(index);
        $('[data-leader-detail]')?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      });
    });
  }

  function bindSiteAnalytics() {
    siteTrack('landing_visit');
    $$('a[href="/play/"]').forEach(link => {
      link.addEventListener('click', () => siteTrack('play_now_click'));
    });
  }

  function initMenu() {
    const menu = $('#mobile-menu');
    const toggle = $('[data-menu-toggle]');
    if (!menu || !toggle) return;
    menu.setAttribute('aria-hidden', 'true');
    toggle.addEventListener('click', () => {
      const open = menu.classList.toggle('is-open');
      toggle.setAttribute('aria-expanded', String(open));
      toggle.setAttribute('aria-label', open ? 'Close navigation' : 'Open navigation');
      menu.setAttribute('aria-hidden', String(!open));
    });
    $$('#mobile-menu a').forEach(link => link.addEventListener('click', () => {
      menu.classList.remove('is-open');
      toggle.setAttribute('aria-expanded', 'false');
      menu.setAttribute('aria-hidden', 'true');
    }));
  }

  function initWouAuth() {
    if (typeof window.wouAuth === 'undefined') return;

    const triggers = $$('[data-wou-auth-trigger]');
    const labels = $$('[data-wou-auth-label]');
    const modal = $('#wou-auth-modal');
    const closeBtn = $('#wou-modal-close-btn');
    const loggedView = $('#wou-auth-logged-view');
    const unloggedView = $('#wou-auth-unlogged-view');
    const userName = $('#wou-user-name');
    const userClan = $('#wou-user-clan');
    const userId = $('#wou-user-id');
    const logoutBtn = $('#wou-logout-btn');
    const authMsg = $('#wou-auth-msg');

    function showMsg(text, type = 'error') {
      if (!authMsg) return;
      authMsg.textContent = text;
      authMsg.className = `wou-msg ${type}`;
      authMsg.classList.remove('hidden');
    }

    const modalHeaderTitle = $('#wou-modal-header-title');

    function updateAuthUi() {
      const isAuth = window.wouAuth.isAuthenticated();
      const user = window.wouAuth.getUser();

      if (isAuth && user) {
        if (modalHeaderTitle) modalHeaderTitle.textContent = 'Account Profile';
        labels.forEach(label => { label.textContent = user.clan_tag ? `[${user.clan_tag}] ${user.display_name}` : (user.display_name || user.username || 'Player'); });
        if (loggedView) loggedView.classList.remove('hidden');
        if (unloggedView) unloggedView.classList.add('hidden');
        if (userName) userName.textContent = user.display_name || user.username || 'Player';
        if (userClan) userClan.textContent = user.clan_tag ? `Clan: [${user.clan_tag}]` : 'No Clan';
        if (userId) userId.textContent = `ID: ${user.id}`;
      } else {
        if (modalHeaderTitle) modalHeaderTitle.textContent = 'Sign In';
        labels.forEach(label => { label.textContent = 'Sign in'; });
        if (loggedView) loggedView.classList.add('hidden');
        if (unloggedView) unloggedView.classList.remove('hidden');
      }
    }

    triggers.forEach(trigger => trigger.addEventListener('click', () => {
      updateAuthUi();
      window.wouAuth.openModal();
    }));

    closeBtn?.addEventListener('click', () => {
      window.wouAuth.closeModal();
    });

    modal?.addEventListener('click', (e) => {
      if (e.target === modal) {
        window.wouAuth.closeModal();
      }
    });

    logoutBtn?.addEventListener('click', () => {
      window.wouAuth.logout();
    });

    // SSO buttons
    $('#wou-login-google')?.addEventListener('click', () => {
      window.wouAuth.loginWithOAuth('google');
    });

    $('#wou-login-discord')?.addEventListener('click', () => {
      window.wouAuth.loginWithOAuth('discord');
    });

    $('#wou-login-twitter')?.addEventListener('click', () => {
      window.wouAuth.loginWithOAuth('twitter');
    });

    $('#wou-login-meta')?.addEventListener('click', () => {
      window.wouAuth.loginWithOAuth('meta');
    });

    $('#wou-login-eth')?.addEventListener('click', async () => {
      try {
        await window.wouAuth.loginWithEthereum();
      } catch (err) {
        showMsg(err.message || 'MetaMask connection failed');
      }
    });

    $('#wou-login-sol')?.addEventListener('click', async () => {
      try {
        await window.wouAuth.loginWithSolana();
      } catch (err) {
        showMsg(err.message || 'Phantom connection failed');
      }
    });

    $('#wou-login-passkey')?.addEventListener('click', async () => {
      try {
        await window.wouAuth.loginWithPasskey();
      } catch (err) {
        showMsg(err.message || 'Passkey authentication failed');
      }
    });

    // Email OTP flow
    let pendingEmail = '';
    const emailForm = $('#wou-email-form');
    const emailInput = $('#wou-email-input');
    const otpInput = $('#wou-otp-input');
    const step1 = $('#wou-email-step-1');
    const step2 = $('#wou-email-step-2');
    const verifyOtpBtn = $('#wou-verify-otp-btn');

    emailForm?.addEventListener('submit', async (e) => {
      e.preventDefault();
      const email = emailInput?.value.trim();
      if (!email) return;
      pendingEmail = email;
      try {
        await window.wouAuth.requestOtp(email, false, 'shadows_of_war');
        step1?.classList.add('hidden');
        step2?.classList.remove('hidden');
        showMsg(`Verification code sent to ${email}`, 'success');
      } catch (err) {
        showMsg(window.wouAuth.describeOtpError(err).message || 'Failed to send code');
      }
    });

    verifyOtpBtn?.addEventListener('click', async () => {
      const code = otpInput?.value.trim();
      if (!code || !pendingEmail) return;
      try {
        await window.wouAuth.verifyOtp(pendingEmail, code, 'shadows_of_war');
      } catch (err) {
        showMsg(err.message || 'Invalid code');
      }
    });

    window.addEventListener('wou:auth-state-change', () => {
      updateAuthUi();
    });

    updateAuthUi();
  }

  initTheme();
  renderLeaderRail();
  bindLeaderGrid();
  if (leaders.length) {
    updateLeader(0);
  }
  initMenu();
  initWouAuth();
  bindSiteAnalytics();
})();
