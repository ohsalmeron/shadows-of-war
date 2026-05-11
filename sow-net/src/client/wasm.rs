use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

pub struct SowClient {
    ws: WebSocket,
    pub rx: std::sync::mpsc::Receiver<String>,
}

impl SowClient {
    pub async fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let ws = WebSocket::new(url).map_err(|e| e.as_string().unwrap_or_else(|| "WebSocket creation failed".to_string()))?;
        
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        
        let tx_clone = tx.clone();
        let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
            if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                let text: String = txt.into();
                let _ = tx_clone.send(text);
            }
        });
        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();
        
        let onerror_callback = Closure::<dyn FnMut(_)>::new(move |e: ErrorEvent| {
            log::error!("WASM WebSocket error: {:?}", e.message());
        });
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        Ok(Self { ws, rx })
    }

    pub fn send(&self, msg: String) {
        let _ = self.ws.send_with_str(&msg);
    }
}
