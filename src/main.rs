use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use anyhow::Result;

fn main() -> Result<()> {
    println!("Starting drummr audio engine...");

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

    println!("Playing sine wave for 2 seconds...");
    std::thread::sleep(std::time::Duration::from_secs(2));

    Ok(())
}
