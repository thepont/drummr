use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};
use anyhow::Result;
use tokio::sync::mpsc;

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

    pub async fn start<F, Fut>(&self, addr: &str, on_message: F) -> Result<()> 
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let listener = TcpListener::bind(addr).await?;
        println!("WebSocket server listening on: {}", addr);

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
                        senders.lock().unwrap().push(tx);
                        
                        let write_task = tokio::spawn(async move {
                            while let Some(msg) = rx.recv().await {
                                if let Err(e) = write.send(tokio_tungstenite::tungstenite::Message::Text(msg.into())).await {
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

        Ok(())
    }

    pub async fn broadcast(&self, message: String) {
        let mut senders = self.senders.lock().unwrap();
        let count_before = senders.len();
        senders.retain(|tx| {
            tx.send(message.clone()).is_ok()
        });
        let count_after = senders.len();
        if count_after < count_before {
            println!("Cleaned up {} disconnected WS clients", count_before - count_after);
        }
    }
}
