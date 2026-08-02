#!/usr/bin/env bash
# Recreates the DPDK dev VM (sow-dev-2nic) from scratch.
# The whole F-Stack/DPDK validation environment depends on this exact topology:
#   2 NICs with Accelerated Networking, the data NIC becomes the DPDK VF
#   (Mellanox 77e8:00:02.0) that sow-relay binds with F-Stack on :80.
#
# Verified against the live environment on 2026-08-02. Idempotent: safe to rerun.
#
# Usage: ./create_dpdk_vm.sh
# Requires: az CLI logged in (az login), SSH key at $SSH_KEY_PUB (default ~/.ssh/id_rsa.pub)

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration (mirrors the live VM exactly)
# ---------------------------------------------------------------------------
RG="rg-sow-dev-eastus2"
LOC="eastus2"
VNET="sow-devVNET"
VNET_PREFIX="10.0.0.0/16"
SUBNET_MGMT="mgmt"
SUBNET_MGMT_PREFIX="10.0.1.0/24"
SUBNET_DATA="data"
SUBNET_DATA_PREFIX="10.0.2.0/24"
NSG="sow-devNSG"
PIP_MGMT="sow-dev-mgmt-pip"
PIP_DATA="sow-dev-data-pip"
NIC_MGMT="sow-dev-mgmt"
NIC_DATA="sow-dev-data"
VM="sow-dev-2nic"
VM_SIZE="Standard_D4als_v6"            # 4 vCPU, AMD, Accelerated Networking capable
IMAGE="Canonical:ubuntu-24_04-lts:server:latest"
OS_DISK_SKU="StandardSSD_LRS"
OS_DISK_SIZE_GB="30"
SSH_KEY_PUB="${SSH_KEY_PUB:-$HOME/.ssh/id_rsa.pub}"

# ---------------------------------------------------------------------------
# 1. Resource group
# ---------------------------------------------------------------------------
az group create --name "$RG" --location "$LOC" -o none

# ---------------------------------------------------------------------------
# 2. VNet + subnets
# ---------------------------------------------------------------------------
az network vnet create -g "$RG" -n "$VNET" --address-prefix "$VNET_PREFIX" \
  --subnet-name "$SUBNET_MGMT" --subnet-prefix "$SUBNET_MGMT_PREFIX" -o none
az network vnet subnet create -g "$RG" --vnet-name "$VNET" -n "$SUBNET_DATA" \
  --address-prefix "$SUBNET_DATA_PREFIX" -o none

# ---------------------------------------------------------------------------
# 3. NSG (associated to the mgmt NIC; data NIC has no NSG)
# ---------------------------------------------------------------------------
az network nsg create -g "$RG" -n "$NSG" -l "$LOC" -o none
az network nsg rule create -g "$RG" --nsg-name "$NSG" -n tcp80 \
  --priority 800 --direction Inbound --access Allow --protocol Tcp \
  --source-address-prefixes '*' --destination-port-ranges 80 -o none
az network nsg rule create -g "$RG" --nsg-name "$NSG" -n ssh22 \
  --priority 1000 --direction Inbound --access Allow --protocol Tcp \
  --source-address-prefixes '*' --destination-port-ranges 22 -o none
# Orchestrator WS is NOT exposed publicly (127.0.0.1 daemon). To expose:
# az network nsg rule create -g "$RG" --nsg-name "$NSG" -n tcp25565 \
#   --priority 900 --direction Inbound --access Allow --protocol Tcp \
#   --source-address-prefixes '*' --destination-port-ranges 25565 -o none

# ---------------------------------------------------------------------------
# 4. Public IPs (static; data PIP is the relay data-plane address)
# ---------------------------------------------------------------------------
az network public-ip create -g "$RG" -n "$PIP_MGMT" --allocation-method Static -o none
az network public-ip create -g "$RG" -n "$PIP_DATA" --allocation-method Static -o none

# ---------------------------------------------------------------------------
# 5. NICs with Accelerated Networking
# ---------------------------------------------------------------------------
az network nic create -g "$RG" -n "$NIC_MGMT" \
  --vnet-name "$VNET" --subnet "$SUBNET_MGMT" --public-ip-address "$PIP_MGMT" \
  --network-security-group "$NSG" --accelerated-networking true -o none
az network nic create -g "$RG" -n "$NIC_DATA" \
  --vnet-name "$VNET" --subnet "$SUBNET_DATA" --public-ip-address "$PIP_DATA" \
  --accelerated-networking true -o none

# ---------------------------------------------------------------------------
# 6. VM (both NICs, D4als_v6, Ubuntu 24.04)
# ---------------------------------------------------------------------------
az vm create -g "$RG" -n "$VM" \
  --image "$IMAGE" \
  --size "$VM_SIZE" \
  --nics "$NIC_MGMT" "$NIC_DATA" \
  --admin-username azureuser \
  --ssh-key-values "$SSH_KEY_PUB" \
  --os-disk-size-gb "$OS_DISK_SIZE_GB" \
  --storage-sku "$OS_DISK_SKU" \
  -o none

MGMT_IP=$(az network public-ip show -g "$RG" -n "$PIP_MGMT" --query ipAddress -o tsv)
DATA_IP=$(az network public-ip show -g "$RG" -n "$PIP_DATA" --query ipAddress -o tsv)
echo "VM ready: ssh azureuser@${MGMT_IP}  (mgmt) / ${DATA_IP} (data)"

cat <<'POST'
--------------------------------------------------------------------------
POST-DEPLOY REQUIRED (DPDK specifics; do not skip):
  1. hugepages (F-Stack needs 1GB pages):
       echo 'vm.nr_hugepages=1024' | sudo tee /etc/sysctl.d/99-hugepages.conf
       echo 'hugepagesz=1G' | sudo tee -a /etc/sysctl.d/99-hugepages.conf
       sudo sysctl -p /etc/sysctl.d/99-hugepages.conf
     If 1G pages are unavailable in the VM: 2MB pages also work:
       echo 'vm.nr_hugepages=1024' > /etc/sysctl.d/99-hugepages.conf && sysctl -p
  2. F-Stack userspace lib (FreeBSD kernel in userspace, compiled for Linux):
       scp libfstack.a.1.26 from the old VM (/usr/local/lib/) to /usr/local/lib/
       ldconfig  (or set LD_LIBRARY_PATH=/usr/local/lib)
  3. DPDK binding: the DATA NIC VF must be bound to vfio-pci:
       sudo dpdk-devbind.py --bind=vfio-pci <pci_addr>   # e.g. 77e8:00:02.0
       (verify: dpdk-devbind.py -s | grep <pci_addr>)
  4. sow-relay config lives in fstack-bridge/echo-vf.ini (in this repo) with:
       allow=<vf-pci>, port0 addr=10.0.2.4 (the DATA NIC private IP)
  5. Redis + workspace:
       sudo apt-get install -y redis-server && sudo systemctl enable redis-server
       rsync this workspace to ~/shadows-of-war/
  6. Systemd units: sow-relay.service (root, --conf echo-vf.ini
       --proc-type=primary --proc-id=0) then sow-server.service
       (SOW_RELAY_HOST=<DATA_IP> SOW_RELAY_MGMT_URL=http://127.0.0.1:8080).
POST
