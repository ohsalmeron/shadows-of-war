export function setupLobby(container: HTMLElement, onStart: () => void) {
  const lobbyHtml = `
    <div class="lobby-container interactive glass-panel">
      <h1>Shadows of War</h1>
      <h2>Lobby Browser</h2>
      
      <div class="server-list">
        <table>
          <thead>
            <tr>
              <th>Server Name</th>
              <th>Map</th>
              <th>Players</th>
              <th>Ping</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>[Official] NA East</td>
              <td>Earth 2042</td>
              <td>14 / 32</td>
              <td><span style="color: var(--accent-green)">24ms</span></td>
              <td><button class="btn-primary join-btn">Join</button></td>
            </tr>
            <tr>
              <td>[Official] EU West</td>
              <td>Pangaea</td>
              <td>31 / 32</td>
              <td><span style="color: var(--accent-gold)">115ms</span></td>
              <td><button class="btn-primary join-btn">Join</button></td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  `;

  const wrapper = document.createElement('div');
  wrapper.innerHTML = lobbyHtml;
  container.appendChild(wrapper);

  // Add styles specific to lobby
  const style = document.createElement('style');
  style.textContent = `
    .lobby-container {
      position: absolute;
      top: 50%;
      left: 50%;
      transform: translate(-50%, -50%);
      width: 800px;
      max-width: 90vw;
      padding: 32px;
      display: flex;
      flex-direction: column;
      gap: 24px;
    }
    
    h1 {
      text-align: center;
      color: var(--primary);
      font-size: 3rem;
      text-transform: uppercase;
      text-shadow: 0 0 20px rgba(0, 210, 255, 0.5);
    }
    
    h2 {
      font-size: 1.5rem;
      border-bottom: 1px solid var(--panel-border);
      padding-bottom: 8px;
    }
    
    .server-list table {
      width: 100%;
      border-collapse: collapse;
      text-align: left;
    }
    
    .server-list th {
      color: var(--text-muted);
      padding: 12px;
      border-bottom: 1px solid var(--panel-border);
    }
    
    .server-list td {
      padding: 16px 12px;
      border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    }
    
    .server-list tr:hover td {
      background: rgba(255, 255, 255, 0.02);
    }
  `;
  container.appendChild(style);

  // Bind events
  const buttons = wrapper.querySelectorAll('.join-btn');
  buttons.forEach(btn => {
    btn.addEventListener('click', () => {
      btn.textContent = 'Connecting...';
      setTimeout(() => {
        onStart();
      }, 500); // Simulate network delay
    });
  });
}
