#!/usr/bin/env bash
set -euo pipefail

# Shadows of War — FreeBSD Compiler VM Orchestrator
# Adhering to the Elon Musk 5-Step Algorithm (Ponytail-Ultra Style)

VM_NAME="freebsd-compiler"
VM_DIR="/home/bizkit/vm"
IMAGE_PATH="/var/lib/libvirt/images/FreeBSD-15.1-STABLE-amd64-ufs.qcow2"
COMPRESSED_IMAGE="${VM_DIR}/FreeBSD-15.1-STABLE-amd64-ufs.qcow2.xz"
WORKSPACE_DIR="/home/bizkit/shadows-of-war"

get_vm_ip() {
    # Retrieve VM IP address from DHCP leases natively
    local ip=""
    # Try virsh domifaddr first
    ip=$(sudo virsh domifaddr "$VM_NAME" --source lease 2>/dev/null | grep -oE '192\.168\.[0-9]+\.[0-9]+' | head -n 1 || true)
    if [ -z "$ip" ]; then
        # Fallback to direct leases lookup
        if [ -f "/var/lib/libvirt/dnsmasq/default.leases" ]; then
            ip=$(grep -i "$(sudo virsh dominfo "$VM_NAME" 2>/dev/null | grep -i "uuid" | awk '{print $2}' || true)" /var/lib/libvirt/dnsmasq/default.leases | awk '{print $3}' | head -n 1 || true)
            if [ -z "$ip" ]; then
                # Fallback to scanning by name
                ip=$(grep -i "freebsd" /var/lib/libvirt/dnsmasq/default.leases | awk '{print $3}' | head -n 1 || true)
            fi
        fi
    fi
    echo "$ip"
}

case "${1:-}" in
    decompress)
        echo "==> Step 1 & 2: Extracting pre-built FreeBSD QCOW2 image..."
        if [ ! -f "$COMPRESSED_IMAGE" ]; then
            echo "Error: Compressed image not found at $COMPRESSED_IMAGE"
            exit 1
        fi
        if [ -f "$IMAGE_PATH" ]; then
            echo "Base QCOW2 image already decompressed."
        else
            xz -d -v -k "$COMPRESSED_IMAGE"
        fi
        echo "==> Resizing image disk to +25G..."
        qemu-img resize "$IMAGE_PATH" +25G
        ;;

    start)
        echo "==> Starting FreeBSD compiler VM with SPICE graphics..."
        if sudo virsh list --name | grep -q "^${VM_NAME}$"; then
            echo "VM is already running."
        else
            if sudo virsh list --all --name | grep -q "^${VM_NAME}$"; then
                sudo virsh start "$VM_NAME"
            else
                echo "Defining and installing virtual machine via KVM with SPICE..."
                sudo virt-install \
                  --name "$VM_NAME" \
                  --ram 6144 \
                  --vcpus 6 \
                  --disk "path=${IMAGE_PATH},format=qcow2" \
                  --import \
                  --os-variant freebsd14.0 \
                  --network network=default \
                  --graphics spice,listen=127.0.0.1 \
                  --noautoconsole
            fi
        fi
        ;;

    stop)
        echo "==> Stopping FreeBSD compiler VM cleanly..."
        sudo virsh shutdown "$VM_NAME" || sudo virsh destroy "$VM_NAME" || true
        ;;

    bootstrap)
        echo "==> Running bootstrap_ssh serial console automation..."
        # First ensure VM is running
        "$0" start
        
        # Run python serial agent
        sudo python3 "${VM_DIR}/bootstrap_ssh.py"
        
        # Fetch the IP address
        echo "==> Resolving FreeBSD VM IP address..."
        IP=""
        for i in {1..20}; do
            IP=$(get_vm_ip)
            if [ -n "$IP" ]; then
                break
            fi
            echo "Waiting for DHCP IP address (Attempt $i/20)..."
            sleep 2
        done
        
        if [ -z "$IP" ]; then
            echo "Error: Could not resolve guest IP address."
            exit 1
        fi
        echo "VM is online at IP: $IP"
        
        # Test SSH connection
        echo "==> Verifying passwordless SSH access..."
        ssh -o StrictHostKeyChecking=no -o ConnectTimeout=5 "root@${IP}" "uname -a"
        
        # Bootstrap compiler environment inside guest
        echo "==> Bootstrapping package manager and dependencies (Rust, Git, Clang)..."
        ssh -o StrictHostKeyChecking=no "root@${IP}" "
            env ASSUME_ALWAYS_YES=yes pkg update
            env ASSUME_ALWAYS_YES=yes pkg install -y rustup git clang gcc
            rustup-init -y --default-toolchain stable
        "
        echo "✅ FreeBSD VM is 100% Bootstrapped and Ready!"
        ;;

    compile)
        IP=$(get_vm_ip)
        if [ -z "$IP" ]; then
            echo "VM is offline or IP could not be resolved. Starting VM..."
            "$0" start
            sleep 5
            IP=$(get_vm_ip)
            if [ -z "$IP" ]; then
                echo "Error: VM IP could not be resolved. Please run bootstrap first."
                exit 1
            fi
        fi
        echo "Compiling on FreeBSD VM at $IP..."
        
        echo "==> Synchronizing shadows-of-war workspace to the VM..."
        rsync -avz --delete \
            --exclude "target" \
            --exclude ".git" \
            --exclude "node_modules" \
            --exclude "sow-web/node_modules" \
            "${WORKSPACE_DIR}/" "root@${IP}:/usr/src/shadows-of-war/"
            
        echo "==> Running Cargo production build in FreeBSD environment..."
        ssh "root@${IP}" "
            cd /usr/src/shadows-of-war
            . \$HOME/.cargo/env
            cargo build --release -p sow-server -p sow-relay
        "
        
        echo "==> Retrieving compiled FreeBSD binaries..."
        mkdir -p "${WORKSPACE_DIR}/target/x86_64-unknown-freebsd/release/"
        scp "root@${IP}:/usr/src/shadows-of-war/target/release/sow-server" "${WORKSPACE_DIR}/target/x86_64-unknown-freebsd/release/sow-server"
        scp "root@${IP}:/usr/src/shadows-of-war/target/release/sow-relay" "${WORKSPACE_DIR}/target/x86_64-unknown-freebsd/release/sow-relay"
        
        echo "🎉 SUCCESS: FreeBSD binaries retrieved locally at: ${WORKSPACE_DIR}/target/x86_64-unknown-freebsd/release/"
        ;;

    *)
        echo "Usage: $0 {decompress|start|stop|bootstrap|compile}"
        exit 1
        ;;
esac
