/* ==========================================================================
   SHADOWS OF WAR V4.1 - SMASH BROS ENGINE (REPAIR & STABILITY FIX)
   ========================================================================== */

const OFFICIAL_LEADERS = [
  {
    id: "caesar",
    enumName: "Caesar",
    name: "Julius Caesar",
    civ: "Roman Empire",
    civCode: "ROM",
    perkCategory: "City",
    perkTitle: "Legions of Rome",
    perkDesc: "Armies fight 10% stronger.",
    troopMult: 1.10,
    districtBuff: "City & Armory Boost",
    desktopImg: "/assets/cdn/leaders/caesar_desktop.webp",
    avatarImg: "/assets/cdn/avatars/caesar.webp"
  },
  {
    id: "cleopatra",
    enumName: "Cleopatra",
    name: "Cleopatra VII",
    civ: "Egyptian Empire",
    civCode: "EGY",
    perkCategory: "Gold",
    perkTitle: "Gift of the Nile",
    perkDesc: "Factory districts generate +50% Gold.",
    troopMult: 1.0,
    districtBuff: "+50% Gold Boost",
    desktopImg: "/assets/cdn/leaders/cleopatra_desktop.webp",
    avatarImg: "/assets/cdn/avatars/cleopatra.webp"
  },
  {
    id: "ragnar",
    enumName: "Ragnar",
    name: "Ragnar Lothbrok",
    civ: "Norse Kingdom",
    civCode: "NOR",
    perkCategory: "Gold",
    perkTitle: "Longship Raid",
    perkDesc: "Ports generate +50% Gold.",
    troopMult: 1.0,
    districtBuff: "+50% Port Gold",
    desktopImg: "/assets/cdn/leaders/ragnar_desktop.webp",
    avatarImg: "/assets/cdn/avatars/ragnar.webp"
  },
  {
    id: "suntzu",
    enumName: "SunTzu",
    name: "Sun Tzu",
    civ: "Chinese Empire",
    civCode: "CHN",
    perkCategory: "Production",
    perkTitle: "Art of War",
    perkDesc: "Factory districts produce troops 20% faster.",
    troopMult: 1.0,
    districtBuff: "+20% Production Speed",
    desktopImg: "/assets/cdn/leaders/sun_tzu_desktop.webp",
    avatarImg: "/assets/cdn/avatars/sun_tzu.webp"
  },
  {
    id: "alexander",
    enumName: "Alexander",
    name: "Alexander the Great",
    civ: "Macedonian Empire",
    civCode: "MAC",
    perkCategory: "Conquest",
    perkTitle: "Great Conquest",
    perkDesc: "Territory conquest expands 15% faster.",
    troopMult: 1.0,
    districtBuff: "+15% Expansion Speed",
    desktopImg: "/assets/cdn/leaders/alexander_desktop.webp",
    avatarImg: "/assets/cdn/avatars/alexander.webp"
  },
  {
    id: "genghiskhan",
    enumName: "GenghisKhan",
    name: "Genghis Khan",
    civ: "Mongol Horde",
    civCode: "MON",
    perkCategory: "Gold",
    perkTitle: "Horde Momentum",
    perkDesc: "Gain 10% gold spent by defeated enemies.",
    troopMult: 1.0,
    districtBuff: "+10% Loot Bonus",
    desktopImg: "/assets/cdn/leaders/genghis_khan_desktop.webp",
    avatarImg: "/assets/cdn/avatars/genghis_khan.webp"
  },
  {
    id: "richard",
    enumName: "RichardTheLionheart",
    name: "Richard the Lionheart",
    civ: "Angevin Empire",
    civCode: "ENG",
    perkCategory: "City",
    perkTitle: "Crusader Fortresses",
    perkDesc: "City districts grant +50% max troop capacity.",
    troopMult: 1.0,
    districtBuff: "+50% Max Capacity",
    desktopImg: "/assets/cdn/leaders/richard_the_lionheart_desktop.webp",
    avatarImg: "/assets/cdn/avatars/richard_the_lionheart.webp"
  },
  {
    id: "vercingetorix",
    enumName: "Vercingetorix",
    name: "Vercingetorix",
    civ: "Gallic Tribes",
    civCode: "GAL",
    perkCategory: "City",
    perkTitle: "Hillfort Gaul",
    perkDesc: "City districts generate +50% troop income.",
    troopMult: 1.0,
    districtBuff: "+50% Troop Income",
    desktopImg: "/assets/cdn/leaders/vercingetorix_desktop.webp",
    avatarImg: "/assets/cdn/avatars/vercingetorix.webp"
  },
  {
    id: "boudica",
    enumName: "Boudica",
    name: "Boudica",
    civ: "Iceni Kingdom",
    civCode: "ICE",
    perkCategory: "Gold",
    perkTitle: "Iceni Revolt",
    perkDesc: "City districts generate +50% Gold.",
    troopMult: 1.0,
    districtBuff: "+50% City Gold",
    desktopImg: "/assets/cdn/leaders/boudica_desktop.webp",
    avatarImg: "/assets/cdn/avatars/boudica.webp"
  },
  {
    id: "ladysixsky",
    enumName: "LadySixSky",
    name: "Lady Six Sky",
    civ: "Maya Civilization",
    civCode: "MAY",
    perkCategory: "Gold",
    perkTitle: "Temple of the Sky",
    perkDesc: "Factory districts generate +50% Gold.",
    troopMult: 1.0,
    districtBuff: "+50% Factory Gold",
    desktopImg: "/assets/cdn/leaders/lady_six_sky_desktop.webp",
    avatarImg: "/assets/cdn/avatars/lady_six_sky.webp"
  },
  {
    id: "leonidas",
    enumName: "Leonidas",
    name: "King Leonidas",
    civ: "Sparta",
    civCode: "SPA",
    perkCategory: "City",
    perkTitle: "Spartan Phalanx",
    perkDesc: "Armory districts grant +50% max troop capacity.",
    troopMult: 1.0,
    districtBuff: "+50% Armory Capacity",
    desktopImg: "/assets/cdn/leaders/leonidas_desktop.webp",
    avatarImg: "/assets/cdn/avatars/leonidas.webp"
  },
  {
    id: "napoleon",
    enumName: "Napoleon",
    name: "Napoleon Bonaparte",
    civ: "Kingdom of France",
    civCode: "FRA",
    perkCategory: "Conquest",
    perkTitle: "Grande Armée",
    perkDesc: "Territory expansion 20% faster.",
    troopMult: 1.0,
    districtBuff: "+20% Army Speed",
    desktopImg: "/assets/cdn/leaders/napoleon_desktop.webp",
    avatarImg: "/assets/cdn/avatars/napoleon.webp"
  }
];

