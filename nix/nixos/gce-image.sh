#!/usr/bin/env bash
# Build a GCE-compatible NixOS disk image from flake.#vps
#
# Prerequisites: nix, gcloud SDK (gsutil), a GCS bucket, GCP project with Compute API.
#
# Usage:
#   PROJECT_ID=my-project BUCKET_NAME=my-bucket ./nix/nixos/gce-image.sh
#
# Then in GCE console: Create VM → Custom image → pick the uploaded image → attach static IP.
# First boot: ensure nix/nixos/vps/authorized_keys contains your SSH public key, then:
#   ./sow infra --host NEW_IP
#   ./sow prod -v && ./sow ptr -v
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT}"

PROJECT_ID="${PROJECT_ID:?set PROJECT_ID}"
BUCKET_NAME="${BUCKET_NAME:?set BUCKET_NAME}"
IMAGE_NAME="${IMAGE_NAME:-sow-nixos-vps}"
IMAGE_DATE="$(date -u +%Y%m%d)"

echo "==> Building NixOS system toplevel from .#nixosConfigurations.vps.config.system.build.toplevel"
TOPLEVEL="$(nix build --print-out-paths ".#nixosConfigurations.vps.config.system.build.toplevel")"

echo "==> Building GCE raw image via nixos-generators"
OUT="${ROOT}/dist/gce-${IMAGE_NAME}-${IMAGE_DATE}.raw.tar.gz"
mkdir -p "${ROOT}/dist"

nix run github:nix-community/nixos-generators -- \
  -f gce \
  -c "${ROOT}#nixosConfigurations.vps" \
  -o "${OUT%.tar.gz}"

echo "==> Uploading to gs://${BUCKET_NAME}/"
gsutil cp "${OUT}" "gs://${BUCKET_NAME}/"

GCE_IMAGE="projects/${PROJECT_ID}/global/images/${IMAGE_NAME}-${IMAGE_DATE}"
echo "==> Registering GCE image ${GCE_IMAGE}"
gcloud compute images create "${IMAGE_NAME}-${IMAGE_DATE}" \
  --project="${PROJECT_ID}" \
  --source-uri="gs://${BUCKET_NAME}/$(basename "${OUT}")" \
  --guest-os-features=UEFI_COMPATIBLE,VIRTIO_SCSI_MULTIQUEUE

echo "✅ GCE image ready: ${GCE_IMAGE}"
echo "   Create a VM from this image, attach your static IP, then run:"
echo "     ./sow infra --host YOUR_IP"
echo "     ./sow prod -v"
