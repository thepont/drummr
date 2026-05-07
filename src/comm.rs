use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};
use anyhow::Result;

type Clients = Arc<Mutex<Vec<futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>, tokio_tungstenite::tungstenite::Message>>>>;

pub struct CommEngine {
    clients: Clients,
}

impl CommEngine {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn start(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        println!("WebSocket server listening on: {}", addr);

        let clients = self.clients.clone();

        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let clients = clients.clone();
                tokio::spawn(async move {
                    if let Ok(ws_stream) = accept_async(stream).await {
                        let (write, _read) = ws_stream.split();
                        clients.lock().unwrap().push(write);
                        println!("New WebSocket client connected.");
                    }
                });
            }
        });

        Ok(())
    }

    pub async fn broadcast(&self, message: String) {
        let mut sinks = {
            let mut clients = self.clients.lock().unwrap();
            clients.drain(..).collect::<Vec<_>>()
        };

        let mut active_sinks = Vec::new();
        for mut sink in sinks {
            if let Ok(_) = sink.send(tokio_tungstenite::tungstenite::Message::Text(message.clone().into())).await {
                active_sinks.push(sink);
            }
        }

        let mut clients = self.clients.lock().unwrap();
        clients.extend(active_sinks);
    }
}