window.OFFICIAL_LEADERS = OFFICIAL_LEADERS;

let globalData = { leaders: [], realms: [] };
let currentLeaderIdx = 0;

document.addEventListener("DOMContentLoaded", () => {
  initTheme();
  setupViewRouter();
  renderSmashRoster();
  renderLeadersGrid(OFFICIAL_LEADERS);
  loadCompendiumData();
  setupEventListeners();
  setupKeyboardNav();
  setupCompareModal();
  setupMailingWidget();
  checkPreregState();
  selectHero(OFFICIAL_LEADERS[0], 0, false);
});

// View Router
function setupViewRouter() {
  document.querySelectorAll(".nav-tab[data-view], a[data-view]").forEach(btn => {
    btn.addEventListener("click", (e) => {
      e.preventDefault();
      const targetView = btn.getAttribute("data-view");
      if (targetView === "arena") {
        window.location.href = "/play/";
        return;
      }
      showView(targetView);
    });
  });

  const mobileBtn = document.getElementById("mobile-menu-btn");
  const navList = document.getElementById("nav-links-list");

  if (mobileBtn && navList) {
    mobileBtn.addEventListener("click", () => {
      navList.classList.toggle("mobile-open");
    });
  }
}

function showView(viewName) {
  document.querySelectorAll(".subview-section").forEach(sec => {
    sec.classList.remove("active");
  });

  document.querySelectorAll(".nav-tab").forEach(tab => {
    if (tab.getAttribute("data-view") === viewName) {
      tab.classList.add("active");
    } else {
      tab.classList.remove("active");
    }
  });

  const targetSec = document.getElementById(`view-${viewName}`);
  if (targetSec) {
    targetSec.classList.add("active");
    window.scrollTo({ top: 0, behavior: "smooth" });
  }

  const navList = document.getElementById("nav-links-list");
  if (navList) navList.classList.remove("mobile-open");
}

