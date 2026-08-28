//! M1b — echo over the bridge: tokio worker runs the logic, ff_run thread does I/O.
//!
//! Path: VF → `ff_zc_recv` (ff_run) → RX ring → tokio worker (echo logic, pool
//! BytesMut, 1 copy) → TX ring → `ff_zc_send` (ff_run) → VF.
//!
//! Run (physical VF):
//!   ./echo_m1b --conf echo-vf.ini --proc-type=primary --proc-id=0

use fstack_bridge::bridge::{self, Cmd, Ev};
use fstack_bridge::ffi::{ev_set, kevent, EV_ADD, EVFILT_READ};
use libc::{
    c_int, c_void, sockaddr_in, socklen_t, AF_INET, INADDR_ANY, SOCK_STREAM, SOL_SOCKET,
    SO_REUSEADDR, FIONBIO,
};
use std::collections::HashMap;
use std::ffi::CString;
use std::mem;
use std::ptr;

const LISTEN_PORT: u16 = 80;

fn main() {
    let prog_args: Vec<CString> = std::env::args()
        .filter(|a| !a.starts_with("--fstack-")) // keep --conf/--proc-type/--proc-id for ff_load_config
        .map(|a| CString::new(a).unwrap())
        .collect();

    unsafe {
        if let Err(code) = fstack_bridge::init(&prog_args, &[]) {
            eprintln!("[echo-m1b] init failed (code={})", code);
            std::process::exit(1);
        }

        let (mut pid, mut qid, mut nbq, mut reta) = (0u16, 0u16, 0u16, 0u16);
        if fstack_bridge::ff_rss_self_queue_info(&mut pid, &mut qid, &mut nbq, &mut reta) == 0 {
            eprintln!(
                "[BOOT] proc_id={} queue_id={} nb_queues={} reta_size={}",
                pid, qid, nbq, reta
            );
        }

        bridge::setup();
        bridge::KQ = fstack_bridge::ff_kqueue();
        if bridge::KQ < 0 {
            eprintln!("[echo-m1b] ff_kqueue failed");
            std::process::exit(1);
        }

        let lfd = fstack_bridge::ff_socket(AF_INET, SOCK_STREAM, 0);
        if lfd < 0 {
            eprintln!("[echo-m1b] ff_socket failed");
            std::process::exit(1);
        }
        bridge::LISTEN_FD = lfd;

        let on: c_int = 1;
        fstack_bridge::ff_setsockopt(
            lfd,
            SOL_SOCKET,
            SO_REUSEADDR,
            &on as *const _ as *const c_void,
            mem::size_of::<c_int>() as socklen_t,
        );
        fstack_bridge::ff_ioctl(lfd, FIONBIO as libc::c_ulong, &on);

        let mut addr: sockaddr_in = mem::zeroed();
        addr.sin_family = AF_INET as u16;
        addr.sin_port = LISTEN_PORT.to_be();
        addr.sin_addr.s_addr = INADDR_ANY;
        if fstack_bridge::ff_bind(lfd, &addr, mem::size_of::<sockaddr_in>() as socklen_t) < 0 {
            eprintln!("[echo-m1b] ff_bind :{} failed", LISTEN_PORT);
            std::process::exit(1);
        }
        if fstack_bridge::ff_listen(lfd, 512) < 0 {
            eprintln!("[echo-m1b] ff_listen failed");
            std::process::exit(1);
        }

        let mut kev: kevent = mem::zeroed();
        ev_set(&mut kev, lfd as usize, EVFILT_READ, EV_ADD, 0, 512, ptr::null_mut());
        fstack_bridge::ff_kevent(bridge::KQ, &kev, 1, ptr::null_mut(), 0, ptr::null());

        // Tokio workers run the logic; they never touch ff_* (rings only).
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.spawn(bridge_worker());

        eprintln!("[BOOT] listening on :{}, entering ff_run (bridge driver)", LISTEN_PORT);
        fstack_bridge::ff_run(bridge::driver_cb, ptr::null_mut());
    }
}

/// Worker: consumes RX events, runs the echo logic, pushes TX commands.
async fn bridge_worker() {
    let rx = bridge::rx_ring();
    let tx = bridge::tx_ring();
    let notify = bridge::notify();
    let mut gens: HashMap<c_int, u64> = HashMap::new();

    loop {
        while let Some(ev) = rx.pop() {
            match ev {
                Ev::Accept { fd, generation, .. } => {
                    gens.insert(fd, generation);
                }
                Ev::Data { fd, mut guard } => {
                    let mut buf = bridge::take_buf();
                    unsafe {
                        for seg in guard.segments() {
                            buf.extend_from_slice(seg);
                        }
                    }
                    let generation = *gens.get(&fd).unwrap_or(&0);
                    if tx.push(Cmd::Send { fd, generation, buf, tx_pending: None }).is_err() {
                        eprintln!("[tokio] TX ring full — dropping send for fd={}", fd);
                    }
                    drop(guard); // recycles the DPDK mbuf on the ff_run thread
                }
                Ev::Closed { fd } => {
                    gens.remove(&fd);
                }
            }
        }
        notify.notified().await;
    }
}
