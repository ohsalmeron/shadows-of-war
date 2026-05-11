use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

pub struct SowClient {
    ws: WebSocket,
    pub rx: std::sync::mpsc::Receiver<String>,
    socket_closed: Arc<AtomicBool>,
}

impl SowClient {
    /// True after the browser fires `close` on the socket (idle kill, network drop, server reset).
    #[inline]
    pub fn is_socket_closed(&self) -> bool {
        self.socket_closed.load(Ordering::Relaxed)
    }

    pub async fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let ws = WebSocket::new(url).map_err(|e| e.as_string().unwrap_or_else(|| "WebSocket creation failed".to_string()))?;
        
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let socket_closed = Arc::new(AtomicBool::new(false));

        let tx_clone = tx.clone();
        let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
            if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                let text: String = txt.into();
                let _ = tx_clone.send(text);
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
        
        let onerror_callback = Closure::<dyn FnMut(_)>::new(move |e: ErrorEvent| {
            log::error!("WASM WebSocket error: {:?}", e.message());
        });
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        Ok(Self { ws, rx, socket_closed })
    }

    pub fn send(&self, msg: String) {
        let _ = self.ws.send_with_str(&msg);
    }
}