// Minimal Icon Theme Switcher
function initTheme() {
  const savedTheme = localStorage.getItem("sow_theme") || "light";
  applyTheme(savedTheme);

  const toggleBtn = document.getElementById("theme-toggle-btn");
  if (toggleBtn) {
    toggleBtn.addEventListener("click", () => {
      const current = document.documentElement.getAttribute("data-theme") === "dark" ? "dark" : "light";
      const nextTheme = current === "dark" ? "light" : "dark";
      applyTheme(nextTheme);
      localStorage.setItem("sow_theme", nextTheme);
    });
  }
}

function applyTheme(theme) {
  const svgEl = document.getElementById("theme-icon-svg");
  if (theme === "dark") {
    document.documentElement.setAttribute("data-theme", "dark");
    if (svgEl) {
      svgEl.innerHTML = `<circle cx="12" cy="12" r="5"></circle><line x1="12" y1="1" x2="12" y2="3"></line><line x1="12" y1="21" x2="12" y2="23"></line><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"></line><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"></line><line x1="1" y1="12" x2="3" y2="12"></line><line x1="21" y1="12" x2="23" y2="12"></line><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"></line><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"></line>`;
    }
  } else {
    document.documentElement.removeAttribute("data-theme");
    if (svgEl) {
      svgEl.innerHTML = `<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"></path>`;
    }
  }
}

// Floating Mailing List Widget
function setupMailingWidget() {
  const widget = document.getElementById("mailing-widget");
  const closeBtn = document.getElementById("close-mailing-widget");
  const form = document.getElementById("mailing-form");

  if (localStorage.getItem("sow_mailing_subscribed") || localStorage.getItem("sow_mailing_dismissed")) {
    if (widget) widget.style.display = "none";
  }

  if (closeBtn && widget) {
    closeBtn.addEventListener("click", () => {
      widget.style.display = "none";
      localStorage.setItem("sow_mailing_dismissed", "true");
    });
  }

  if (form && widget) {
    form.addEventListener("submit", (e) => {
      e.preventDefault();
      const email = document.getElementById("mailing-email").value;
      localStorage.setItem("sow_mailing_subscribed", email);
      showToast("Done");
      widget.style.display = "none";
    });
  }
}

// Keyboard Navigation
function setupKeyboardNav() {
  document.addEventListener("keydown", (e) => {
    if (e.target.tagName === "INPUT" || e.target.tagName === "TEXTAREA" || e.target.tagName === "SELECT") return;

    if (e.key === "ArrowRight" || e.key === "d" || e.key === "D") {
      const nextIdx = (currentLeaderIdx + 1) % OFFICIAL_LEADERS.length;
      selectHero(OFFICIAL_LEADERS[nextIdx], nextIdx, false);
    } else if (e.key === "ArrowLeft" || e.key === "a" || e.key === "A") {
      const prevIdx = (currentLeaderIdx - 1 + OFFICIAL_LEADERS.length) % OFFICIAL_LEADERS.length;
      selectHero(OFFICIAL_LEADERS[prevIdx], prevIdx, false);
    } else if (e.key === "Escape") {
      document.querySelectorAll(".modal-overlay.active").forEach(m => m.classList.remove("active"));
    }
  });
}

// Copy Build
function copyLeaderBuild() {
  const hero = OFFICIAL_LEADERS[currentLeaderIdx];
  const text = `${hero.name} | ${hero.civ}\nPerk: ${hero.perkTitle}\nDistrict: ${hero.districtBuff}`;
  
  if (navigator.clipboard) {
    navigator.clipboard.writeText(text).then(() => {
      showToast("Done");
    }).catch(err => console.warn(err));
  }
}

