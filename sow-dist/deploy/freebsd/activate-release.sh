#!/bin/sh
set -eu

id="${1:?release id required}"
version="${2:?version required}"
stage="${3:?stage path required}"

case "${id}:${version}" in
	*[!A-Za-z0-9._:-]*)
		echo "invalid release identity" >&2
		exit 2
		;;
esac

root="/srv/sow"
target="${root}/releases/${id}"
pending="${root}/releases/.${id}.pending"
lock="/var/run/sow-deploy.lock"

# Retry a shell expression up to N times with 500ms sleep between attempts.
retry() {
	local label=${1:?label}; local n=${2:?count}; shift 2
	local i=0
	while [ "$i" -lt "$n" ]; do
		if eval "$*"; then return 0; fi
		i=$((i + 1))
		[ "$i" -lt "$n" ] && sleep 0.5
	done
	echo "RETRY_FAIL: ${label} after ${n} attempts" >&2
	return 1
}
previous=""
activated=0
complete=0
db_changed=1
server_changed=1
relay_changed=1
maps_changed=1
version_changed=1
db_rc_changed=1
server_rc_changed=1
nginx_changed=1
db_touched=0
server_touched=0
nginx_touched=0

if ! mkdir "${lock}" 2>/dev/null; then
	echo "another deployment is active" >&2
	exit 3
fi

finish()
{
	status=$?
	trap - EXIT HUP INT TERM

	if [ "${complete}" -ne 1 ]; then
		for service_name in sow_database sow_server; do
			if [ -f "${lock}/${service_name}" ]; then
				install -o root -g wheel -m 0555 \
					"${lock}/${service_name}" "/usr/local/etc/rc.d/${service_name}"
			fi
		done
		if [ -f "${lock}/nginx.conf" ]; then
			install -o root -g wheel -m 0644 \
				"${lock}/nginx.conf" /usr/local/etc/nginx/nginx.conf
		fi

		if [ "${activated}" -eq 1 ] && [ -n "${previous}" ]; then
			echo "deployment failed; restoring ${previous}" >&2
			link="${root}/.current.rollback.$$"
			ln -s "${previous}" "${link}"
			mv -fh "${link}" "${root}/current"
		fi
		if [ "${db_touched}" -eq 1 ]; then
			service sow_database restart || true
		fi
		if [ "${server_touched}" -eq 1 ]; then
			service sow_server restart || true
		fi
		if [ "${nginx_touched}" -eq 1 ]; then
			nginx -t && service nginx reload || true
		fi
	fi

	rm -f "${lock}/sow_database" "${lock}/sow_server" "${lock}/nginx.conf"
	rmdir "${lock}" 2>/dev/null || true
	exit "${status}"
}
trap finish EXIT HUP INT TERM

# Remove links left inside a release by older activators that followed
# /srv/sow/current instead of replacing it.
find "${root}/releases" -mindepth 2 -maxdepth 2 -type l \
	-name '.current.*' -delete

test -f "${stage}/SHA256"
test -x "${stage}/bin/sow-database"
test -x "${stage}/bin/sow-server"
test -f "${stage}/web/play/index.html"
test -f "${stage}/web/game-manifest.json"
test -f "${stage}/maps/world/map.bin"
test -f "${stage}/ops/rc.d/sow_database"
test -f "${stage}/ops/rc.d/sow_server"
test -f "${stage}/ops/nginx.conf"
/bin/sh -n "${stage}/ops/rc.d/sow_database"
/bin/sh -n "${stage}/ops/rc.d/sow_server"
nginx -t -c "${stage}/ops/nginx.conf"

(cd "${stage}" && sha256sum --quiet -c SHA256)

if [ -e "${target}" ]; then
	cmp -s "${stage}/SHA256" "${target}/SHA256" ||
		{ echo "release id already exists with different content" >&2; exit 4; }
