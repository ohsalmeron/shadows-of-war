#!/usr/bin/env python3
import sys
import os
import time
import select
import pty
import subprocess

def get_pub_key():
    key_path = "/home/YOUR_USER/.ssh/id_ed25519.pub"
    if os.path.exists(key_path):
        with open(key_path, "r") as f:
            return f.read().strip()
    else:
        print("Error: SSH public key not found at", key_path)
        sys.exit(1)

def main():
    pub_key = get_pub_key()
    print("SSH Public Key Loaded successfully.")
    
    # Create a master/slave pseudo-terminal pair (fake a controlling TTY)
    master_fd, slave_fd = pty.openpty()
    
    cmd = ["sudo", "virsh", "console", "YOUR_VM_NAME"]
    print(f"Spawning via pseudo-TTY: {' '.join(cmd)}")
    
    proc = subprocess.Popen(
        cmd,
        stdin=slave_fd,
        stdout=slave_fd,
        stderr=slave_fd,
        close_fds=True,
        preexec_fn=os.setsid # Create a new session group
    )
    
    # Close the slave descriptor in the parent process
    os.close(slave_fd)
    
    # Set master descriptor to non-blocking
    os.set_blocking(master_fd, False)
    
    buffer = ""
    logged_in = False
    config_done = False
    
    # Send a few Carriage Returns to wake up the serial line
    print("Waking up console...")
    os.write(master_fd, b"\r\r\r")
    time.sleep(1)
    
    steps = [
        ("mkdir -p /root/.ssh", "#"),
        (f"echo '{pub_key}' > /root/.ssh/authorized_keys", "#"),
        ("chmod 700 /root/.ssh", "#"),
        ("chmod 600 /root/.ssh/authorized_keys", "#"),
        ("sysrc sshd_enable=\"YES\"", "#"),
        ("echo 'PermitRootLogin yes' >> /etc/ssh/sshd_config", "#"),
        ("service sshd restart", "#"),
        ("echo 'BOOTSTRAP_COMPLETE'", "BOOTSTRAP_COMPLETE"),
    ]
    
    step_idx = 0
    last_wake = time.time()
    
    while True:
        # Check if child died
        if proc.poll() is not None:
            print("\nError: virsh console process died unexpectedly.")
            break
            
        r, _, _ = select.select([master_fd], [], [], 1)
        if not r:
            # Wake up if idle
            if time.time() - last_wake > 3:
                print("\nSending CR to wake up console...")
                os.write(master_fd, b"\r")
                last_wake = time.time()
            continue
            
        try:
            chunk = os.read(master_fd, 1024).decode('utf-8', errors='ignore')
            if not chunk:
                break
            buffer += chunk
            sys.stdout.write(chunk)
            sys.stdout.flush()
        except Exception as e:
            print("Error reading chunk:", e)
            break
            
        if not logged_in:
            if "login:" in buffer or "Amnesiac" in buffer:
                print("\n[DETECTED LOGIN PROMPT] Logging in as root...")
                os.write(master_fd, b"root\r")
                buffer = ""
                time.sleep(1)
                logged_in = True
                last_wake = time.time()
            elif "#" in buffer or "root@" in buffer:
                print("\n[DETECTED ACTIVE SHELL PROMPT]")
                logged_in = True
                buffer = ""
                
        if logged_in and not config_done:
            # Execute step by step
            if step_idx < len(steps):
                cmd_line, expected_response = steps[step_idx]
                # Check if prompt is active
                if expected_response in buffer or buffer.strip().endswith("#") or buffer.strip().endswith("root@:"):
                    print(f"\n[RUNNING STEP {step_idx+1}/{len(steps)}] {cmd_line}")
                    os.write(master_fd, f"{cmd_line}\r".encode('utf-8'))
                    buffer = ""
                    step_idx += 1
                    time.sleep(0.5)
                    last_wake = time.time()
            else:
                print("\n[SUCCESS] FreeBSD compiler VM configured successfully offline!")
                config_done = True
                os.write(master_fd, b"exit\r")
                break
                
    # Close master FD and terminate child
    os.close(master_fd)
    proc.terminate()
    proc.wait()
    print("Serial interaction complete.")

if __name__ == "__main__":
    main()
