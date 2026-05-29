#[cfg(test)]
mod tests {
    use cpal::traits::HostTrait;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use rtrb::RingBuffer;
    use drummr::audio::start_audio;
    use drummr::state::{SharedState, MidiEvent, AudioCommand};
    use drummr::kit::{KitEngine, DrumKit};

    #[test]
    fn test_default_host_exists() {
        let _host = cpal::default_host();
    }

    #[test]
    fn test_can_build_stream() {
        let host = cpal::default_host();
        if let Some(device) = host.default_output_device() {
            let (_, midi_cons) = RingBuffer::<MidiEvent>::new(10);
            let (_, cmd_cons) = RingBuffer::<AudioCommand>::new(10);
            let (error_tx, _) = mpsc::unbounded_channel();
            
            let kit = KitEngine::new(48000.0);
            let snapshot = DrumKit { name: "test".into(), description: None, sounds: vec![] };
            let shared_state = Arc::new(SharedState::new(snapshot, vec![], error_tx.clone()));

            let stream_res = start_audio(&device, midi_cons, cmd_cons, kit, shared_state, error_tx, None);
            assert!(stream_res.is_ok(), "Failed to build stream: {:?}", stream_res.err());
        }
    }
}