// Render Roster with Initial Fallback & Event Listeners
function renderSmashRoster() {
  const container = document.getElementById("smash-roster-strip");
  if (!container) return;

  container.innerHTML = OFFICIAL_LEADERS.map((hero, idx) => {
    const parts = hero.name.split(' ');
    const initials = (parts[0]?.[0] || '') + (parts[1]?.[0] || '');

    return `
      <button class="roster-thumb ${idx === 0 ? 'active' : ''}" data-smash-idx="${idx}" title="${hero.name}">
        <img src="${hero.avatarImg}" alt="${hero.name}" onerror="this.style.display='none'; this.nextElementSibling.style.display='flex';" />
        <div style="display: none; width: 100%; height: 100%; align-items: center; justify-content: center; background: var(--bg-main); color: var(--accent); font-weight: 900;">
          ${initials}
        </div>
      </button>
    `;
  }).join('');

  document.querySelectorAll(".roster-thumb").forEach(thumb => {
    thumb.addEventListener("click", () => {
      const idx = parseInt(thumb.getAttribute("data-smash-idx"), 10);
      selectHero(OFFICIAL_LEADERS[idx], idx, false);
    });
  });
}

// Load Compendium Data (1,270 leaders & 1,062 realms)
async function loadCompendiumData() {
  try {
    let data = null;
    if (window.COMPENDIUM_DATA) {
      data = window.COMPENDIUM_DATA;
    } else if (window.COMPENDIUM_FALLBACK) {
      data = await window.COMPENDIUM_FALLBACK;
    }

    if (!data) {
      const res = await fetch('./data.json');
      if (res.ok) data = await res.json();
    }

    if (data) {
      globalData.leaders = data.leaders || [];
      globalData.realms = data.realms || [];
      renderCompendium(globalData.leaders);
      renderAtlas(globalData.realms, "ALL");
    }
  } catch (e) {
    console.warn("Compendium load fallback:", e);
  }
}

// Render Empires Grid (3 Columns)
function renderLeadersGrid(list) {
  const container = document.getElementById("empires-grid");
  if (!container) return;

  container.innerHTML = list.map((hero) => {
    const origIdx = OFFICIAL_LEADERS.findIndex(h => h.id === hero.id);
    return `
      <div class="legend-card ${origIdx === currentLeaderIdx ? 'active' : ''}" data-hero-idx="${origIdx}">
        <div class="legend-avatar-wrapper">
          <img src="${hero.avatarImg}" alt="${hero.name}" class="legend-avatar-img" onerror="this.src='/assets/cdn/leaders/caesar_desktop.webp';" />
        </div>
        <div class="legend-info">
          <h4 class="legend-name">${hero.name}</h4>
          <span class="legend-title">${hero.civ}</span>
          <button class="btn-dark btn-select-leader" data-hero-idx="${origIdx}" style="margin-top: 0.8rem; padding: 0.4rem 0.8rem; font-size: 0.75rem; width: 100%;">Select</button>
        </div>
      </div>
    `;
  }).join('');

  document.querySelectorAll(".btn-select-leader").forEach(btn => {
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      const idx = parseInt(btn.getAttribute("data-hero-idx"), 10);
      selectHero(OFFICIAL_LEADERS[idx], idx, false);
      showToast("Done");
      showView("home");
    });
  });

  document.querySelectorAll(".legend-card[data-hero-idx]").forEach(card => {
    card.addEventListener("click", () => {
      const idx = parseInt(card.getAttribute("data-hero-idx"), 10);
      selectHero(OFFICIAL_LEADERS[idx], idx, false);
    });
  });
}

// Select Active Hero Stage
function selectHero(hero, idx, shouldScroll = false) {
  currentLeaderIdx = idx;
  const heroImg = document.getElementById("hero-main-img");
  const heroName = document.getElementById("hero-main-name");
  const heroRole = document.getElementById("hero-main-role");
  const heroLore = document.getElementById("hero-main-lore");

  if (heroImg) {
    heroImg.src = hero.desktopImg;
    heroImg.onerror = () => { heroImg.src = "/assets/cdn/leaders/caesar_desktop.webp"; };
  }

  if (heroName) heroName.textContent = hero.name.toUpperCase();
  if (heroRole) heroRole.textContent = hero.civ.toUpperCase();
  if (heroLore) heroLore.textContent = `${hero.districtBuff}`;

  document.querySelectorAll(".roster-thumb").forEach(t => {
    const tIdx = parseInt(t.getAttribute("data-smash-idx"), 10);
    if (tIdx === currentLeaderIdx) {
      t.classList.add("active");
    } else {
      t.classList.remove("active");
    }
  });

  document.querySelectorAll(".legend-card[data-hero-idx]").forEach(c => {
    const cIdx = parseInt(c.getAttribute("data-hero-idx"), 10);
    if (cIdx === currentLeaderIdx) {
      c.classList.add("active");
    } else {
      c.classList.remove("active");
    }
  });

  if (shouldScroll) {
    const stage = document.getElementById("smash-stage-card");
    if (stage) stage.scrollIntoView({ behavior: "smooth", block: "center" });
  }
}
window.selectHero = selectHero;

