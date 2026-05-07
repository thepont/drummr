use drummr::comm::CommEngine;

#[tokio::test]
async fn test_comm_engine_broadcast() {
    let engine = CommEngine::new();
    
    // Test that we can start it with a dummy callback
    let _ = engine.start("127.0.0.1:0", |_| async {}).await;
    
    // Broadcast should not panic even with no clients
    engine.broadcast("test message".to_string()).await;
}
