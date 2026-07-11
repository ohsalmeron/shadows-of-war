#!/usr/local/bin/python3
# ponytail: Simple high-performance FreeBSD bandwidth monitor for 1Gbps limit
import subprocess
import sys
import time

INTERFACE = "vtnet0"
THRESHOLD_MBPS = 900  # Alert at 90% of 1Gbps

def send_alert(message):
    print(f"[ALERT] {message}", file=sys.stderr)
    try:
        with open("/var/log/bandwidth_monitor.log", "a") as f:
            f.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')} - {message}\n")
    except Exception as e:
        print(f"Failed to write log: {e}", file=sys.stderr)

def main():
    print(f"Starting bandwidth monitor on {INTERFACE} (Threshold: {THRESHOLD_MBPS} Mbps)...")
    proc = subprocess.Popen(
        ["netstat", "-I", INTERFACE, "-b", "-w", "5"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    # Skip the first two header lines
    for _ in range(2):
        proc.stdout.readline()
        
    while True:
        line = proc.stdout.readline()
        if not line:
            break
        parts = line.split()
        if len(parts) >= 7:
            try:
                bytes_in = int(parts[3])
                bytes_out = int(parts[6])
                mbps = ((bytes_in + bytes_out) * 8 / 5) / 1000000.0
                if mbps > THRESHOLD_MBPS:
                    send_alert(f"IONOS VPS Bandwidth Saturation Alert: {mbps:.2f} Mbps exceeded threshold of {THRESHOLD_MBPS} Mbps!")
            except ValueError:
                continue

if __name__ == "__main__":
    main()
