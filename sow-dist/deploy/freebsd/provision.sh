#!/bin/sh
# RETIRED: Azure FreeBSD provisioning experiment.
#
# This file is intentionally fail-closed. It disabled PF and targeted the
# superseded Azure FreeBSD host; it is not valid for current IONOS production.
# All application deployment belongs to ./sow p.
echo "ERROR: retired Azure FreeBSD provisioner; use current IONOS tooling and ./sow p" >&2
exit 1
