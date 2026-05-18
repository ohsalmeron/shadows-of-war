use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use wasm_bindgen::prelude::*;
use web_sys::{Event, MessageEvent, WebSocket};

pub struct SowClient {
    ws: WebSocket,
    pub rx: std::sync::mpsc::Receiver<Vec<u8>>,
    socket_closed: Arc<AtomicBool>,
}

impl SowClient {
    /// True after the browser fires `close` on the socket (idle kill, network drop, server reset).
    #[inline]
    pub fn is_socket_closed(&self) -> bool {
        self.socket_closed.load(Ordering::Relaxed)
    }

    pub async fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let ws = WebSocket::new(url).map_err(|e| {
            e.as_string()
                .unwrap_or_else(|| "WebSocket creation failed".to_string())
        })?;

        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        let socket_closed = Arc::new(AtomicBool::new(false));

        let tx_clone = tx.clone();
        let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
            if let Ok(ab) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
                let array = js_sys::Uint8Array::new(&ab);
                let mut data = vec![0; array.length() as usize];
                array.copy_to(&mut data);
                let _ = tx_clone.send(data);
            }
        });
        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        let closed_flag = Arc::clone(&socket_closed);
        let onclose_callback = Closure::<dyn FnMut()>::new(move || {
            closed_flag.store(true, Ordering::Release);
            log::warn!("WASM WebSocket closed");
        });
        ws.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
        onclose_callback.forget();

        let onerror_callback = Closure::<dyn FnMut(_)>::new(move |e: Event| {
            log::error!("WASM WebSocket error occurred on connection: {}", e.type_());
        });
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        let (open_tx, open_rx) = futures_channel::oneshot::channel::<()>();
        let open_tx = std::rc::Rc::new(std::cell::RefCell::new(Some(open_tx)));
        let onopen_callback = Closure::<dyn FnMut()>::new(move || {
            if let Some(tx) = open_tx.borrow_mut().take() {
                let _ = tx.send(());
            }
        });
        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        // Wait for the open event before returning
        let _ = open_rx.await;

        Ok(Self {
            ws,
            rx,
            socket_closed,
        })
    }

    pub fn send(&self, msg: Vec<u8>) {
        if self.ws.ready_state() == WebSocket::OPEN {
            let array = js_sys::Uint8Array::from(msg.as_slice());
            let _ = self.ws.send_with_array_buffer(&array.buffer());
        } else {
            log::warn!(
                "Attempted to send on non-open WebSocket (state: {})",
                self.ws.ready_state()
            );
        }
    }
}
