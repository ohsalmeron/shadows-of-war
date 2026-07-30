#!/bin/sh
# sow-watchdog — every 20s, detect silent failures. No overhead to server.
# Deploy: deploy to sow server, run via nohup or cron.
# No interaction with server code — read-only checks.

PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
WATCHDOG_LOG=/var/log/sow/watchdog.log

if command -v valkey-cli >/dev/null 2>&1; then
    REDIS_CLI=valkey-cli
elif command -v redis-cli >/dev/null 2>&1; then
    REDIS_CLI=redis-cli
else
    REDIS_CLI=:
fi

PORTS_KEY=sow:ports
PGREP="pgrep -l"

log() { echo "$(date '+%Y-%m-%dT%H:%M:%S%z') $*" >> "$WATCHDOG_LOG"; }
alert() { log "ALERT $*"; }

last_ports_count=""
last_relay_count=""

while true; do
  sleep 20

  # 1. Redis alive
  if ! $REDIS_CLI PING 2>/dev/null | grep -q PONG; then
    alert "[REDIS] PING failed — valkey may be down"
  fi

  # 2. sow:ports cardinality — if stuck same value >5min, possible desync
  ports_count=$($REDIS_CLI SCARD "$PORTS_KEY" 2>/dev/null || echo "error")
  if [ "$ports_count" = "error" ]; then
    alert "[REDIS] SCARD $PORTS_KEY failed"
  elif [ -n "$ports_count" ] && [ "$ports_count" -gt 900 ]; then
    alert "[PORTS] Near exhaustion: $ports_count ports allocated (max 910)"
  fi
  if [ -n "$last_ports_count" ] && [ "$ports_count" = "$last_ports_count" ] && [ "$ports_count" != "error" ]; then
    # Track consecutive same counts — alert at 15 iterations (~5 min)
    if [ -z "$ports_stuck" ]; then ports_stuck=0; fi
    ports_stuck=$((ports_stuck + 1))
    if [ "$ports_stuck" -ge 15 ]; then
      alert "[PORTS] SCARD unchanged at $ports_count for 15 checks (~5 min) — possible Redis desync"
      ports_stuck=0
    fi
  else
    ports_stuck=0
  fi
  last_ports_count="$ports_count"

  # 3. sow-relay process count
  relay_count=$(pgrep sow-relay 2>/dev/null | wc -l | tr -d ' ')
  if [ "$relay_count" -eq 0 ]; then
    alert "[RELAY] Zero sow-relay processes running"
  fi
  if [ -n "$last_relay_count" ] && [ "$relay_count" -ne "$last_relay_count" ]; then
    log "[RELAY] Process count changed: $last_relay_count → $relay_count"
  fi
  last_relay_count="$relay_count"

  # 4. sow-server alive
  if ! pgrep sow-server >/dev/null 2>&1; then
    alert "[SERVER] sow-server process not found"
  fi

  # 5. Relay log files vs processes — zombie detection
  relay_logs=$(ls -1 /var/log/sow/relay_*.log 2>/dev/null | wc -l)
  if [ "$relay_logs" -gt "$relay_count" ] && [ "$relay_count" -gt 0 ]; then
    diff=$((relay_logs - relay_count))
    log "[RELAY] $diff more log files than processes — possible zombies"
  fi

  # 6. Redis relay keys vs processes — ghost detection
  ghost_keys=$($REDIS_CLI KEYS "sow:relay:*" 2>/dev/null | wc -l)
  if [ "$ghost_keys" -gt "$relay_count" ] && [ "$relay_count" -gt 0 ]; then
    diff=$((ghost_keys - relay_count))
    log "[RELAY] $diff more Redis sow:relay:* keys than processes — ghosts possible"
  fi

  # 7. Server log for CRITICAL/ERROR in last 20s
  if [ -f /var/log/sow/server.log ]; then
    recent_errors=$(tail -20 /var/log/sow/server.log | grep -cE '\[CRITICAL\]|\[REDIS\].*FAILED|\[REDIS\].*Cannot query' 2>/dev/null || echo 0)
    if [ "$recent_errors" -gt 0 ]; then
      log "[SERVER] $recent_errors CRITICAL/FAILED log lines in last 20s"
    fi
  fi
done
