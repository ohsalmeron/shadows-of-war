//! # fstack-bridge
//!
//! Direct-FFI, zero-copy Rust bridge over F-Stack (DPDK + FreeBSD userspace TCP/IP stack).
//!
//! Single-thread-per-process model: all `ff_*` calls run on the `ff_run` loop thread.
//! Scale horizontally = N processes (one per RSS queue/core), launched with distinct
//! `--proc-id`, exactly like F-Stack's own examples.

// F-Stack exposes process-global mutable C state and the bridge confines all
// access to the single `ff_run` thread.  The references below are the explicit
// ABI boundary; replacing them requires redesigning the F-Stack integration.
#![allow(static_mut_refs)]

pub mod bridge;
pub mod ffi;
pub mod packet;

pub use packet::tcp_destination_queue;

pub use ffi::{
    dispatch_func_t, ev_set, ff_accept, ff_bind, ff_close, ff_dpdk_if_up, ff_dpdk_init,
    ff_freebsd_init, ff_ioctl, ff_kevent, ff_kqueue, ff_listen, ff_load_config, ff_read,
    ff_regist_packet_dispatcher, ff_rss_self_queue_info, ff_run, ff_setsockopt, ff_shutdown,
    ff_socket, ff_stop_run, ff_write, ff_zc_mbuf_get, ff_zc_mbuf_segment, ff_zc_mbuf_write,
    ff_zc_recv, ff_zc_recv_free, ff_zc_send, kevent, loop_func_t, EVFILT_READ, EVFILT_WRITE,
    EV_ADD, EV_CLEAR, EV_DELETE, EV_EOF, EV_ERROR, FF_DISPATCH_ERROR, FF_DISPATCH_RESPONSE,
};

use libc::{c_char, c_int};
use std::ffi::CString;
use std::ptr;

/// Initialize F-Stack with optional extra DPDK EAL arguments injected after `ff_load_config`.
///
/// This is the *manual* init sequence (the same `ff_init` performs internally), but lets us
/// inject EAL args that F-Stack's config parser cannot express — notably `--vdev=net_tap0,...`
/// and `--no-pci` for SSH-safe TAP development (F-Stack hardcodes `virtio_user` for its
/// `[vdevN]` section, see lib/ff_config.c:1191, so TAP must be injected).
///
/// `config_args` = the program argv (must include `--conf <file>`, `--proc-type`, `--proc-id`).
/// `extra_eal`   = extra EAL tokens, e.g. `["--no-pci", "--vdev=net_tap0,iface=tap0"]`
///                  (empty for the physical VF — the `[dpdk] allow=` line handles it).
///
/// MUST be called once, before any other `ff_*` call, on the thread that will later drive
/// `ff_run` (the same thread becomes the loop thread).
///
/// # Safety
/// F-Stack init touches global singletons (EAL, the FreeBSD stack) and is not re-entrant.
pub unsafe fn init(config_args: &[CString], extra_eal: &[&str]) -> Result<(), i32> {
    // Build the argv vector for ff_load_config (program + --conf ... --proc-type ... --proc-id).
    let mut argv: Vec<*mut c_char> = config_args
        .iter()
        .map(|s| s.as_ptr() as *mut c_char)
        .collect();
    argv.push(ptr::null_mut());
    let argc = (argv.len() - 1) as c_int;

    if ff_load_config(argc, argv.as_mut_ptr()) < 0 {
        eprintln!("[fstack-bridge] ff_load_config failed");
        return Err(1);
    }

    // Inject extra EAL tokens into the global dpdk_argv built by ff_load_config.
    // dpdk_argv has DPDK_ARGV_MAX slots (16 args + NULL); abort if we would overflow.
    for tok in extra_eal {
        if dpdk_argc() as usize + 1 >= ffi::DPDK_ARGV_MAX {
            eprintln!("[fstack-bridge] dpdk_argv overflow adding {:?}", tok);
            return Err(2);
        }
        push_eal_token(tok);
    }
    // NULL-terminate.
    ffi::dpdk_argv[dpdk_argc() as usize] = ptr::null_mut();

    eprintln!(
        "[fstack-bridge] EAL argv ({}): {:?}",
        dpdk_argc(),
        eal_argv_display()
    );

    let argv_ptr = ffi::dpdk_argv.as_mut_ptr();
    if ff_dpdk_init(dpdk_argc(), argv_ptr) < 0 {
        eprintln!("[fstack-bridge] ff_dpdk_init failed");
        return Err(3);
    }
    if ff_freebsd_init() < 0 {
        eprintln!("[fstack-bridge] ff_freebsd_init failed");
        return Err(4);
    }
    if ff_dpdk_if_up() < 0 {
        eprintln!("[fstack-bridge] ff_dpdk_if_up failed");
        return Err(5);
    }
    Ok(())
}

#[inline]
unsafe fn dpdk_argc() -> c_int {
    ffi::dpdk_argc
}

unsafe fn push_eal_token(tok: &str) {
    let c = CString::new(tok).expect("NUL in EAL token");
    let raw = c.into_raw(); // leaked intentionally: dpdk_argv owns it for the process lifetime
    let idx = dpdk_argc() as usize;
    ffi::dpdk_argv[idx] = raw;
    ffi::dpdk_argc += 1;
}

unsafe fn eal_argv_display() -> Vec<String> {
    let mut out = Vec::new();
    for i in 0..dpdk_argc() as usize {
        let p = ffi::dpdk_argv[i];
        if p.is_null() {
            break;
        }
        let s = std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned();
        out.push(s);
    }
    out
}

/// Re-export of the opaque FFI types for downstream consumers.
pub use ffi::ff_zc_mbuf;
