use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub struct SowClient {
    tx: mpsc::Sender<String>,
    pub rx: std::sync::mpsc::Receiver<String>,
    _task: JoinHandle<()>,
}

impl SowClient {
    /// Native disconnect is detected when the receive channel disconnects (reader task exited).
    #[inline]
    pub fn is_socket_closed(&self) -> bool {
        false
    }

    pub async fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let (ws_stream, _) = connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();
        
        let (tx, mut rx) = mpsc::channel::<String>(32);
        let (std_tx, std_rx) = std::sync::mpsc::channel::<String>();
        
        let task = tokio::spawn(async move {
            log::info!("[SOW-CLIENT] Background network task started!");
            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        if let Some(text) = msg {
                            // log::debug!("[SOW-CLIENT] Sending msg to websocket: {}", text);
                            if let Err(e) = write.send(Message::Text(text.into())).await {
                                log::error!("[SOW-CLIENT] Write error: {}", e);
                                break;
                            }
                        } else {
                            log::warn!("[SOW-CLIENT] rx.recv() returned None!");
                            break;
                        }
                    }
                    msg = read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                // log::debug!("[SOW-CLIENT] Received WS msg: {}", text);
                                if std_tx.send(text.to_string()).is_err() {
                                    log::error!("[SOW-CLIENT] std_tx.send failed!");
                                    break;
                                }
                            }
                            Some(Ok(_other)) => {
                                // log::debug!("[SOW-CLIENT] Received non-text WS msg");
                            }
                            Some(Err(e)) => {
                                log::error!("[SOW-CLIENT] read.next() error: {}", e);
                                break;
                            }
                            None => {
                                log::warn!("[SOW-CLIENT] read.next() returned None!");
                                break;
                            }
                        }
                    }
                }
            }
            log::info!("[SOW-CLIENT] Background network task exited!");
        });

        Ok(Self { tx, rx: std_rx, _task: task })
    }

    pub fn send(&self, msg: String) {
        let _ = self.tx.try_send(msg);
    }
}
