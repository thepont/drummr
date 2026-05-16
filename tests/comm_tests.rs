use drummr::comm::CommEngine;
use futures_util::{SinkExt, StreamExt};
use tokio::time::{Duration, sleep};
use tokio_tungstenite::connect_async;

#[tokio::test]
async fn test_comm_engine_broadcast() {
    let engine = CommEngine::new();

    // Test that we can start it with a dummy callback
    let addr = engine.start("127.0.0.1:0", |_| async {}).await.unwrap();

    // Broadcast should not panic even with no clients
    engine.broadcast("test message".to_string());

    let url = format!("ws://{}", addr);
    let (ws_stream, _) = connect_async(url.clone()).await.expect("Failed to connect");
    let (_, mut read) = ws_stream.split();

    engine.broadcast("hello client".to_string());

    if let Some(Ok(msg)) = read.next().await {
        assert_eq!(msg.to_text().unwrap(), "hello client");
    } else {
        panic!("Did not receive broadcast");
    }
}

#[tokio::test]
async fn test_multiple_clients_broadcast() {
    let engine = CommEngine::new();
    let addr = engine.start("127.0.0.1:0", |_| async {}).await.unwrap();
    let url = format!("ws://{}", addr);

    let mut clients = Vec::new();
    for _ in 0..3 {
        let (ws_stream, _) = connect_async(url.clone()).await.expect("Failed to connect");
        clients.push(ws_stream);
    }

    let message = "broadcast to all".to_string();
    engine.broadcast(message.clone());

    for mut ws in clients {
        let (_, mut read) = ws.split();
        if let Some(Ok(msg)) = read.next().await {
            assert_eq!(msg.to_text().unwrap(), &message);
        } else {
            panic!("Client did not receive broadcast");
        }
    }
}

#[tokio::test]
async fn test_client_disconnection_cleanup() {
    let engine = CommEngine::new();
    let addr = engine.start("127.0.0.1:0", |_| async {}).await.unwrap();
    let url = format!("ws://{}", addr);

    {
        let (ws_stream, _) = connect_async(url.clone()).await.expect("Failed to connect");
        // Connection is dropped here as ws_stream goes out of scope
        drop(ws_stream);
    }

    // Give it a moment to detect disconnection and for the write task to abort
    sleep(Duration::from_millis(100)).await;

    // Broadcast should trigger cleanup
    engine.broadcast("trigger cleanup".to_string());

    // There shouldn't be any active senders now (or at least it should be stable)
    // We can't easily check the private `senders` field, but we can verify it doesn't crash
    // and we can try to connect a new one and see it works.

    let (ws_stream, _) = connect_async(url.clone()).await.expect("Failed to connect");
    let (_, mut read) = ws_stream.split();

    engine.broadcast("new client".to_string());

    if let Some(Ok(msg)) = read.next().await {
        assert_eq!(msg.to_text().unwrap(), "new client");
    } else {
        panic!("New client did not receive broadcast");
    }
}

#[tokio::test]
async fn test_incoming_messages() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);

    let engine = CommEngine::new();
    let addr = engine
        .start("127.0.0.1:0", move |msg| {
            let tx = tx.clone();
            async move {
                tx.send(msg).await.unwrap();
            }
        })
        .await
        .unwrap();

    let url = format!("ws://{}", addr);
    let (mut ws_stream, _) = connect_async(url.clone()).await.expect("Failed to connect");

    let test_msg = "hello from client";
    ws_stream
        .send(tokio_tungstenite::tungstenite::Message::Text(
            test_msg.into(),
        ))
        .await
        .unwrap();

    let received = rx
        .recv()
        .await
        .expect("Did not receive message in callback");
    assert_eq!(received, test_msg);
}
