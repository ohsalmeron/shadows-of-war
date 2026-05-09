import './style.css';
import { setupLobby } from './lobby/lobby';
import { setupCombatHud } from './hud/combat';

// @ts-ignore
if (window.__TAURI__) {
  document.body.setAttribute('data-tauri', 'true');
}

const uiLayer = document.getElementById('ui-layer')!;

// Spawn the WASM Worker
const worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });

const canvas = document.getElementById('render-canvas') as HTMLCanvasElement;
const ctx = canvas.getContext('2d')!;

let mapTerrain: Uint8Array | null = null;
let mapState: Uint16Array | null = null;
let mapWidth = 0;
let mapHeight = 0;

// Camera
let cameraX = 0;
let cameraY = 0;
let cameraZoom = 4; // TILE_SIZE equivalent
let isDragging = false;
let lastMouseX = 0;
let lastMouseY = 0;

// FPS
let lastTime = performance.now();
let frameCount = 0;
let fps = 0;

function resize() {
  canvas.width = window.innerWidth;
  canvas.height = window.innerHeight;
}
window.addEventListener('resize', resize);
resize();

// Bot colors mapped from IDs 100-103, and 1 for Human
const colors: Record<number, string> = {
  1: '#2266ff', // Human is Blue
  100: '#ff4444',
  101: '#44ff44',
  102: '#ffff44',
  103: '#aa44aa'
};

// Controls
canvas.addEventListener('mousedown', (e) => {
  isDragging = true;
  lastMouseX = e.clientX;
  lastMouseY = e.clientY;
});

canvas.addEventListener('mousemove', (e) => {
  if (isDragging) {
    cameraX += (e.clientX - lastMouseX);
    cameraY += (e.clientY - lastMouseY);
    lastMouseX = e.clientX;
    lastMouseY = e.clientY;
  }
});

canvas.addEventListener('mouseup', (e) => {
  isDragging = false;
  // If it was a click (not a drag), send intent
  // Simple heuristic: if we didn't move much, it's a click. 
  // For now just fire it anyway as an example.
});

canvas.addEventListener('click', (e) => {
  // Convert screen coordinates to map coordinates
  const mapX = Math.floor((e.clientX - cameraX) / cameraZoom);
  const mapY = Math.floor((e.clientY - cameraY) / cameraZoom);
  
  if (mapX >= 0 && mapX < mapWidth && mapY >= 0 && mapY < mapHeight) {
    console.log(`Clicked map at ${mapX}, ${mapY}`);
    // Extract owner from state
    if (mapState) {
      const idx = mapY * mapWidth + mapX;
      const owner = mapState[idx] & 0x0FFF;
      
      // Send Attack Intent
      worker.postMessage({
        type: 'attack', // Temporary: handle_intent will need proper JSON matching Rust struct
        target_owner: owner,
        troops: null
      });
    }
  }
});

canvas.addEventListener('wheel', (e) => {
  const zoomFactor = 1.1;
  const oldZoom = cameraZoom;
  if (e.deltaY < 0) cameraZoom *= zoomFactor;
  else cameraZoom /= zoomFactor;
  
  // Clamp zoom
  cameraZoom = Math.max(1, Math.min(cameraZoom, 20));
  
  // Zoom towards mouse
  const mouseX = e.clientX;
  const mouseY = e.clientY;
  cameraX = mouseX - (mouseX - cameraX) * (cameraZoom / oldZoom);
  cameraY = mouseY - (mouseY - cameraY) * (cameraZoom / oldZoom);
});

worker.onmessage = (e: MessageEvent) => {
  const data = e.data;
  
  if (data.type === 'init_map') {
    mapWidth = data.width;
    mapHeight = data.height;
    mapTerrain = data.terrain; // Uint8Array
    
    // Center camera
    cameraX = window.innerWidth / 2 - (mapWidth * cameraZoom) / 2;
    cameraY = window.innerHeight / 2 - (mapHeight * cameraZoom) / 2;
    
    console.log(`Initialized Map: ${mapWidth}x${mapHeight}`);
  } else if (data.type === 'state_update') {
    mapState = data.payload.state;
    // update combat HUD
    const w = window as any;
    if (w.updateCombatHud) {
      w.updateCombatHud(data.payload.troops, data.payload.gold, data.payload.maxTroops);
    }
  }
};

function render() {
  requestAnimationFrame(render);
  
  const now = performance.now();
  frameCount++;
  if (now - lastTime >= 1000) {
    fps = frameCount;
    frameCount = 0;
    lastTime = now;
  }
  
  ctx.fillStyle = '#0a0a1a'; // Space/Dark background
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  
  if (!mapTerrain || !mapState || mapWidth === 0) return;
  
  // Calculate viewport bounds to only render visible tiles
  const startX = Math.max(0, Math.floor(-cameraX / cameraZoom));
  const startY = Math.max(0, Math.floor(-cameraY / cameraZoom));
  const endX = Math.min(mapWidth, Math.ceil((canvas.width - cameraX) / cameraZoom));
  const endY = Math.min(mapHeight, Math.ceil((canvas.height - cameraY) / cameraZoom));
  
  const viewWidth = endX - startX;
  const viewHeight = endY - startY;
  if (viewWidth > 0 && viewHeight > 0) {
    const imageData = new ImageData(viewWidth, viewHeight);
    const data = imageData.data;
    
    for (let y = startY; y < endY; y++) {
      for (let x = startX; x < endX; x++) {
        const idx = y * mapWidth + x;
        const tileInfo = mapTerrain[idx];
        const isLand = (tileInfo & 0b10000000) !== 0;
        
        const owner = mapState[idx] & 0x0FFF;
        
        let r, g, b;
        if (owner > 0) {
          if (owner === 1) { r=34; g=102; b=255; }
          else if (owner === 100) { r=255; g=68; b=68; }
          else if (owner === 101) { r=68; g=255; b=68; }
          else if (owner === 102) { r=255; g=255; b=68; }
          else if (owner === 103) { r=170; g=68; b=170; }
          else { r=255; g=255; b=255; }
        } else if (isLand) {
          r=45; g=76; b=30; // #2d4c1e
        } else {
          r=30; g=60; b=90; // #1e3c5a
        }
        
        const offset = ((y - startY) * viewWidth + (x - startX)) * 4;
        data[offset] = r;
        data[offset+1] = g;
        data[offset+2] = b;
        data[offset+3] = 255;
      }
    }
    
    const offCanvas = document.createElement('canvas');
    offCanvas.width = viewWidth;
    offCanvas.height = viewHeight;
    offCanvas.getContext('2d')!.putImageData(imageData, 0, 0);

    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(
      offCanvas,
      Math.floor(cameraX + startX * cameraZoom),
      Math.floor(cameraY + startY * cameraZoom),
      Math.ceil(viewWidth * cameraZoom),
      Math.ceil(viewHeight * cameraZoom)
    );
  }
  
  // Draw FPS
  ctx.fillStyle = '#ffffff';
  ctx.font = '16px monospace';
  ctx.fillText(`FPS: ${fps}`, 10, 20);
}

// Start loop
requestAnimationFrame(render);
// App State
type AppState = 'LOBBY' | 'PLAYING';
export function setAppState(state: AppState) {
  uiLayer.innerHTML = ''; // Clear UI
  
  if (state === 'LOBBY') {
    setupLobby(uiLayer, () => {
      // On game start
      setAppState('PLAYING');
    });
  } else if (state === 'PLAYING') {
    setupCombatHud(uiLayer);
  }
}

// Initialize
setAppState('LOBBY');
