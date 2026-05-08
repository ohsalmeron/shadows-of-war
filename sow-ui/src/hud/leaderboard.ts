export function setupLeaderboard(container: HTMLElement) {
  const leaderboardHtml = `
    <div class="leaderboard glass-panel interactive">
      <h3>Leaderboard</h3>
      <table>
        <thead>
          <tr>
            <th>Rank</th>
            <th>Player</th>
            <th>Score</th>
          </tr>
        </thead>
        <tbody id="leaderboard-body">
          <tr style="color: var(--primary)">
            <td>1</td>
            <td>Bizkit</td>
            <td>4,520</td>
          </tr>
          <tr style="color: var(--accent-red)">
            <td>2</td>
            <td>EnemyCommander</td>
            <td>3,100</td>
          </tr>
          <tr style="color: var(--accent-green)">
            <td>3</td>
            <td>Bot 1</td>
            <td>1,200</td>
          </tr>
        </tbody>
      </table>
    </div>
  `;

  const wrapper = document.createElement('div');
  wrapper.innerHTML = leaderboardHtml;
  container.appendChild(wrapper);

  const style = document.createElement('style');
  style.textContent = `
    .leaderboard {
      position: absolute;
      top: 16px;
      right: 16px;
      padding: 16px;
      min-width: 250px;
    }
    
    .leaderboard h3 {
      margin-bottom: 12px;
      font-size: 1.2rem;
      border-bottom: 1px solid var(--panel-border);
      padding-bottom: 8px;
    }
    
    .leaderboard table {
      width: 100%;
      border-collapse: collapse;
      text-align: left;
    }
    
    .leaderboard th {
      color: var(--text-muted);
      font-size: 0.85rem;
      text-transform: uppercase;
      padding-bottom: 8px;
    }
    
    .leaderboard td {
      padding: 8px 0;
      font-weight: 600;
      font-family: var(--font-display);
    }
  `;
  container.appendChild(style);
}
