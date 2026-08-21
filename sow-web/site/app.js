(() => {
  const leaders = [
    { id: 'caesar', name: 'Julius Caesar', civ: 'Roman Empire', code: 'ROM', ability: 'Legions of Rome', description: 'Armies fight 10% stronger.', image: 'caesar' },
    { id: 'cleopatra', name: 'Cleopatra VII', civ: 'Egyptian Empire', code: 'EGY', ability: 'Gift of the Nile', description: 'Factory districts generate +50% Gold.', image: 'cleopatra' },
    { id: 'ragnar', name: 'Ragnar Lothbrok', civ: 'Norse Kingdom', code: 'NOR', ability: 'Longship Raid', description: 'Ports generate +50% Gold.', image: 'ragnar' },
    { id: 'suntzu', name: 'Sun Tzu', civ: 'Chinese Empire', code: 'CHN', ability: 'Art of War', description: 'Factory districts produce troops 20% faster.', image: 'sun_tzu' },
    { id: 'alexander', name: 'Alexander the Great', civ: 'Macedonian Empire', code: 'MAC', ability: 'Great Conquest', description: 'Territory conquest expands 15% faster.', image: 'alexander' },
    { id: 'genghiskhan', name: 'Genghis Khan', civ: 'Mongol Horde', code: 'MON', ability: 'Horde Momentum', description: 'Gain 10% gold spent by defeated enemies.', image: 'genghis_khan' },
    { id: 'richard', name: 'Richard the Lionheart', civ: 'Angevin Empire', code: 'ENG', ability: 'Crusader Fortresses', description: 'City districts grant +50% max troop capacity.', image: 'richard_the_lionheart' },
    { id: 'vercingetorix', name: 'Vercingetorix', civ: 'Gallic Tribes', code: 'GAL', ability: 'Hillfort Gaul', description: 'City districts generate +50% troop income.', image: 'vercingetorix' },
    { id: 'boudica', name: 'Boudica', civ: 'Iceni Kingdom', code: 'ICE', ability: 'Iceni Revolt', description: 'City districts generate +50% Gold.', image: 'boudica' },
    { id: 'ladysixsky', name: 'Lady Six Sky', civ: 'Maya Civilization', code: 'MAY', ability: 'Temple of the Sky', description: 'Factory districts generate +50% Gold.', image: 'lady_six_sky' },
    { id: 'leonidas', name: 'King Leonidas', civ: 'Sparta', code: 'SPA', ability: 'Spartan Phalanx', description: 'Armory districts grant +50% max troop capacity.', image: 'leonidas' },
    { id: 'napoleon', name: 'Napoleon Bonaparte', civ: 'Kingdom of France', code: 'FRA', ability: 'Grande Armée', description: 'Territory expansion moves 20% faster.', image: 'napoleon' }
  ];

  const asset = (leader, mobile = false) => `/assets/cdn/leaders/${leader.image}_${mobile ? 'mobile' : 'desktop'}.webp`;
  const avatar = leader => `/assets/cdn/avatars/${leader.image}.webp`;
  const $ = selector => document.querySelector(selector);
  const $$ = selector => [...document.querySelectorAll(selector)];

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
    $('[data-hero-name]').textContent = leader.name;
    $('[data-hero-civ]').textContent = leader.civ;
    $('[data-hero-code]').textContent = `${leader.code} / ${String(index + 1).padStart(2, '0')}`;
    $('.hero-frame-index').innerHTML = `${String(index + 1).padStart(2, '0')} <span>/</span> 12`;
    $('[data-detail-image]').src = asset(leader);
    $('[data-detail-image]').alt = `${leader.name} artwork`;
    $('[data-detail-code]').textContent = `${leader.code} · ${String(index + 1).padStart(2, '0')}`;
    $('[data-detail-name]').textContent = leader.name;
    $('[data-detail-civ]').textContent = leader.civ;
    $('[data-detail-ability]').textContent = leader.ability;
    $('[data-detail-description]').textContent = leader.description;
    $$('.leader-chip, .leader-card').forEach((item, itemIndex) => item.classList.toggle('is-active', itemIndex === index));
  }

  function renderLeaderRail() {
    const rail = $('[data-leader-rail]');
    if (!rail) return;
    leaders.forEach((leader, index) => {
      const button = document.createElement('button');
      button.className = 'leader-chip';
      button.type = 'button';
      button.title = leader.name;
      button.setAttribute('aria-label', `Select ${leader.name}`);
      button.innerHTML = `<img src="${avatar(leader)}" alt=""><span>${String(index + 1).padStart(2, '0')}</span>`;
      button.addEventListener('click', () => updateLeader(index));
      rail.appendChild(button);
    });
  }

  function renderLeaderGrid() {
    const grid = $('[data-leaders-grid]');
    if (!grid) return;
    leaders.forEach((leader, index) => {
      const button = document.createElement('button');
      button.className = 'leader-card';
      button.type = 'button';
      button.setAttribute('aria-label', `Inspect ${leader.name}`);
      button.innerHTML = `<img src="${asset(leader)}" alt="${leader.name} artwork" loading="lazy"><span class="leader-card-info"><b>${leader.name}</b><span>${leader.civ}</span></span>`;
      button.addEventListener('click', () => {
        updateLeader(index);
        $('[data-leader-detail]')?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      });
      grid.appendChild(button);
    });
  }

  function initMenu() {
    const menu = $('#mobile-menu');
    const toggle = $('[data-menu-toggle]');
    if (!menu || !toggle) return;
    toggle.addEventListener('click', () => {
      const open = menu.classList.toggle('is-open');
      toggle.setAttribute('aria-expanded', String(open));
      toggle.setAttribute('aria-label', open ? 'Close navigation' : 'Open navigation');
    });
    $$('#mobile-menu a').forEach(link => link.addEventListener('click', () => {
      menu.classList.remove('is-open');
      toggle.setAttribute('aria-expanded', 'false');
    }));
  }

  initTheme();
  renderLeaderRail();
  renderLeaderGrid();
  updateLeader(0);
  initMenu();
})();
