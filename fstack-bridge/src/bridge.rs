//! M1b — the bridge: ff_run I/O driver thread <-> tokio workers over SPSC rings.
//!
//! Invariant (see lib.rs): every `ff_*` call MUST run on the ff_run loop thread
//! (F-Stack's FreeBSD stack is single-threaded: `pcurthread` is TLS). So:
//!
//! - The driver (the `ff_run` loop callback) is the ONLY code that calls `ff_*`.
//!   It owns the kqueue, accepts, `ff_zc_recv`s, `ff_zc_send`s and `ff_close`s.
//! - Tokio workers never call `ff_*`. They consume `Ev` from the RX ring
//!   (ff_run -> workers), run the application logic, and push `Cmd` to the TX
//!   ring (workers -> ff_run).
//! - `ZcRxGuard` crosses threads with the DPDK mbuf aliased by its segments.
//!   Dropping it pushes `Cmd::Recycle`; the driver calls `ff_zc_recv_free` on the
//!   ff_run thread. The guard is `Send` only in this sense: the mbuf pointer is
//!   never dereferenced outside ff_run, and freeing happens exactly once (the
//!   drop path routes through the ring, not through `ff_zc_recv_free` directly).
//! - Write backpressure: if `ff_zc_send` returns EAGAIN the payload is parked in
//!   `PENDING_SEND` and an `EVFILT_WRITE` event is armed; `flush_write` retries.
//! - The kevent poll uses a 10ms timeout so the TX ring is serviced even when
//!   no network event is pending (a send parked on the ring must not wait for
//!   the next incoming packet).

use crate::ffi::{
    ev_set, ff_zc_mbuf, kevent, EV_ADD, EV_DELETE, EV_ERROR, EV_EOF, EVFILT_READ, EVFILT_WRITE,
};
use bytes::{Buf, BytesMut};
use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;
use libc::{c_char, c_int, c_void, sockaddr_in, socklen_t, size_t, timespec, FIONBIO};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::mem;
use std::pin::Pin;
use std::ptr;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Notify;

pub const MAX_EVENTS: usize = 512;
pub const RX_CAP: usize = 65536;
pub const TX_CAP: usize = 65536;
pub const POOL_CAP: usize = 16384;
pub const POOL_BUF_SIZE: usize = 65536;
/// kevent timeout (ns): bounds how long a TX ring command waits without traffic.
const POLL_TIMEOUT_NS: i64 = 10_000_000;

/// RX ring item: ff_run -> tokio.
pub enum Ev {
    Accept { fd: c_int, generation: u64 },
    Data { fd: c_int, guard: ZcRxGuard },
    Closed { fd: c_int },
}

/// TX ring item: tokio -> ff_run.
pub enum Cmd {
    Send {
        fd: c_int,
        generation: u64,
        buf: BytesMut,
    },
    Close {
        fd: c_int,
        generation: u64,
    },
    /// Return a received mbuf for freeing on the ff_run thread (ff_zc_recv_free).
    Recycle { zm: ff_zc_mbuf },
}

// Sound: the mbuf pointer is only ever dereferenced on the ff_run thread; the
// ring only moves ownership between threads.
unsafe impl Send for Cmd {}

/// RAII owner of a zero-copy receive mbuf. `segments()` borrows the DPDK buffer
/// directly (0 copies); the worker must copy/process before dropping the guard.
/// Dropping pushes `Cmd::Recycle` — actual freeing happens on ff_run.
pub struct ZcRxGuard {
    zm: ff_zc_mbuf,
}

// Sound: the worker never dereferences the mbuf (segments() only aliases the
// payload for the guard's lifetime), and freeing is routed through the TX ring
// to the ff_run thread. The pointer cannot be used concurrently: the guard is
// owned by one thread at a time (moved across the SPSC ring).
unsafe impl Send for ZcRxGuard {}

impl ZcRxGuard {
    pub fn len(&self) -> usize {
        self.zm.len as usize
    }

