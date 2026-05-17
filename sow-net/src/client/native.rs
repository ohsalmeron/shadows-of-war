use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub struct SowClient {
    tx: mpsc::UnboundedSender<Vec<u8>>,
    pub rx: std::sync::mpsc::Receiver<Vec<u8>>,
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
        
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (std_tx, std_rx) = std::sync::mpsc::channel::<Vec<u8>>();
        
        let task = tokio::spawn(async move {
            log::info!("[SOW-CLIENT] Background network task started!");
            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        if let Some(data) = msg {
                            // log::debug!("[SOW-CLIENT] Sending msg to websocket");
                            if let Err(e) = write.send(Message::Binary(data)).await {
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
                            Some(Ok(Message::Binary(data))) => {
                                // log::debug!("[SOW-CLIENT] Received WS msg");
                                if std_tx.send(data).is_err() {
                                    log::error!("[SOW-CLIENT] std_tx.send failed!");
                                    break;
                                }
                            }
                            Some(Ok(_other)) => {
                                // log::debug!("[SOW-CLIENT] Received non-binary WS msg");
                            }
                            Some(Err(e)) => {
                                log::debug!("[SOW-CLIENT] read.next() error: {}", e);
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

    pub fn send(&self, msg: Vec<u8>) {
        let _ = self.tx.send(msg);
    }
}
