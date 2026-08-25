(() => {
  const $ = selector => document.querySelector(selector);
  const $$ = selector => [...document.querySelectorAll(selector)];

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

  const asset = (leader, mobile = false) => `/assets/cdn/leaders/${leader.image}_${mobile ? 'mobile' : 'desktop'}.webp`;
  const avatar = leader => `/assets/cdn/avatars/${leader.image}.webp`;

  function setTheme(theme) {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem('sow-theme', theme);
    const isLight = theme === 'light';
    const toggle = $('[data-theme-toggle]');
    if (toggle) {
      toggle.querySelector('.theme-icon').textContent = isLight ? '☾' : '☼';
      toggle.querySelector('.theme-label').textContent = isLight ? 'Dark mode' : 'Light mode';
      toggle.setAttribute('aria-label', isLight ? 'Switch to dark mode' : 'Switch to light mode');
    }
    const themeColor = $('meta[name="theme-color"]');
    if (themeColor) themeColor.content = isLight ? '#f5f0e6' : '#0a0a0e';
  }

  function initTheme() {
    const saved = localStorage.getItem('sow-theme');
    const preferred = window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
    setTheme(saved || preferred);
    $('[data-theme-toggle]')?.addEventListener('click', () => {
      setTheme(document.documentElement.dataset.theme === 'light' ? 'dark' : 'light');
    });
  }

  function updateLeader(index) {
    const leader = leaders[index];
    if (!leader) return;
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
    if (detailDesc) detailDesc.textContent = leader.description;

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
      button.innerHTML = `<img src="${avatar(leader)}" alt=""><span>${String(index + 1).padStart(2, '0')}</span>`;
      button.addEventListener('click', () => updateLeader(index));
      rail.appendChild(button);
    });
  }

  function bindLeaderGrid() {
    cards.forEach((card, index) => {
      card.addEventListener('click', () => {
        updateLeader(index);
        $('[data-leader-detail]')?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      });
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

  initTheme();
  renderLeaderRail();
  bindLeaderGrid();
  if (leaders.length) {
    updateLeader(0);
  }
  initMenu();
})();