    /// Segments aliasing the DPDK receive buffer. Must be consumed before the
    /// guard is dropped (the mbuf is recycled on drop).
    pub unsafe fn segments(&mut self) -> Vec<&[u8]> {
        let mut out = Vec::new();
        let mut p: *mut c_void = ptr::null_mut();
        let mut l: c_int = 0;
        while crate::ffi::ff_zc_mbuf_segment(&mut self.zm, &mut p, &mut l) > 0 {
            if l > 0 && !p.is_null() {
                out.push(std::slice::from_raw_parts(p as *const u8, l as usize));
            }
        }
        out
    }
}

impl Drop for ZcRxGuard {
    fn drop(&mut self) {
        let zm = mem::replace(&mut self.zm, unsafe { mem::zeroed() });
        if !zm.bsd_mbuf.is_null() {
            if let Some(tx) = TX.get() {
                if tx.push(Cmd::Recycle { zm }).is_err() {
                    eprintln!("[bridge] TX ring full: DPDK mbuf leaked");
                }
            }
        }
    }
}

// ---- shared state ----------------------------------------------------------

/// kqueue + listening fd, owned by the example and read by the driver.
pub static mut KQ: c_int = -1;
pub static mut LISTEN_FD: c_int = -1;

static RX: OnceLock<Arc<ArrayQueue<Ev>>> = OnceLock::new();
static TX: OnceLock<Arc<ArrayQueue<Cmd>>> = OnceLock::new();
static POOL: OnceLock<Arc<ArrayQueue<BytesMut>>> = OnceLock::new();
static NOTIFY: OnceLock<Arc<Notify>> = OnceLock::new();
/// Woken by the driver after every TX drain; async writers park here when the
/// TX ring is full (backpressure for the AsyncWrite side).
static TX_SPACE: AtomicWaker = AtomicWaker::new();

/// RX deliveries that could not be pushed (ring full) — retried next iteration.
static mut PENDING_RX: VecDeque<Ev> = VecDeque::new();
/// Payloads parked on EAGAIN, retried on EVFILT_WRITE. Value carries the
/// connection generation so a stale send can never hit a reused fd.
static mut PENDING_SEND: Option<HashMap<c_int, (u64, BytesMut)>> = None;
/// fd -> generation of the connection currently owning that fd. The generation
/// is the accept counter value at accept time: strictly unique per connection
/// for the lifetime of the driver. A `Cmd::Close`/`Cmd::Send` is honored only
/// if its generation still matches — otherwise it belongs to a connection whose
/// fd has already been closed and reused, and must be ignored (the alternative
/// kills the NEW connection that now owns the fd).
static mut FD_GEN: Option<HashMap<c_int, u64>> = None;

/// Accessor for the single-threaded driver state (only ff_run touches it).
unsafe fn pending_send() -> &'static mut HashMap<c_int, (u64, BytesMut)> {
    PENDING_SEND.get_or_insert_with(HashMap::new)
}

/// Accessor for the fd -> generation map (only ff_run touches it).
unsafe fn fd_gen() -> &'static mut HashMap<c_int, u64> {
    FD_GEN.get_or_insert_with(HashMap::new)
}

static mut ACCEPTS: u64 = 0;
static mut ECHOES: u64 = 0;
static mut RECV_BYTES: u64 = 0;
static mut LAST_STATS_AT: u64 = 0;

/// Create rings + pool + notify. Call once before spawning any worker.
pub fn setup() {
    RX.set(Arc::new(ArrayQueue::new(RX_CAP))).ok();
    TX.set(Arc::new(ArrayQueue::new(TX_CAP))).ok();
    POOL.set(Arc::new(ArrayQueue::new(POOL_CAP))).ok();
    NOTIFY.set(Arc::new(Notify::new())).ok();
    unsafe {
        PENDING_RX = VecDeque::new();
        PENDING_SEND = None;
        FD_GEN = None;
    }
}

