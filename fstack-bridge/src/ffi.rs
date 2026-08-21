//! Direct `extern "C"` FFI to F-Stack (`libfstack.a`).
//!
//! Every `ff_*` call MUST execute on the single `ff_run` thread
//! (F-Stack's FreeBSD stack is single-threaded: `pcurthread` is TLS,
//! `_sleep` returns EPERM, see lib/ff_kern_synch.c). The bridge enforces
//! this by routing all stack calls through the `ff_run` loop callback.

#![allow(non_camel_case_types, non_snake_case, dead_code)]

use libc::{c_char, c_int, c_void, sockaddr_in, socklen_t, ssize_t, size_t, timespec};

/// DPDK_CONFIG_NUM = 16, so dpdk_argv holds 16 args + 1 NULL sentinel.
pub const DPDK_ARGV_MAX: usize = 17;

/// `struct ff_zc_mbuf` (ff_api.h:373). Opaque BSD mbuf chain + cursor.
/// The `bsd_mbuf`/`bsd_mbuf_off` are FreeBSD `struct mbuf*` (treated as `void*` here).
#[repr(C)]
pub struct ff_zc_mbuf {
    pub bsd_mbuf: *mut c_void,
    pub bsd_mbuf_off: *mut c_void,
    pub off: c_int,
    pub len: c_int,
}

/// F-Stack's own `struct kevent` (ff_event.h:85) — NOT the Linux one.
#[repr(C)]
pub struct kevent {
    pub ident: usize,    // uintptr_t
    pub filter: i16,
    pub flags: u16,
    pub fflags: u32,
    pub data: i64,       // __int64_t
    pub udata: *mut c_void,
    pub ext: [u64; 4],
}

/// `int (*)(void *)` — the per-iteration loop callback passed to `ff_run`.
pub type loop_func_t = unsafe extern "C" fn(*mut c_void) -> c_int;

/// F-Stack packet dispatcher callback. It runs on the RX path before the
/// packet enters the FreeBSD stack. Returning another queue transfers the
/// mbuf through F-Stack's shared dispatch ring to that queue's process.
pub type dispatch_func_t = unsafe extern "C" fn(
    data: *mut c_void,
    len: *mut u16,
    queue_id: u16,
    nb_queues: u16,
) -> c_int;

pub const FF_DISPATCH_ERROR: c_int = -1;
pub const FF_DISPATCH_RESPONSE: c_int = -2;

// ---- kqueue filter/flag constants (ff_event.h) ----
pub const EVFILT_READ: i16 = -1;
pub const EVFILT_WRITE: i16 = -2;
pub const EV_ADD: u16 = 0x0001;
pub const EV_DELETE: u16 = 0x0002;
pub const EV_CLEAR: u16 = 0x0020;
pub const EV_EOF: u16 = 0x8000;
pub const EV_ERROR: u16 = 0x4000;

extern "C" {
    // ---- init (manual sequence; ff_init does the same but can't inject EAL args) ----
    pub fn ff_load_config(argc: c_int, argv: *mut *mut c_char) -> c_int;
    pub fn ff_dpdk_init(argc: c_int, argv: *mut *mut c_char) -> c_int;
    pub fn ff_freebsd_init() -> c_int;
    pub fn ff_dpdk_if_up() -> c_int;

    // DPDK EAL argv built by ff_load_config from the [dpdk] section (extern globals).
    pub static mut dpdk_argc: c_int;
    pub static mut dpdk_argv: [*mut c_char; DPDK_ARGV_MAX];

    // ---- run loop ----
    pub fn ff_run(callback: loop_func_t, arg: *mut c_void);
    pub fn ff_stop_run();
    pub fn ff_regist_packet_dispatcher(func: dispatch_func_t);

    // ---- kqueue ----
    pub fn ff_kqueue() -> c_int;
    pub fn ff_kevent(
        kq: c_int,
        changelist: *const kevent,
        nchanges: c_int,
        eventlist: *mut kevent,
        nevents: c_int,
        timeout: *const timespec,
    ) -> c_int;

    // ---- sockets ----
    pub fn ff_socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int;
    pub fn ff_ioctl(fd: c_int, request: libc::c_ulong, ...) -> c_int;
    pub fn ff_setsockopt(
        s: c_int, level: c_int, optname: c_int, optval: *const c_void, optlen: socklen_t,
    ) -> c_int;
    pub fn ff_bind(s: c_int, addr: *const sockaddr_in, addrlen: socklen_t) -> c_int;
    pub fn ff_listen(s: c_int, backlog: c_int) -> c_int;
    pub fn ff_accept(s: c_int, addr: *mut sockaddr_in, addrlen: *mut socklen_t) -> c_int;
    pub fn ff_close(fd: c_int) -> c_int;
    pub fn ff_shutdown(s: c_int, how: c_int) -> c_int;

    // ---- introspection ----
    pub fn ff_rss_self_queue_info(
        proc_id: *mut u16, queueid: *mut u16, nb_queues: *mut u16, reta_size: *mut u16,
    ) -> c_int;

    // ---- plain read/write (non-zero-copy fallback) ----
    pub fn ff_read(fd: c_int, buf: *mut c_void, nbytes: size_t) -> ssize_t;
    pub fn ff_write(fd: c_int, buf: *const c_void, nbytes: size_t) -> ssize_t;

    // ---- ZERO-COPY send (ff_api.h:393-475) ----
    pub fn ff_zc_mbuf_get(m: *mut ff_zc_mbuf, len: c_int) -> c_int;
    pub fn ff_zc_mbuf_write(m: *mut ff_zc_mbuf, data: *const c_char, len: c_int) -> c_int;
    pub fn ff_zc_send(fd: c_int, mb: *const c_void, nbytes: size_t) -> ssize_t;

    // ---- ZERO-COPY receive (ff_api.h:433-460; needs libfstack built with FF_ZC_RECV=1) ----
    pub fn ff_zc_recv(fd: c_int, zm: *mut ff_zc_mbuf, nbytes: size_t) -> ssize_t;
    pub fn ff_zc_mbuf_segment(zm: *mut ff_zc_mbuf, seg_data: *mut *mut c_void, seg_len: *mut c_int) -> c_int;
    pub fn ff_zc_recv_free(zm: *mut ff_zc_mbuf);

    // ---- traffic counters (ff_api.h:199; struct ff_traffic_args, ff_msg.h:103) ----
    pub fn ff_get_traffic(buffer: *mut c_void);
}

/// Mirror of `struct ff_traffic_args` (ff_msg.h:103). `tx_dropped` counts
/// segments freed at `rte_eth_tx_burst` when the NIC TX ring cannot take them.
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct FfTraffic {
    pub rx_packets: u64,
    pub rx_bytes: u64,
    pub tx_packets: u64,
    pub tx_bytes: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
}

/// Helper mirroring the `EV_SET` macro (ff_event.h:55). Safe to call to populate one kevent.
#[inline]
pub fn ev_set(
    kev: &mut kevent,
    ident: usize,
    filter: i16,
    flags: u16,
    fflags: u32,
    data: i64,
    udata: *mut c_void,
) {
    kev.ident = ident;
    kev.filter = filter;
    kev.flags = flags;
    kev.fflags = fflags;
    kev.data = data;
    kev.udata = udata;
    kev.ext = [0; 4];
}