// Comparison Modal
function setupCompareModal() {
  const btnCompare = document.getElementById("btn-compare-modal");
  const modalCompare = document.getElementById("modal-compare");
  const btnClose = document.getElementById("close-compare-modal");
  const sel1 = document.getElementById("compare-select-1");
  const sel2 = document.getElementById("compare-select-2");

  if (sel1 && sel2) {
    const opts = OFFICIAL_LEADERS.map((h, i) => `<option value="${i}">${h.name}</option>`).join('');
    sel1.innerHTML = opts;
    sel2.innerHTML = opts;
    sel2.selectedIndex = 1;

    const renderCompare = () => {
      const h1 = OFFICIAL_LEADERS[parseInt(sel1.value, 10)];
      const h2 = OFFICIAL_LEADERS[parseInt(sel2.value, 10)];
      const box1 = document.getElementById("compare-box-1");
      const box2 = document.getElementById("compare-box-2");

      if (box1 && h1) {
        box1.innerHTML = `
          <h4 style="font-family: var(--font-title); font-size: 1.1rem;">${h1.name}</h4>
          <span style="font-size: 0.8rem; font-weight: 800; color: var(--accent);">${h1.civ}</span>
          <p style="font-size: 0.85rem; margin-top: 0.5rem; font-weight: 700;">${h1.perkTitle}</p>
        `;
      }

      if (box2 && h2) {
        box2.innerHTML = `
          <h4 style="font-family: var(--font-title); font-size: 1.1rem;">${h2.name}</h4>
          <span style="font-size: 0.8rem; font-weight: 800; color: var(--accent);">${h2.civ}</span>
          <p style="font-size: 0.85rem; margin-top: 0.5rem; font-weight: 700;">${h2.perkTitle}</p>
        `;
      }
    };

    sel1.addEventListener("change", renderCompare);
    sel2.addEventListener("change", renderCompare);

    if (btnCompare && modalCompare) {
      btnCompare.addEventListener("click", () => {
        renderCompare();
        modalCompare.classList.add("active");
      });
    }

    if (btnClose && modalCompare) {
      btnClose.addEventListener("click", () => {
        modalCompare.classList.remove("active");
      });
    }
  }
}

// Render Compendium (1,270 Leaders)
function renderCompendium(leadersList) {
  const container = document.getElementById("compendium-grid");
  if (!container) return;

  if (!leadersList || leadersList.length === 0) {
    container.innerHTML = `<div style="grid-column: 1/-1; text-align: center; padding: 2rem; color: var(--text-sub);">No results</div>`;
    return;
  }

  container.innerHTML = leadersList.slice(0, 48).map(l => `
    <div class="icon-box compendium-card" data-leader-id="${l.id}">
      <div class="icon-label">${l.name}</div>
      <div style="font-size: 0.75rem; color: var(--text-sub); font-weight: 700;">${l.faction} • ${l.era}</div>
    </div>
  `).join('');

  document.querySelectorAll(".compendium-card").forEach(card => {
    card.addEventListener("click", () => {
      const lId = parseInt(card.getAttribute("data-leader-id"), 10);
      const leader = globalData.leaders.find(x => x.id === lId);
      if (leader) openLeaderModal(leader);
    });
  });
}

function openLeaderModal(l) {
  const body = document.getElementById("leader-modal-body");
  const modal = document.getElementById("modal-leader-detail");
  if (!body || !modal) return;

  body.innerHTML = `
    <h3 style="font-size: 1.8rem; font-family: var(--font-title); color: var(--text-main); margin-bottom: 0.4rem;">${l.name}</h3>
    <div style="font-size: 0.85rem; font-weight: 800; color: var(--accent); text-transform: uppercase; margin-bottom: 1rem;">${l.faction} • ${l.era}</div>
    <p style="font-size: 0.95rem; line-height: 1.6; font-weight: 600; color: var(--text-main);">${l.hero}</p>
  `;
  modal.classList.add("active");
}