pub fn rx_ring() -> Arc<ArrayQueue<Ev>> {
    RX.get().expect("bridge::setup()").clone()
}
pub fn tx_ring() -> Arc<ArrayQueue<Cmd>> {
    TX.get().expect("bridge::setup()").clone()
}
pub fn notify() -> Arc<Notify> {
    NOTIFY.get().expect("bridge::setup()").clone()
}

/// Take a clean writable buffer from the pool (or allocate one if empty).
pub fn take_buf() -> BytesMut {
    let mut b = POOL
        .get()
        .expect("bridge::setup()")
        .pop()
        .unwrap_or_else(|| BytesMut::with_capacity(POOL_BUF_SIZE));
    b.clear();
    b
}

/// Return a buffer to the pool (dropped if oversized/overfull).
pub fn put_buf(buf: BytesMut) {
    if buf.capacity() <= POOL_BUF_SIZE {
        let _ = POOL.get().expect("bridge::setup()").push(buf);
    }
}

// ---- Conn: AsyncRead/AsyncWrite over the rings (M2, for tokio-tungstenite) --

/// One accepted connection as an async byte stream.
///
/// - `poll_read` pops `ZcRxGuard`s from the per-connection mpsc. The guard's
///   DPDK payload is copied out inside this call (directly into the caller's
///   buffer when it fits — one copy — or via an internal pool buffer for large
///   guards) and the guard is dropped, which recycles the mbuf on ff_run.
/// - `poll_write` copies into a pool `BytesMut` and pushes `Cmd::Send`; if the
///   TX ring is full it parks the waker and the driver wakes it after draining.
/// - `poll_shutdown`/`Drop` push `Cmd::Close`.
///
/// This stream layer is for protocol codecs (WebSocket); the game path keeps
/// the owned-buffer `BytesMut` API (single copy, no stream re-buffering).
pub struct Conn {
    fd: c_int,
    generation: u64,
    rx: UnboundedReceiver<ZcRxGuard>,
    /// Leftover of a guard that was too large for the caller's read buffer.
    rx_buf: BytesMut,
}

impl Conn {
    pub fn new(fd: c_int, generation: u64, rx: UnboundedReceiver<ZcRxGuard>) -> Self {
        Conn {
            fd,
            generation,
            rx,
            rx_buf: BytesMut::new(),
        }
    }

    pub fn fd(&self) -> c_int {
        self.fd
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        if let Some(tx) = TX.get() {
            let _ = tx.push(Cmd::Close {
                fd: self.fd,
                generation: self.generation,
            });
        }
    }
}

impl AsyncRead for Conn {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Serve leftovers of an oversized guard first.
        if !self.rx_buf.is_empty() {
            let n = std::cmp::min(self.rx_buf.len(), buf.remaining());
            buf.put_slice(&self.rx_buf[..n]);
            self.rx_buf.advance(n);
            return Poll::Ready(Ok(()));
        }

