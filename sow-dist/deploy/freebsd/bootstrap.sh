#!/bin/sh
# FreeBSD production bootstrap — run once on a fresh VM.
# Requires: root (or sudo), internet access for pkg.
# After this script, `./sow p` handles all application deploys.
set -eu

echo "==> 1/4 ZFS datasets"
zfs create -o mountpoint=/srv/sow zroot/sow 2>/dev/null || true
zfs create -o mountpoint=/srv/sow/releases zroot/sow/releases 2>/dev/null || true
zfs create -o mountpoint=/var/db/sow -o exec=off -o setuid=off -o devices=off zroot/sow/state 2>/dev/null || true
zfs create -o mountpoint=/var/db/sow/replays -o exec=off -o setuid=off -o devices=off zroot/sow/state/replays 2>/dev/null || true
zfs create -o mountpoint=/var/log/sow -o exec=off -o setuid=off -o devices=off -o quota=8G zroot/sow/log 2>/dev/null || true
mkdir -p /srv/sow/releases
echo "  datasets ready"

echo "==> 2/4 Packages"
pkg install -y nginx valkey
echo "  packages installed"

echo "==> 3/4 Service users"
pw useradd sowserver -d /srv/sow -s /usr/sbin/nologin -c "SoW server" 2>/dev/null || true
pw useradd sowdb -d /var/db/sow -s /usr/sbin/nologin -c "SoW database" 2>/dev/null || true
chown -R sowserver:sowserver /srv/sow
chown -R sowdb:sowdb /var/db/sow
chown -R sowserver:sowserver /var/log/sow
echo "  users created"

echo "==> 4/4 Services"
sysrc nginx_enable=YES
sysrc valkey_enable=YES
sysrc sow_server_enable=YES
sysrc sow_database_enable=YES
echo "  services enabled"

echo "==> Valkey config"
sysrc valkey_bind="127.0.0.1 ::1"
sysrc valkey_protected_mode="yes"
echo "  valkey loopback-only"

echo ""
echo "Bootstrap complete."
echo "Next: run './sow p' from the workstation to deploy the application."