// Render Atlas (1,062 Realms)
function renderAtlas(realmsList, continentFilter) {
  const container = document.getElementById("realms-grid");
  if (!container) return;

  const filtered = continentFilter === "ALL" 
    ? realmsList 
    : realmsList.filter(r => r.continent.toLowerCase() === continentFilter.toLowerCase());

  if (!filtered || filtered.length === 0) {
    container.innerHTML = `<div style="grid-column: 1/-1; text-align: center; padding: 2rem; color: var(--text-sub);">No results</div>`;
    return;
  }

  container.innerHTML = filtered.slice(0, 48).map(r => `
    <div class="icon-box realm-card-item" data-realm-name="${r.name}" data-realm-kind="${r.kind}" data-realm-era="${r.era}" data-realm-cont="${r.continent}" data-realm-lat="${r.lat}" data-realm-lon="${r.lon}">
      <div class="icon-label">${r.name}</div>
      <div style="font-size: 0.75rem; color: var(--text-sub); font-weight: 700;">${r.continent} • ${r.kind}</div>
    </div>
  `).join('');

  document.querySelectorAll(".realm-card-item").forEach(item => {
    const updateCompass = () => {
      const name = item.getAttribute("data-realm-name");
      const lat = parseFloat(item.getAttribute("data-realm-lat"));
      const lon = parseFloat(item.getAttribute("data-realm-lon"));

      const nameEl = document.getElementById("compass-realm-name");
      const coordsEl = document.getElementById("compass-realm-coords");
      const dotEl = document.getElementById("compass-dot");

      if (nameEl) nameEl.textContent = name;
      if (coordsEl) coordsEl.textContent = `Lat: ${lat.toFixed(2)}°, Lon: ${lon.toFixed(2)}°`;

      if (dotEl) {
        const cx = 50 + (lon / 180) * 45;
        const cy = 50 - (lat / 90) * 45;
        dotEl.setAttribute("cx", Math.max(5, Math.min(95, cx)));
        dotEl.setAttribute("cy", Math.max(5, Math.min(95, cy)));
      }
    };

    item.addEventListener("mouseenter", updateCompass);

    item.addEventListener("click", () => {
      updateCompass();
      const name = item.getAttribute("data-realm-name");
      const kind = item.getAttribute("data-realm-kind");
      const era = item.getAttribute("data-realm-era");
      const cont = item.getAttribute("data-realm-cont");
      const lat = parseFloat(item.getAttribute("data-realm-lat"));
      const lon = parseFloat(item.getAttribute("data-realm-lon"));

      openRealmModal({ name, kind, era, continent: cont, lat, lon });
    });
  });
}

function openRealmModal(realm) {
  const body = document.getElementById("realm-modal-body");
  const modal = document.getElementById("modal-realm-detail");
  if (!body || !modal) return;

  body.innerHTML = `
    <h3 style="font-size: 1.8rem; font-family: var(--font-title); color: var(--text-main); margin-bottom: 0.4rem;">${realm.name}</h3>
    <div style="font-size: 0.85rem; font-weight: 800; color: var(--accent); text-transform: uppercase; margin-bottom: 1rem;">${realm.continent} • ${realm.kind}</div>
    <div style="font-family: monospace; font-size: 0.85rem; margin-bottom: 1.5rem;">Lat: ${realm.lat.toFixed(4)}°, Lon: ${realm.lon.toFixed(4)}°</div>
    <button class="btn-primary pulse" id="btn-play-realm" style="width: 100%; justify-content: center;">
      Play
    </button>
  `;

  modal.classList.add("active");

  const btnPlay = document.getElementById("btn-play-realm");
  if (btnPlay) {
    btnPlay.addEventListener("click", () => {
      showToast("Done");
      modal.classList.remove("active");
      window.location.href = "/play/";
    });
  }
}

