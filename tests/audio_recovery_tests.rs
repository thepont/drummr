#[cfg(test)]
mod tests {
    use std::sync::Arc;
    
    use drummr::state::SharedState;
    use drummr::kit::DrumKit;

    fn build_state() -> Arc<SharedState> {
        let kit = DrumKit {
            name: "test".into(),
            description: None,
            sounds: vec![],
        };
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        Arc::new(SharedState::new(kit, vec![], tx))
    }

    #[test]
    fn test_shared_state_initialization() {
        let state = build_state();
        assert_eq!(state.load_bpm(), 120.0);
    }
}
