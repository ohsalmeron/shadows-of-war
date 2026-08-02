//! M1a — Zero-copy TCP echo on the F-Stack `ff_run` thread (no tokio).
//!
//! Receive path: `ff_zc_recv` → `ff_zc_mbuf_segment` (0 copies; segments alias the DPDK NIC buffer).
//! Send path:    `ff_zc_mbuf_get` → per-segment `ff_zc_mbuf_write` (1 copy; hardware floor) → `ff_zc_send`.
//!
//! Run (TAP, SSH-safe dev loop):
//!   FSTACK_TAP=1 ./echo --conf echo-tap.ini --proc-type=primary --proc-id=0
//! Run (physical VF):
//!   ./echo --conf echo-vf.ini --proc-type=primary --proc-id=0
//!
//! `--proc-id` selects this process's RSS queue/core (see config.ini `lcore_mask`).

use fstack_bridge::{ev_set, kevent};
use fstack_bridge::ffi::{ff_zc_mbuf, EV_ADD, EV_EOF, EV_ERROR, EVFILT_READ};
use libc::{
    c_int, c_void, sockaddr_in, socklen_t, AF_INET, INADDR_ANY, SOCK_STREAM, SOL_SOCKET,
    SO_REUSEADDR, FIONBIO,
};
use std::ffi::CString;
use std::mem;
use std::ptr;

const MAX_EVENTS: usize = 512;
const LISTEN_PORT: u16 = 8080;

// Globals are sound: F-Stack drives a single loop thread; loop_cb is the only accessor.
static mut KQ: c_int = -1;
static mut LISTEN_FD: c_int = -1;
static mut ACCEPTS: u64 = 0;
static mut ECHOES: u64 = 0;
static mut RECV_BYTES: u64 = 0;
static mut LAST_STATS_AT: u64 = 0;

fn main() {
    let prog_args: Vec<CString> = std::env::args()
        .filter(|a| !a.starts_with("--fstack-")) // keep --conf/--proc-type/--proc-id for ff_load_config
        .map(|a| CString::new(a).unwrap())
        .collect();

    // TAP dev loop: inject --no-pci + net_tap0 vdev (FSTACK_TAP=1). Empty for the physical VF.
    let mut extra_eal: Vec<&str> = Vec::new();
    if std::env::var("FSTACK_TAP").ok().as_deref() == Some("1") {
        extra_eal.push("--no-pci");
        extra_eal.push("--iova-mode=va"); // TAP vdev needs VA IOVA (no physical NIC for PA)
        extra_eal.push("--vdev=net_tap0,iface=tap0");
        // Azure: vdev_netvsc's custom scan auto-injects net_vdev_netvsc devargs and its
        // probe would touch the synthetic NIC (eth0) — SSH death. `ignore=1` makes the
        // probe a no-op and stops the scan callback from adding its own devargs.
        extra_eal.push("--vdev=net_vdev_netvsc,ignore=1");
        eprintln!("[echo] TAP mode (--no-pci + --iova-mode=va + net_tap0, netvsc ignored)");
    } else {
        eprintln!("[echo] physical VF mode (config.ini [dpdk] allow=)");
    }

    unsafe {
        if let Err(code) = fstack_bridge::init(&prog_args, &extra_eal) {
            eprintln!("[echo] init failed (code={})", code);
            std::process::exit(1);
        }

        // Report this process's RSS queue assignment.
        let (mut pid, mut qid, mut nbq, mut reta) = (0u16, 0u16, 0u16, 0u16);
        if fstack_bridge::ff_rss_self_queue_info(&mut pid, &mut qid, &mut nbq, &mut reta) == 0 {
            eprintln!(
                "[BOOT] proc_id={} queue_id={} nb_queues={} reta_size={}",
                pid, qid, nbq, reta
            );
        }

        KQ = fstack_bridge::ff_kqueue();
        if KQ < 0 {
            eprintln!("[echo] ff_kqueue failed");
            std::process::exit(1);
        }

        LISTEN_FD = fstack_bridge::ff_socket(AF_INET, SOCK_STREAM, 0);
        if LISTEN_FD < 0 {
            eprintln!("[echo] ff_socket failed");
            std::process::exit(1);
        }

        let on: c_int = 1;
        fstack_bridge::ff_setsockopt(
            LISTEN_FD,
            SOL_SOCKET,
            SO_REUSEADDR,
            &on as *const _ as *const c_void,
            mem::size_of::<c_int>() as socklen_t,
        );
        fstack_bridge::ff_ioctl(LISTEN_FD, FIONBIO as libc::c_ulong, &on);

        let mut addr: sockaddr_in = mem::zeroed();
        addr.sin_family = AF_INET as u16;
        addr.sin_port = LISTEN_PORT.to_be();
        addr.sin_addr.s_addr = INADDR_ANY;
        if fstack_bridge::ff_bind(
            LISTEN_FD,
            &addr,
            mem::size_of::<sockaddr_in>() as socklen_t,
        ) < 0
        {
            eprintln!("[echo] ff_bind :{} failed", LISTEN_PORT);
            std::process::exit(1);
        }
        if fstack_bridge::ff_listen(LISTEN_FD, MAX_EVENTS as c_int) < 0 {
            eprintln!("[echo] ff_listen failed");
            std::process::exit(1);
        }

        let mut kev: kevent = mem::zeroed();
        ev_set(&mut kev, LISTEN_FD as usize, EVFILT_READ, EV_ADD, 0, MAX_EVENTS as i64, ptr::null_mut());
        fstack_bridge::ff_kevent(KQ, &kev, 1, ptr::null_mut(), 0, ptr::null());

        eprintln!("[BOOT] listening on :{}, entering ff_run", LISTEN_PORT);
        fstack_bridge::ff_run(loop_cb, ptr::null_mut());
    }
}

