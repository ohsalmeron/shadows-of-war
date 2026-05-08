use wasm_bindgen::prelude::*;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

pub struct SowClient {
    ws: WebSocket,
}

impl SowClient {
    pub async fn connect(url: &str) -> Result<Self, JsValue> {
        let ws = WebSocket::new(url)?;
        
        let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
            if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                let text: String = txt.into();
                log::info!("WASM received: {}", text);
            }
        });
        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();
        
        let onerror_callback = Closure::<dyn FnMut(_)>::new(move |e: ErrorEvent| {
            log::error!("WASM WebSocket error: {:?}", e.message());
        });
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        Ok(Self { ws })
    }

    pub fn send(&self, msg: String) {
        let _ = self.ws.send_with_str(&msg);
    }
}