// Event Listeners
function setupEventListeners() {
  const btnCopyBuild = document.getElementById("btn-copy-build");
  if (btnCopyBuild) btnCopyBuild.addEventListener("click", copyLeaderBuild);

  document.querySelectorAll("#perk-filter-tabs .cat-tab").forEach(tab => {
    tab.addEventListener("click", () => {
      document.querySelectorAll("#perk-filter-tabs .cat-tab").forEach(t => t.classList.remove("active"));
      tab.classList.add("active");
      const perkCat = tab.getAttribute("data-perk-filter");

      if (perkCat === "ALL") {
        renderLeadersGrid(OFFICIAL_LEADERS);
      } else {
        const filtered = OFFICIAL_LEADERS.filter(h => h.perkCategory === perkCat);
        renderLeadersGrid(filtered);
      }
    });
  });

  const searchInput = document.getElementById("search-leader-input");
  if (searchInput) {
    searchInput.addEventListener("input", (e) => {
      const q = e.target.value.toLowerCase().trim();
      if (!q) {
        renderCompendium(globalData.leaders);
      } else {
        const filtered = globalData.leaders.filter(l => 
          l.name.toLowerCase().includes(q) || 
          l.faction.toLowerCase().includes(q) || 
          l.era.toLowerCase().includes(q)
        );
        renderCompendium(filtered);
      }
    });
  }

  document.querySelectorAll("#continent-tabs .cat-tab").forEach(tab => {
    tab.addEventListener("click", () => {
      document.querySelectorAll("#continent-tabs .cat-tab").forEach(t => t.classList.remove("active"));
      tab.classList.add("active");
      const cont = tab.getAttribute("data-cont");
      renderAtlas(globalData.realms, cont);
    });
  });

  const btnFS = document.getElementById("btn-fullscreen");
  if (btnFS) {
    btnFS.addEventListener("click", () => {
      const container = document.getElementById("game-stage");
      if (container) {
        if (!document.fullscreenElement) {
          container.requestFullscreen().catch(err => console.warn(err));
        } else {
          document.exitFullscreen();
        }
      }
    });
  }

  const modalPre = document.getElementById("modal-prereg");
  const btnPre = document.getElementById("btn-prereg");
  const btnClosePre = document.getElementById("close-prereg-modal");

  if (btnPre && modalPre) {
    btnPre.addEventListener("click", (e) => {
      e.preventDefault();
      modalPre.classList.add("active");
    });
  }

  if (btnClosePre && modalPre) {
    btnClosePre.addEventListener("click", () => {
      modalPre.classList.remove("active");
    });
  }

  const formPrereg = document.getElementById("prereg-form");
  if (formPrereg) {
    formPrereg.addEventListener("submit", (e) => {
      e.preventDefault();
      const name = document.getElementById("prereg-name").value;
      const email = document.getElementById("prereg-email").value;
      localStorage.setItem("sow_commander_registered", JSON.stringify({ name, email, date: new Date().toISOString() }));
      showToast("Done");
      modalPre.classList.remove("active");
      checkPreregState();
    });
  }

  const modalLeaderDetail = document.getElementById("modal-leader-detail");
  const btnCloseLeader = document.getElementById("close-leader-modal");
  if (btnCloseLeader && modalLeaderDetail) {
    btnCloseLeader.addEventListener("click", () => {
      modalLeaderDetail.classList.remove("active");
    });
  }

  const modalRealmDetail = document.getElementById("modal-realm-detail");
  const btnCloseRealm = document.getElementById("close-realm-modal");
  if (btnCloseRealm && modalRealmDetail) {
    btnCloseRealm.addEventListener("click", () => {
      modalRealmDetail.classList.remove("active");
    });
  }

  const modalTerms = document.getElementById("modal-terms");
  const btnShowTerms = document.getElementById("btn-show-terms");
  const btnCloseTerms = document.getElementById("close-terms-modal");

  if (btnShowTerms && modalTerms) {
    btnShowTerms.addEventListener("click", () => {
      modalTerms.classList.add("active");
    });
  }

  if (btnCloseTerms && modalTerms) {
    btnCloseTerms.addEventListener("click", () => {
      modalTerms.classList.remove("active");
    });
  }
}

function checkPreregState() {
  const saved = localStorage.getItem("sow_commander_registered");
  const btnPre = document.getElementById("btn-prereg");
  if (saved && btnPre) {
    btnPre.textContent = `Registered`;
  }
}

function showToast(msg) {
  const toast = document.getElementById("toast-notify");
  if (!toast) return;
  toast.textContent = msg;
  toast.style.display = "block";
  setTimeout(() => {
    toast.style.display = "none";
  }, 2000);
}
