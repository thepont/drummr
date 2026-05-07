#[cfg(test)]
mod tests {
    use tokio::net::TcpStream;
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
    use futures_util::{StreamExt, SinkExt};

    #[tokio::test]
    async fn test_websocket_connection() {
        use drummr::comm::CommEngine;
        use std::sync::Arc;

        let engine = Arc::new(CommEngine::new());
        engine.start("127.0.0.1:8081").await.unwrap();

        let (ws_stream, _) = connect_async("ws://127.0.0.1:8081").await.expect("Failed to connect");
        
        println!("Connected to WebSocket server");
        drop(ws_stream);
    }
}
