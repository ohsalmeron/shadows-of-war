export function setupCombatHud(container: HTMLElement) {
  const hudHtml = `
    <div class="hud-top-bar glass-panel interactive">
      <div class="resource-group">
        <span class="resource-icon">💰</span>
        <span class="resource-val gold-val" id="hud-gold">0</span>
      </div>
      <div class="resource-group">
        <span class="resource-icon">⚔️</span>
        <span class="resource-val troops-val" id="hud-troops">0 / 0</span>
      </div>
    </div>
    
    <div class="action-menu glass-panel interactive">
      <h3>Build Menu</h3>
      <div class="build-grid">
        <button class="build-btn btn-primary" data-type="farm">
          🌾 Farm
          <span>250 💰</span>
        </button>
        <button class="build-btn btn-primary" data-type="barracks">
          🎪 Barracks
          <span>500 💰</span>
        </button>
        <button class="build-btn btn-primary" data-type="fort">
          🏰 Fort
          <span>1,500 💰</span>
        </button>
      </div>
    </div>
  `;

  const wrapper = document.createElement('div');
  wrapper.innerHTML = hudHtml;
  container.appendChild(wrapper);

  const style = document.createElement('style');
  style.textContent = `
    .hud-top-bar {
      position: absolute;
      top: 16px;
      left: 50%;
      transform: translateX(-50%);
      padding: 12px 32px;
      display: flex;
      gap: 48px;
      border-radius: 32px;
    }
    
    .resource-group {
      display: flex;
      align-items: center;
      gap: 12px;
      font-size: 1.25rem;
      font-family: var(--font-display);
      font-weight: 700;
    }
    
    .gold-val { color: var(--accent-gold); }
    .troops-val { color: var(--accent-red); }
    .territory-val { color: var(--primary); }
    
    .action-menu {
      position: absolute;
      bottom: 24px;
      left: 50%;
      transform: translateX(-50%);
      padding: 24px;
      border-radius: 24px;
    }
    
    .action-menu h3 {
      text-align: center;
      margin-bottom: 16px;
      color: var(--text-muted);
    }
    
    .build-grid {
      display: flex;
      gap: 16px;
    }
    
    .build-btn {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 8px;
      padding: 16px 24px;
      font-size: 1.1rem;
    }
    
    .build-btn span {
      font-size: 0.9rem;
      opacity: 0.8;
      font-weight: 400;
    }
  `;
  container.appendChild(style);

  // Setup Leaderboard as well
  import('./leaderboard').then(({ setupLeaderboard }) => {
    setupLeaderboard(container);
  });

  const goldEl = document.getElementById('hud-gold');
  const troopsEl = document.getElementById('hud-troops');

  (window as any).updateCombatHud = (troops: number, gold: number, maxTroops: number) => {
    if (goldEl) goldEl.innerText = Math.floor(gold).toLocaleString();
    if (troopsEl) troopsEl.innerText = `${Math.floor(troops).toLocaleString()} / ${Math.floor(maxTroops).toLocaleString()}`;
  };
}
