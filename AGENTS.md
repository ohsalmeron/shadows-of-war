# Agent Instructions

## Core Rules

- Execute **exactly** what the user asks. Nothing more, nothing less.
- Be direct and concise. When asked for a short answer, give one. No preamble, no postamble, no explanation unless asked.
- Professional, respectful, intelligent. Follow instructions the first time.
- The user's message is the only agenda. Everything else is noise.
- Infrastructure decisions require explicit user approval first.
- Firewall, PF, NSG, IP rules require explicit authorization before existing.
- In plan mode: the user sets the pace. You respond, you wait, you execute when told.
- If something requires concept explanation, explain first, ensure understanding, execute only after confirmation.
- Ask before acting. Certainty comes from asking, not assuming.
- The prompt is always clear. Execute. No second-guessing.
- Questions must be ONLY about what the user asked. Not about side tangents, not about what you think they might need.
- Unsolicited opinions, suggestions, brainstorming, or ideas are detrimental unless explicitly requested. If they wanted input, they'd ask for it.

## Lessons Learned

- An IP hardcoded in PF without authorization caused total SSH loss to sow-prod-freebsd
- "Admin user" in Azure FreeBSD requires real sudo verification — `az vm user update` reports "Succeeded" without making actual changes on FreeBSD
- Root cause fix > ten workarounds
- Verify > assume. Always.
- **The pipeline is law, not an option.** If `./sow p` fails, fix the pipeline, don't bypass it with manual commands. No "urgency", "workaround", "just this once" justifies a bypass. Every bypass corrupts state and makes the unpredictable what was once reliable.

## Verification Rules

- FIRST command on any new VM: verify sudo/root works. Show output.
- Every Azure command must be confirmed with a second independent command showing the result.
- If a step fails, that step is the absolute priority until resolved.
- Every new resource exists first as a user-approved plan.

## Anti-Brick Checklist (Azure FreeBSD VMs)

- `pw moduser root -h 0` (password for serial console)
- `pf_enable=NO`, `pflog_enable=NO`, `pfctl -d`
- `NOPASSWD` sudo for admin user (FreeBSD waagent doesn't manage sudo well)
- Verify post-deploy: `sudo id`, `pfctl -si`, `grep autoboot_delay /boot/loader.conf`

## Production

- VM: FreeBSD 15.1-RELEASE, Azure, Standard_D2als_v6, 2 vCPU / 4 GiB.
- OS pool: `zroot` (30 GiB). No data disk.
- Datasets: `zroot/sow` → `/srv/sow`, `zroot/sow/releases` → `/srv/sow/releases`, `zroot/sow/state` → `/var/db/sow`, `zroot/sow/log` → `/var/log/sow` (8G quota).
- NSG is the only network barrier. PF disabled permanently.
- `autoboot_delay=5` for serial console recovery.

### Deploy Pipeline (`./sow p`)
1. Preflight: check releases dir + sudo + sha256sum on VM
2. Build: WASM local, FreeBSD binaries via rsync+cargo on build VM
3. Assemble content-addressed release (SHA-256)
4. Upload via rsync + SCP activate-release.sh
5. Activator: verify, install rc.d/nginx, activate symlink, restart services, verify health
6. Public verification (optional, `SOW_REQUIRE_PUBLIC`)

### Backfill (`./sow b`)
- Build on FreeBSD VM, deploy to remote host as rc.d service.
- Binary at `/usr/local/libexec/sow-backfill`, config at `/usr/local/etc/sow-backfill.conf`.

### Service users
- `sowserver`: game server + relays
- `sowdb`: database
- Valkey as `valkey`, Nginx as `www`

## Agent Audit (review what previous agents did)

```sh
# 1. List recent sessions
opencode session list

# 2. Export session as JSON (filter messages with jq)
opencode export <session_id> | jq '.messages[] | {role: .data.role, text: .data.text}' | less

# 3. Check for pipeline bypasses in bash commands
opencode export <session_id> | jq '.messages[] | select(.data.tool == "bash") | .data.text' | grep -E "ssh.*service |scp |ln -sfn"
```

Look for: `ssh sow 'service ...'` manual, `scp` direct, `ln -sfn`, env var overrides in commands — these are pipeline bypasses.

## Authorized Debugging (read-only, never deploy)

```sh
# Service status
ssh sow 'sudo service sow_server status'
ssh sow 'sudo service sow_database status'

# Logs
ssh sow 'sudo tail -50 /var/log/sow/server.log'
ssh sow 'sudo tail -50 /var/log/sow/database.log'

# Active connections
ssh sow 'sudo sockstat -4l | grep -E "sow_|relay"'

# Processes
ssh sow 'ps aux | grep -E "sow_server|sow_database|relay"'

# Current active release
ssh sow 'readlink /usr/local/sow/current'

# Health
curl -s http://20.7.77.78/health
curl -s http://20.7.77.78/admin/lobbies

# Remote backfill (IONOS)
ssh ionos 'ps aux | grep sow-backfill'
ssh ionos 'tail -50 /var/log/sow/backfill.log'
```

**Clear line:** if the command modifies files, restarts services, or moves releases, it's not debug — it's deploy. That goes through the pipeline.

## Absolute Rules

- The pipeline (`./sow p`, `./sow b`) is the ONLY deploy interface. No manual activations, direct scp, env var overrides.
- **If the pipeline fails, fix the pipeline, don't bypass it.** No "urgency", "workaround", "just this once". Zero exceptions.
- **Disobedience is not acceptable.** The agent does not decide when to follow or skip rules. If the user says "use the pipeline", use the pipeline. If they say "don't do X", don't do X. Period.
- Never hardcode infrastructure paths in open source code. Use `.env` or environment variables.
- Zero overhead to the user: no sleeps, waits, polling, or artificial latency.
- Nothing fails silently. Every Redis operation that fails must log the error.
- Only commit when explicitly asked.
- Ask before assuming. No override without confirmation.

### Backfill
`./sow b`
