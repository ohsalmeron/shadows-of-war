import init, { SimulationWorker } from '../pkg/sow_wasm';
import type { ToWorkerMessage } from './bridge';

let sim: SimulationWorker;

async function start() {
  const wasm = await init();
  
  sim = new SimulationWorker();
  
  // Spawn human first (Player 1)
  sim.spawn_human(1);
  // Spawn 4 bots
  sim.spawn_random_bots(4);
  
  // @ts-ignore: WASM not recompiled yet
  const width = sim.map_width();
  const height = sim.map_height();
  const statePtr = sim.map_state_ptr();
  const terrainPtr = sim.map_terrain_ptr();
  const tileCount = width * height;
  
  // Copy the static terrain data once
  const terrainArray = new Uint8Array(wasm.memory.buffer, terrainPtr, tileCount);
  const terrainCopy = new Uint8Array(terrainArray);
  
  // Tell main thread about map size
  self.postMessage({ type: 'init_map', width, height, terrain: terrainCopy }, { transfer: [terrainCopy.buffer] });
  
  setInterval(() => {
    // Tick the simulation
    const tick = sim.tick();
    
    // Copy the ownership data
    const stateArray = new Uint16Array(wasm.memory.buffer, statePtr, tileCount);
    const stateCopy = new Uint16Array(stateArray); // Slice a copy so we don't send memory pointer directly
    
    // Broadcast state update to UI
    self.postMessage({
      type: 'state_update',
      payload: {
        tick,
        state: stateCopy, // transfer
        troops: sim.human_troops(),
        gold: sim.human_gold(),
        maxTroops: sim.human_max_troops()
      }
    }, { transfer: [stateCopy.buffer] }); // Use options object for TS
    
  }, 100); // 100ms lockstep tick
}

start().catch(console.error);

self.onmessage = (e: MessageEvent<ToWorkerMessage>) => {
  if (sim) {
    sim.handle_intent(e.data);
  }
};