        match self.rx.poll_recv(cx) {
            Poll::Ready(Some(mut guard)) => {
                let segs = unsafe { guard.segments() };
                let total: usize = segs.iter().map(|s| s.len()).sum();
                if total <= buf.remaining() {
                    for s in &segs {
                        buf.put_slice(s);
                    }
                } else {
                    let mut b = take_buf();
                    for s in &segs {
                        b.extend_from_slice(s);
                    }
                    let n = std::cmp::min(b.len(), buf.remaining());
                    buf.put_slice(&b[..n]);
                    b.advance(n);
                    self.rx_buf = b;
                }
                drop(guard); // recycles the mbuf on ff_run
                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) => Poll::Ready(Ok(())), // Ev::Closed received: EOF
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for Conn {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }
        let mut b = take_buf();
        b.extend_from_slice(buf);
        match TX
            .get()
            .expect("bridge::setup()")
            .push(Cmd::Send {
                fd: self.fd,
                generation: self.generation,
                buf: b,
            }) {
            Ok(()) => Poll::Ready(Ok(buf.len())),
            Err(Cmd::Send {
                fd: _,
                generation: _,
                buf: b,
            }) => {
                put_buf(b);
                TX_SPACE.register(cx.waker());
                Poll::Pending
            }
            Err(_) => Poll::Pending, // unreachable: we only ever push Cmd::Send here
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Fire-and-forget: once pushed, the payload is owned by the ff_run driver.
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Some(tx) = TX.get() {
            if tx
                .push(Cmd::Close {
                    fd: self.fd,
                    generation: self.generation,
                })
                .is_err()
            {
                TX_SPACE.register(cx.waker());
                return Poll::Pending;
            }
        }
        Poll::Ready(Ok(()))
    }
}

// ---- driver (ff_run loop callback — the only ff_* caller) ------------------

/// Loop callback passed to `ff_run`. Do NOT call from tokio.
pub unsafe extern "C" fn driver_cb(_arg: *mut c_void) -> c_int {
    // 1. Service worker commands first (send/close/recycle).
    drain_tx();
    TX_SPACE.wake(); // parked AsyncWrite writers may retry now

    // 2. Network events; short timeout so TX commands are serviced even idle.
    let mut events: [kevent; MAX_EVENTS] = mem::zeroed();
    let ts = timespec {
        tv_sec: 0,
        tv_nsec: POLL_TIMEOUT_NS,
    };
    let nevents = crate::ffi::ff_kevent(
        KQ,
        ptr::null(),
        0,
        events.as_mut_ptr(),
        MAX_EVENTS as c_int,
        &ts,
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
            close_fd(fd);
            continue;
        }
        if fd == LISTEN_FD {
            accept_pending();
        } else if ev.filter == EVFILT_READ {
            zc_read(fd);
        } else if ev.filter == EVFILT_WRITE {
            flush_write(fd);
        }
    }

    // 3. Retry RX deliveries blocked on a full ring.
    let mut delivered = false;
    while let Some(ev) = PENDING_RX.pop_front() {
        match RX.get().expect("bridge::setup()").push(ev) {
            Ok(()) => delivered = true,
            Err(ev) => {
                PENDING_RX.push_front(ev);
                break;
            }
        }
    }
    if delivered {
        NOTIFY.get().expect("bridge::setup()").notify_one();
    }

    maybe_stats(nevents == 0);
    0
}

unsafe fn accept_pending() {
    loop {
        let mut peer: sockaddr_in = mem::zeroed();
        let mut peerlen: socklen_t = mem::size_of::<sockaddr_in>() as socklen_t;
        let nfd = crate::ffi::ff_accept(LISTEN_FD, &mut peer, &mut peerlen);
        if nfd < 0 {
            break;
        }
        let on: c_int = 1;
        crate::ffi::ff_ioctl(nfd, FIONBIO as libc::c_ulong, &on);
        ACCEPTS += 1;
        // The accept counter is strictly monotonic: it is this connection's
        // unique generation while it owns the fd.
        fd_gen().insert(nfd, ACCEPTS);
        let mut kev: kevent = mem::zeroed();
        ev_set(&mut kev, nfd as usize, EVFILT_READ, EV_ADD, 0, 0, ptr::null_mut());
        crate::ffi::ff_kevent(KQ, &kev, 1, ptr::null_mut(), 0, ptr::null());
        push_rx(Ev::Accept { fd: nfd, generation: ACCEPTS });
    }
}

unsafe fn zc_read(fd: c_int) {
    let mut zm: ff_zc_mbuf = mem::zeroed();
    let n = crate::ffi::ff_zc_recv(fd, &mut zm, 65536);
    if n > 0 {
        RECV_BYTES += n as u64;
        push_rx(Ev::Data {
            fd,
            guard: ZcRxGuard { zm },
        });
    } else if n == 0 {
        // peer half-closed
        close_fd(fd);
    }
    // n < 0 => EAGAIN: level-triggered kqueue will re-report.
}

