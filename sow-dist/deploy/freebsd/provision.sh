#!/bin/sh
# FreeBSD Azure provisioning — runs as root on first boot via nuage-init
#
# Usage: pass via --custom-data at VM creation time.
# Inject YOUR_ROOT_PASSWORD and YOUR_ADMIN_USER at deploy time.
# Never commit passwords to this file or the repository.

if [ -z "${ROOT_PW:-}" ]; then
    echo "ROOT_PW not set — skipping root password (serial console unavailable)"
else
    echo "$ROOT_PW" | pw moduser root -h 0
fi

ADMIN="${ADMIN_USER:-fixer}"
pw moduser "$ADMIN" -G wheel,operator 2>/dev/null || pw useradd "$ADMIN" -G wheel,operator -m -s /bin/sh
mkdir -p /usr/local/etc/sudoers.d
echo "$ADMIN ALL=(ALL) NOPASSWD: ALL" > "/usr/local/etc/sudoers.d/$ADMIN"
chmod 440 "/usr/local/etc/sudoers.d/$ADMIN"

sed -i '' 's/autoboot_delay="-1"/autoboot_delay="5"/' /boot/loader.conf

sysrc pf_enable=NO
sysrc pflog_enable=NO
pfctl -d 2>/dev/null

sysrc sshd_enable=YES

touch /var/db/provisioning_done
