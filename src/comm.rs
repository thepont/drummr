use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::accept_async;

type Sender = tokio::sync::mpsc::UnboundedSender<String>;

pub struct CommEngine {
    senders: Arc<Mutex<Vec<Sender>>>,
}

impl CommEngine {
    pub fn new() -> Self {
        Self {
            senders: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn start<F, Fut>(&self, addr: &str, on_message: F) -> Result<std::net::SocketAddr>
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        println!("WebSocket server listening on: {}", local_addr);

        let senders = self.senders.clone();
        let on_message = Arc::new(on_message);

        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let senders = senders.clone();
                let on_message = on_message.clone();

                tokio::spawn(async move {
                    if let Ok(ws_stream) = accept_async(stream).await {
                        println!("New WebSocket client connected.");
                        let (mut write, mut read) = ws_stream.split();

                        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
                        if let Ok(mut s_lock) = senders.lock() {
                            s_lock.push(tx);
                        }

                        let write_task = tokio::spawn(async move {
                            while let Some(msg) = rx.recv().await {
                                if let Err(e) = write
                                    .send(tokio_tungstenite::tungstenite::Message::Text(msg.into()))
                                    .await
                                {
                                    eprintln!("WS Write Error: {}", e);
                                    break;
                                }
                            }
                        });

                        while let Some(Ok(msg)) = read.next().await {
                            if let Ok(text) = msg.into_text() {
                                on_message(text.to_string()).await;
                            }
                        }

                        write_task.abort();
                        println!("WebSocket client disconnected.");
                    }
                });
            }
        });

        Ok(local_addr)
    }

    /// Add an in-process subscriber that receives every subsequent broadcast.
    /// Used by integration tests to capture broadcasts without standing up a
    /// real WebSocket client. The returned receiver mirrors the channel the
    /// real WS write-task drains, so test capture follows the same code path
    /// as production.
    pub fn subscribe(&self) -> mpsc::UnboundedReceiver<String> {
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        if let Ok(mut s_lock) = self.senders.lock() {
            s_lock.push(tx);
        }
        rx
    }

    pub fn broadcast(&self, message: String) {
        if let Ok(mut senders) = self.senders.lock() {
            let count_before = senders.len();
            senders.retain(|tx| tx.send(message.clone()).is_ok());
            let count_after = senders.len();
            if count_after < count_before {
                println!(
                    "Cleaned up {} disconnected WS clients",
                    count_before - count_after
                );
            }
        }
    }
}