unsafe extern "C" fn loop_cb(_arg: *mut c_void) -> c_int {
    let mut events: [kevent; MAX_EVENTS] = mem::zeroed();
    let nevents = fstack_bridge::ff_kevent(
        KQ,
        ptr::null(),
        0,
        events.as_mut_ptr(),
        MAX_EVENTS as c_int,
        ptr::null(),
    );
    if nevents < 0 {
        return -1;
    }

    for i in 0..nevents as usize {
        let ev = &events[i];
        if ev.flags & EV_ERROR != 0 {
            continue;
        }
        let fd = ev.ident as c_int;

        if ev.flags & EV_EOF != 0 {
            fstack_bridge::ff_close(fd);
            continue;
        }

        if fd == LISTEN_FD {
            // Accept all pending connections.
            let mut available = ev.data as i32;
            while available > 0 {
                let mut peer: sockaddr_in = mem::zeroed();
                let mut peerlen: socklen_t = mem::size_of::<sockaddr_in>() as socklen_t;
                let nfd = fstack_bridge::ff_accept(LISTEN_FD, &mut peer, &mut peerlen);
                if nfd < 0 {
                    break;
                }
                let on: c_int = 1;
                fstack_bridge::ff_ioctl(nfd, FIONBIO as libc::c_ulong, &on);
                ACCEPTS += 1;
                let mut kev: kevent = mem::zeroed();
                ev_set(&mut kev, nfd as usize, EVFILT_READ, EV_ADD, 0, 0, ptr::null_mut());
                fstack_bridge::ff_kevent(KQ, &kev, 1, ptr::null_mut(), 0, ptr::null());
                available -= 1;
            }
        } else if ev.filter == EVFILT_READ {
            echo_zero_copy(fd);
        }
    }

    maybe_stats();
    0
}

/// Zero-copy echo: `ff_zc_recv` (segments alias DPDK buffer) → re-pack into a send mbuf
/// via `ff_zc_mbuf_write` (the single fundamental copy) → `ff_zc_send`.
unsafe fn echo_zero_copy(fd: c_int) {
    let mut zm: ff_zc_mbuf = mem::zeroed();
    let n = fstack_bridge::ff_zc_recv(fd, &mut zm, 65536);
    if n > 0 {
        RECV_BYTES += n as u64;
        let total = zm.len;

        let mut sendbuf: ff_zc_mbuf = mem::zeroed();
        if fstack_bridge::ff_zc_mbuf_get(&mut sendbuf, total) == 0 {
            let mut seg_ptr: *mut c_void = ptr::null_mut();
            let mut seg_len: c_int = 0;
            while fstack_bridge::ff_zc_mbuf_segment(&mut zm, &mut seg_ptr, &mut seg_len) > 0 {
                if seg_len > 0 && !seg_ptr.is_null() {
                    fstack_bridge::ff_zc_mbuf_write(&mut sendbuf, seg_ptr as *const libc::c_char, seg_len);
                }
            }
            fstack_bridge::ff_zc_send(fd, sendbuf.bsd_mbuf, total as libc::size_t);
            ECHOES += 1;
        }
        fstack_bridge::ff_zc_recv_free(&mut zm);
    } else if n == 0 {
        // peer half-closed
        fstack_bridge::ff_close(fd);
    }
    // n < 0 => EAGAIN/EWOULDBLOCK: retry next iteration.
}

unsafe fn maybe_stats() {
    if ACCEPTS.saturating_sub(LAST_STATS_AT) >= 25 {
        LAST_STATS_AT = ACCEPTS;
        eprintln!(
            "[stats] accepts={} echoes={} recv_bytes={}",
            ACCEPTS, ECHOES, RECV_BYTES
        );
    }
}
