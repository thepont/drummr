use drummr::midi::MidiEngine;
use drummr::comm::CommEngine;
use std::sync::Arc;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting drummr engine...");

    // Initialize Comm Engine (WebSockets)
    let comm_engine = Arc::new(CommEngine::new());
    comm_engine.start("127.0.0.1:8080").await?;

    // Initialize MIDI
    let mut midi_engine = MidiEngine::new();
    let comm_for_midi = comm_engine.clone();
    
    match midi_engine.start(move |msg| {
        println!("Received MIDI message: {:?}", msg);
        let msg_str = format!("{:?}", msg);
        let comm = comm_for_midi.clone();
        tokio::spawn(async move {
            comm.broadcast(msg_str).await;
        });
    }) {
        Ok(_) => println!("MIDI engine started."),
        Err(e) => eprintln!("Failed to start MIDI engine: {}", e),
    }

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");

    println!("Output device: {}", device.name()?);

    let config = device.default_output_config()?;
    println!("Default output config: {:?}", config);

    let sample_rate = config.sample_rate().0 as f32;
    let mut sample_clock = 0f32;

    let stream = device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            for sample in data.iter_mut() {
                *sample = (sample_clock * 440.0 * 2.0 * std::f32::consts::PI / sample_rate).sin();
                sample_clock = (sample_clock + 1.0) % sample_rate;
            }
        },
        |err| eprintln!("an error occurred on stream: {}", err),
        None,
    )?;

    stream.play()?;

    println!("Playing sine wave for 10 seconds...");
    std::thread::sleep(std::time::Duration::from_secs(10));

    Ok(())
}
