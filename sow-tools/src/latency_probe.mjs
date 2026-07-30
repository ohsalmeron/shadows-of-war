import WebSocket from 'ws';
import { performance } from 'perf_hooks';

const SERVER = process.env.SOW_WS_URL || 'wss://shadowsofwar.io/ws/';
const DURATION_SECS = parseInt(process.env.DURATION || '30', 10);

let start = performance.now();
let msgTimes = [];
let bytesReceived = 0;
let msgCount = 0;
let errors = 0;
let lastReport = 0;

function log(msg) {
  const t = ((performance.now() - start) / 1000).toFixed(2);
  process.stdout.write(`[+${t}s] ${msg}\n`);
}

log(`Connecting to ${SERVER} for ${DURATION_SECS}s`);

const ws = new WebSocket(SERVER);
let connected = false;

ws.on('open', () => {
  connected = true;
  start = performance.now();
  log('Connected');
});

ws.on('message', (data) => {
  const now = performance.now();
  const elapsed = now - start;
  const sinceLastReport = now - lastReport;

  if (elapsed > DURATION_SECS * 1000) {
    ws.close();
    return;
  }

  msgCount++;
  bytesReceived += data.length;
  msgTimes.push(elapsed);

  // Report every ~5 seconds
  if (lastReport === 0 || sinceLastReport > 5000) {
    const rate = msgCount / (elapsed / 1000);
    const bps = bytesReceived / (elapsed / 1000);
    let minGap = Infinity, maxGap = 0, totalGap = 0;
    let gaps = 0;
    for (let i = 1; i < msgTimes.length; i++) {
      const gap = msgTimes[i] - msgTimes[i-1];
      minGap = Math.min(minGap, gap);
      maxGap = Math.max(maxGap, gap);
      totalGap += gap;
      gaps++;
    }
    const avgGap = gaps > 0 ? totalGap / gaps : 0;

    log(
      `${msgCount} msgs, ${(bps/1024).toFixed(1)} KB/s, ` +
      `${rate.toFixed(1)} msg/s, gaps: min=${(minGap/1000).toFixed(2)}s avg=${(avgGap/1000).toFixed(3)}s max=${(maxGap/1000).toFixed(2)}s`
    );
    lastReport = now;
  }
});

ws.on('error', (err) => {
  errors++;
  log(`Error: ${err.message}`);
});

ws.on('close', (code, reason) => {
  const elapsed = (performance.now() - start) / 1000;
  log(`\n=== RESULTS ===`);
  log(`Duration: ${elapsed.toFixed(2)}s`);
  log(`Messages: ${msgCount}`);
  log(`Errors: ${errors}`);
  log(`Throughput: ${(bytesReceived / 1024 / elapsed).toFixed(2)} KB/s`);
  log(`Message rate: ${(msgCount / elapsed).toFixed(2)} msg/s`);

  if (msgTimes.length > 1) {
    msgTimes.sort((a, b) => a - b);
    const p50 = msgTimes[Math.floor(msgTimes.length * 0.50)];
    const p95 = msgTimes[Math.floor(msgTimes.length * 0.95)];
    const p99 = msgTimes[Math.floor(msgTimes.length * 0.99)];
    const intervals = [];
    for (let i = 1; i < msgTimes.length; i++) {
      intervals.push(msgTimes[i] - msgTimes[i-1]);
    }
    intervals.sort((a, b) => a - b);
    const ip50 = intervals[Math.floor(intervals.length * 0.50)];
    const ip95 = intervals[Math.floor(intervals.length * 0.95)];
    const ip99 = intervals[Math.floor(intervals.length * 0.99)];
    log(`Inter-arrival (ms): p50=${(ip50/1000*1000).toFixed(1)} p95=${(ip95/1000*1000).toFixed(1)} p99=${(ip99/1000*1000).toFixed(1)}`);
  }
  process.exit(errors > 0 ? 1 : 0);
});

// Timeout safety
setTimeout(() => {
  if (connected) {
    log('Timeout reached, closing');
    ws.close();
  } else {
    log('Failed to connect within timeout');
    ws.terminate();
    process.exit(1);
  }
}, (DURATION_SECS + 5) * 1000);
