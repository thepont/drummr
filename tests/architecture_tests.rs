use drummr::kit::{DrumKit, DrumSound, KitEngine};
use drummr::state::{SharedState, AudioCommand, StreamRequest};
use arc_swap::ArcSwap;
use std::sync::Arc;
use std::thread;
use rtrb::RingBuffer;
use tokio::sync::mpsc;

#[test]
fn test_arc_swap_snapshot_consistency() {
    let initial_kit = DrumKit {
        name: "Initial".to_string(),
        description: None,
        sounds: vec![DrumSound::default(); 16],
    };
    let (error_tx, _) = mpsc::unbounded_channel();
    let shared_state = Arc::new(SharedState::new(initial_kit, vec![], error_tx));

    let mut handles = vec![];
    
    // Writer threads: spamming different kit names
    for i in 0..10 {
        let ss = shared_state.clone();
        handles.push(thread::spawn(move || {
            for j in 0..100 {
                ss.kit_snapshot.rcu(|snap| {
                    let mut new_snap = (**snap).clone();
                    new_snap.name = format!("Thread {} - Update {}", i, j);
                    new_snap
                });
            }
        }));
    }

    // Reader threads: verifying we always get a valid DrumKit
    for _ in 0..5 {
        let ss = shared_state.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..200 {
                let snap = ss.kit_snapshot.load();
                assert!(snap.sounds.len() == 16);
                // Ensure name isn't corrupted (ArcSwap guarantees atomic pointer swap)
                assert!(snap.name.len() > 0);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn test_audio_command_ring_buffer_processing() {
    let (mut prod, mut cons) = RingBuffer::<AudioCommand>::new(100);
    
    // Send a few commands
    prod.push(AudioCommand::SetParam(0, "freq".to_string(), 440.0)).unwrap();
    prod.push(AudioCommand::SetParam(1, "freq".to_string(), 880.0)).unwrap();
    
    // Simulated audio callback drain
    let mut count = 0;
    while let Ok(cmd) = cons.pop() {
        match cmd {
            AudioCommand::SetParam(slot, param, val) => {
                if slot == 0 { assert_eq!(val, 440.0); }
                if slot == 1 { assert_eq!(val, 880.0); }
                assert_eq!(param, "freq");
                count += 1;
            }
            _ => panic!("Unexpected command"),
        }
    }
    assert_eq!(count, 2);
}

#[test]
fn test_supervisor_start_stop_logic() {
    let (tx, mut rx) = mpsc::unbounded_channel::<StreamRequest>();
    tx.send(StreamRequest::Stop).unwrap();
    let msg = rx.blocking_recv().unwrap();
    match msg {
        StreamRequest::Stop => {},
        _ => panic!("Expected Stop request"),
    }
}