unsafe fn close_fd(fd: c_int) {
    crate::ffi::ff_close(fd);
    // The fd is gone: any stale Cmd::Close/Cmd::Send still in the TX ring for
    // this generation will fail the FD_GEN check (or worse, hit a reused fd
    // with a DIFFERENT generation — also rejected).
    fd_gen().remove(&fd);
    if pending_send().remove(&fd).is_some() {
        remove_write_event(fd);
    }
    push_rx(Ev::Closed { fd });
}

unsafe fn push_rx(ev: Ev) {
    if let Some(rx) = RX.get() {
        match rx.push(ev) {
            Ok(()) => {
                if let Some(n) = NOTIFY.get() {
                    n.notify_one();
                }
            }
            Err(ev) => PENDING_RX.push_back(ev),
        }
    }
}

unsafe fn drain_tx() {
    let Some(tx) = TX.get() else {
        return;
    };
    while let Some(cmd) = tx.pop() {
        match cmd {
            Cmd::Send {
                fd,
                generation,
                buf,
            } => {
                // Reject sends from a connection that no longer owns the fd.
                if fd_gen().get(&fd) == Some(&generation) {
                    try_send(fd, generation, buf);
                } else {
                    put_buf(buf);
                }
            }
            Cmd::Close { fd, generation } => {
                // ONLY close if the generation still matches: a stale Close
                // (the connection died, its fd was reused by a new connection)
                // must not kill the new one. This was the root cause of the
                // relay Fase-2 ws_tasks dying right after `Ready`.
                if fd_gen().get(&fd) == Some(&generation) {
                    close_fd(fd);
                }
            }
            Cmd::Recycle { mut zm } => crate::ffi::ff_zc_recv_free(&mut zm),
        }
    }
}

unsafe fn try_send(fd: c_int, generation: u64, buf: BytesMut) {
    // Re-check: the fd may have been closed and reused while the payload was
    // parked (flush_write path). Never touch a fd this connection doesn't own.
    if fd_gen().get(&fd) != Some(&generation) {
        put_buf(buf);
        return;
    }
    let mut sm: ff_zc_mbuf = mem::zeroed();
    if crate::ffi::ff_zc_mbuf_get(&mut sm, buf.len() as c_int) == 0 {
        crate::ffi::ff_zc_mbuf_write(&mut sm, buf.as_ptr() as *const c_char, buf.len() as c_int);
        let n = crate::ffi::ff_zc_send(fd, sm.bsd_mbuf, buf.len() as size_t);
        if n >= 0 {
            ECHOES += 1;
            put_buf(buf);
        } else {
            // EAGAIN: park the payload, retry on EVFILT_WRITE.
            pending_send().insert(fd, (generation, buf));
            ensure_write_event(fd);
        }
    } else {
        put_buf(buf);
    }
}

unsafe fn flush_write(fd: c_int) {
    match pending_send().remove(&fd) {
        Some((generation, buf)) => try_send(fd, generation, buf),
        None => {
            // Write event with nothing parked: deregister (auto-cleanup path).
            remove_write_event(fd);
        }
    }
}

unsafe fn ensure_write_event(fd: c_int) {
    let mut kev: kevent = mem::zeroed();
    ev_set(&mut kev, fd as usize, EVFILT_WRITE, EV_ADD, 0, 0, ptr::null_mut());
    crate::ffi::ff_kevent(KQ, &kev, 1, ptr::null_mut(), 0, ptr::null());
}

unsafe fn remove_write_event(fd: c_int) {
    let mut kev: kevent = mem::zeroed();
    ev_set(&mut kev, fd as usize, EVFILT_WRITE, EV_DELETE, 0, 0, ptr::null_mut());
    crate::ffi::ff_kevent(KQ, &kev, 1, ptr::null_mut(), 0, ptr::null());
}

unsafe fn maybe_stats(idle: bool) {
    if ACCEPTS.saturating_sub(LAST_STATS_AT) >= 25 || (idle && ACCEPTS != LAST_STATS_AT) {
        LAST_STATS_AT = ACCEPTS;
        eprintln!(
            "[stats] accepts={} echoes={} recv_bytes={}",
            ACCEPTS, ECHOES, RECV_BYTES
        );
    }
}
