use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub struct SowClient {
    tx: mpsc::Sender<String>,
    _task: JoinHandle<()>,
}

impl SowClient {
    pub async fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let (ws_stream, _) = connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();
        
        let (tx, mut rx) = mpsc::channel::<String>(32);
        
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        if let Some(text) = msg {
                            if write.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(Ok(msg)) = read.next() => {
                        if let Message::Text(text) = msg {
                            log::info!("Native received: {}", text);
                        }
                    }
                }
            }
        });

        Ok(Self { tx, _task: task })
    }

    pub fn send(&self, msg: String) {
        let _ = self.tx.try_send(msg);
    }
}
