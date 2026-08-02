//! M3-FaseA — bincode protocol over the bridge WebSocket (real sow-core types).
//!
//! Replaces the raw echo of `echo_ws` with the real game protocol: each binary
//! WS frame is `bincode::deserialize::<ClientMessage>`, and `Ready` is answered
//! with `ServerMessage::Start` (bincode-serialized). The bridge `Conn` is
//! untouched — the protocol lives entirely in this example on top of M2's
//! validated `accept_async` path.
//!
//! Run (physical VF):
//!   ./relay_bincode --conf echo-vf.ini --proc-type=primary --proc-id=0
//!
//! Test:
//!   ./relay_client ws://<data-pip>:80
//!   (or: printf '<bincode Ready>' | websocat -b ws://<data-pip>:80)

use fstack_bridge::bridge::{self, Ev};
use fstack_bridge::ffi::{ev_set, kevent, EV_ADD, EVFILT_READ};
use futures_util::{SinkExt, StreamExt};
use libc::{
    c_int, c_void, sockaddr_in, socklen_t, AF_INET, INADDR_ANY, SOCK_STREAM, SOL_SOCKET,
    SO_REUSEADDR, FIONBIO,
};
use sow_core::game_config::GameConfig;
use sow_core::protocol::{ClientMessage, ServerMessage, ServerStartMessage};
use std::collections::HashMap;
use std::ffi::CString;
use std::mem;
use std::ptr;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

const LISTEN_PORT: u16 = 80;

fn main() {
    let prog_args: Vec<CString> = std::env::args()
        .filter(|a| !a.starts_with("--fstack-")) // keep --conf/--proc-type/--proc-id for ff_load_config
        .map(|a| CString::new(a).unwrap())
        .collect();

    unsafe {
        if let Err(code) = fstack_bridge::init(&prog_args, &[]) {
            eprintln!("[relay-bincode] init failed (code={})", code);
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
            eprintln!("[relay-bincode] ff_kqueue failed");
            std::process::exit(1);
        }

        let lfd = fstack_bridge::ff_socket(AF_INET, SOCK_STREAM, 0);
        if lfd < 0 {
            eprintln!("[relay-bincode] ff_socket failed");
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
            eprintln!("[relay-bincode] ff_bind :{} failed", LISTEN_PORT);
            std::process::exit(1);
        }
        if fstack_bridge::ff_listen(lfd, 512) < 0 {
            eprintln!("[relay-bincode] ff_listen failed");
            std::process::exit(1);
        }

        let mut kev: kevent = mem::zeroed();
        ev_set(&mut kev, lfd as usize, EVFILT_READ, EV_ADD, 0, 512, ptr::null_mut());
        fstack_bridge::ff_kevent(bridge::KQ, &kev, 1, ptr::null_mut(), 0, ptr::null());

        // Tokio workers run the WS logic; they never touch ff_* (rings only).
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.spawn(bridge_worker());

        eprintln!(
            "[BOOT] listening on :{}, entering ff_run (bridge driver)",
            LISTEN_PORT
        );
        fstack_bridge::ff_run(bridge::driver_cb, ptr::null_mut());
    }
}

/// Dispatcher: routes RX events to per-connection channels and spawns one
/// WebSocket task per accepted connection.
async fn bridge_worker() {
    let rx = bridge::rx_ring();
    let notify = bridge::notify();
    let mut conns: HashMap<c_int, mpsc::UnboundedSender<bridge::ZcRxGuard>> = HashMap::new();

    loop {
        while let Some(ev) = rx.pop() {
            match ev {
                Ev::Accept { fd, generation } => {
                    let (tx, rx_conn) = mpsc::unbounded_channel();
                    conns.insert(fd, tx);
                    tokio::spawn(ws_task(fd, generation, rx_conn));
                }
                Ev::Data { fd, guard } => match conns.get(&fd) {
                    Some(tx) => {
                        let _ = tx.send(guard); // Err => conn gone; guard dropped = recycled
                    }
                    None => drop(guard),
                },
                Ev::Closed { fd } => {
                    conns.remove(&fd); // drops sender; ws_task sees EOF
                }
            }
        }
        notify.notified().await;
    }
}

/// One WebSocket connection: RFC 6455 handshake over the bridge Conn, then the
/// real sow-core protocol in bincode on top of it.
async fn ws_task(fd: c_int, generation: u64, rx: mpsc::UnboundedReceiver<bridge::ZcRxGuard>) {
    let conn = bridge::Conn::new(fd, generation, rx);
    let ws = match tokio_tungstenite::accept_async(conn).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("[ws] handshake fail fd={} err={}", fd, e);
            return;
        }
    };
    eprintln!("[ws] handshake OK fd={}", fd);

    let (mut sink, mut stream) = ws.split();
    loop {
        match stream.next().await {
            Some(Ok(Message::Binary(b))) => {
                // b is owned (poll_read copies out of the DPDK mbuf and drops the
                // ZcRxGuard inside the call) — safe to deserialize from it.
                match bincode::deserialize::<ClientMessage>(&b) {
                    Ok(ClientMessage::Ready { lobby_id, player_id }) => {
                        eprintln!("[bincode] Ready lobby={} player={}", lobby_id, player_id);
                        let start = ServerMessage::Start(Box::new(ServerStartMessage {
                            config: GameConfig::default(),
                            my_player_id: Some(player_id),
                            lobby_id: Some(lobby_id),
                            seed: 42,
                            players: Vec::new(),
                            missed_turns: Vec::new(),
                            map_data: None,
                            relay_port: None,
                            relay_host: None,
                        }));
                        match bincode::serialize(&start) {
                            Ok(bytes) => {
                                let _ = sink.send(Message::Binary(bytes)).await;
                            }
                            Err(e) => eprintln!("[bincode] serialize err {}", e),
                        }
                    }
                    Ok(other) => eprintln!("[bincode] other={:?}", other),
                    Err(e) => eprintln!("[bincode] deserialize err {} len={}", e, b.len()),
                }
            }
            Some(Ok(Message::Text(t))) => {
                eprintln!("[ws] text len={}", t.len());
            }
            Some(Ok(Message::Ping(p))) => {
                let _ = sink.send(Message::Pong(p)).await;
            }
            Some(Ok(Message::Close(c))) => {
                let _ = sink.send(Message::Close(c)).await;
                break;
            }
            Some(Ok(_)) => {} // Pong / Frame — nothing to do
            Some(Err(e)) => {
                eprintln!("[ws] recv err fd={} err={}", fd, e);
                break;
            }
            None => break,
        }
    }
    eprintln!("[ws] done fd={}", fd);
}