else
	rm -rf "${pending}"
	mkdir "${pending}"
	cp -Rp "${stage}/." "${pending}/"
	chown -R root:sow "${pending}"
	find "${pending}" -type d -exec chmod 0755 {} +
	find "${pending}" -type f -exec chmod 0644 {} +
	chmod 0550 "${pending}"/bin/*
	(cd "${pending}" && sha256sum --quiet -c SHA256)
	mv "${pending}" "${target}"
fi

previous="$(readlink "${root}/current" 2>/dev/null || true)"
old=""
case "${previous}" in
	releases/*)
		old="${root}/${previous}"
		;;
esac

same_file()
{
	new_hash="$(awk -v file="$1" '$2 == file { print $1 }' "${target}/SHA256")"
	old_hash="$(awk -v file="$1" '$2 == file { print $1 }' "${old}/SHA256" 2>/dev/null || true)"
	[ -n "${new_hash}" ] && [ "${new_hash}" = "${old_hash}" ]
}

same_tree()
{
	new_hash="$(awk -v prefix="$1" 'index($2, prefix) == 1 { print }' "${target}/SHA256" | sha256 -q)"
	old_hash="$(awk -v prefix="$1" 'index($2, prefix) == 1 { print }' "${old}/SHA256" 2>/dev/null | sha256 -q)"
	[ "${new_hash}" = "${old_hash}" ]
}

if [ -n "${old}" ] && [ -f "${old}/SHA256" ]; then
	same_file "bin/sow-database" && db_changed=0
	same_file "bin/sow-server" && server_changed=0
	same_file "bin/sow-relay" && relay_changed=0
	same_tree "maps/" && maps_changed=0
	same_file "VERSION" && version_changed=0
fi

cmp -s "${target}/ops/rc.d/sow_database" /usr/local/etc/rc.d/sow_database &&
	db_rc_changed=0
cmp -s "${target}/ops/rc.d/sow_server" /usr/local/etc/rc.d/sow_server &&
	server_rc_changed=0
cmp -s "${target}/ops/nginx.conf" /usr/local/etc/nginx/nginx.conf &&
	nginx_changed=0

if [ "${db_rc_changed}" -eq 1 ]; then
	[ -f /usr/local/etc/rc.d/sow_database ] &&
		cp /usr/local/etc/rc.d/sow_database "${lock}/sow_database"
	install -o root -g wheel -m 0555 \
		"${target}/ops/rc.d/sow_database" /usr/local/etc/rc.d/sow_database
fi
if [ "${server_rc_changed}" -eq 1 ]; then
	[ -f /usr/local/etc/rc.d/sow_server ] &&
		cp /usr/local/etc/rc.d/sow_server "${lock}/sow_server"
	install -o root -g wheel -m 0555 \
		"${target}/ops/rc.d/sow_server" /usr/local/etc/rc.d/sow_server
fi
if [ "${nginx_changed}" -eq 1 ]; then
	[ -f /usr/local/etc/nginx/nginx.conf ] &&
		cp /usr/local/etc/nginx/nginx.conf "${lock}/nginx.conf"
	install -o root -g wheel -m 0644 \
		"${target}/ops/nginx.conf" /usr/local/etc/nginx/nginx.conf
fi

if [ "${previous}" != "releases/${id}" ]; then
	link="${root}/.current.$$"
	ln -s "releases/${id}" "${link}"
	mv -fh "${link}" "${root}/current"
	activated=1
fi

if [ "${db_changed}" -eq 1 ] || [ "${db_rc_changed}" -eq 1 ]; then
	db_touched=1
	service sow_database restart
fi
if [ "${server_changed}" -eq 1 ] || [ "${maps_changed}" -eq 1 ] ||
	[ "${version_changed}" -eq 1 ] || [ "${server_rc_changed}" -eq 1 ]; then
	server_touched=1
	service sow_server restart
fi
if [ "${nginx_changed}" -eq 1 ]; then
	nginx_touched=1
	nginx -t
	service nginx reload
fi

echo "changes: db=${db_changed} server=${server_changed} relay=${relay_changed} maps=${maps_changed} nginx=${nginx_changed} version=${version_changed}"

service sow_database status
service sow_server status
service valkey status
service nginx status

retry valkey 5 "valkey-cli -h 127.0.0.1 ping | grep -qx PONG"
retry sock_25564 5 "sockstat -4 -l | grep -q '127.0.0.1:25564'"
retry sock_25566 5 "sockstat -4 -l | grep -q '127.0.0.1:25566'"
retry sock_25585 5 "sockstat -4 -l | grep -q '127.0.0.1:25585'"
retry health 5 "fetch -qo /dev/null http://127.0.0.1/health"
retry root 5 "fetch -qo /dev/null http://127.0.0.1/"
retry play 5 "fetch -qo /dev/null http://127.0.0.1/play/"
retry maps 5 "fetch -qo /dev/null http://127.0.0.1/maps/world/map.bin"
retry manifest 5 "manifest=\$(fetch -qo- http://127.0.0.1/game-manifest.json 2>/dev/null) && echo \"\$manifest\" | grep -q '\"version\":\"${version}\"'"

complete=1
if [ "${activated}" -eq 1 ]; then
	echo "activated ${id}"
else
	echo "verified ${id}; no activation needed"
fi
