#!/usr/bin/env bash
# Monitor Redis relay ports and active processes

echo "========================================"
echo "🕹️ Shadows of War - Relay Monitor"
echo "========================================"

# 1. Check Redis for registered ports
echo "-> Redis Registered Ports (sow:ports):"
redis-cli SMEMBERS sow:ports | sed 's/^/   - Port /'

echo ""
echo "-> Redis Active Lobbies (sow:relay:*):"
for key in $(redis-cli KEYS "sow:relay:*"); do
    lobby_id=$(redis-cli GET $key)
    ttl=$(redis-cli TTL $key)
    echo "   - $key -> Lobby $lobby_id (TTL: $ttl seconds)"
done

echo ""
echo "-> Running Relay Processes:"
ps aux | grep "[s]ow-relay" | awk '{print "   - PID: "$2", Command: "$11" "$12" "$13" "$14" "$15}'

echo "========================================"
