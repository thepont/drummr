use crate::state::{AudioCommand, MidiEvent, SharedState};
use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::Consumer;
use std::sync::Arc;

pub fn start_audio(
    device: &cpal::Device,
    mut event_rx: Consumer<MidiEvent>,
    mut cmd_rx: Consumer<AudioCommand>,
    shared_state: Arc<SharedState>,
    error_tx: tokio::sync::mpsc::UnboundedSender<()>,
) -> Result<cpal::Stream> {
    let config_supported = device.default_output_config()?;
    
    // Request low latency but don't hard-lock if the device refuses.
    // This makes the engine portable across different hardware.
    let buffer_size = match config_supported.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max } => {
            cpal::BufferSize::Fixed(128.clamp(*min, *max))
        }
        _ => cpal::BufferSize::Default,
    };

    let mut config: cpal::StreamConfig = config_supported.into();
    config.buffer_size = buffer_size;
    let channels = config.channels as usize;

    let output_stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            if let Ok(mut kit) = shared_state.kit.try_lock() {
                // Snapshot the live BPM once per audio block. Reads are
                // lock-free (Relaxed atomic) and the value is stable for the
                // duration of this callback, so every note-on processed below
                // sees a consistent tempo.
                let bpm = shared_state.load_bpm();
                while let Ok(cmd) = cmd_rx.pop() {
                    match cmd {
                        AudioCommand::SetParam(slot, param, val) => {
                            kit.set_param(slot, &param, val);
                        }
                        AudioCommand::SetMod(slot, param, source, depth) => {
                            if let Some(voice_opt) = kit.voices.get_mut(slot) {
                                if let Some(voice) = voice_opt {
                                    voice.set_mod(&param, source, depth);
                                }
                            }
                        }
                        AudioCommand::SetLfo(slot, index, freq) => {
                            if let Some(voice_opt) = kit.voices.get_mut(slot) {
                                if let Some(voice) = voice_opt {
                                    voice.set_lfo(index, freq);
                                }
                            }
                        }
                        AudioCommand::SetPostFx(slot, param, val) => {
                            kit.set_postfx(slot, &param, val);
                        }
                        AudioCommand::SetGenerative(slot, param, val) => {
                            kit.set_generative(slot, &param, val);
                        }
                        AudioCommand::SetDivision(slot, param, div) => {
                            kit.set_division(slot, &param, div);
                        }
                    }
                }

                while let Ok(msg) = event_rx.pop() {
                    let status = msg[0];
                    let note = msg[1];
                    let velocity_raw = msg[2];
                    // Drums are one-shots: trigger on NoteOn only, ignore NoteOff so the
                    // envelope rings out after the stick lifts. Also treat "NoteOn with
                    // velocity 0" (the running-status NoteOff convention) as a release.
                    if (0x90..=0x9F).contains(&status) && velocity_raw > 0 {
                        kit.trigger(note, velocity_raw as f32 / 127.0, bpm);
                    }
                }

                for (slot_idx, voice_opt) in kit.voices.iter().enumerate() {
                    if let Some(voice) = voice_opt {
                        if voice.is_active() {
                            let vals = voice.get_mod_values();
                            for (src_idx, &val) in vals.iter().enumerate() {
                                shared_state.set_value(slot_idx, src_idx, val);
                            }
                        }
                    }
                }

                for frame in data.chunks_mut(channels) {
                    let mut out = soft_clip(kit.tick() * 0.7);
                    if !out.is_finite() { out = 0.0; }
                    for sample in frame.iter_mut() {
                        *sample = out;
                    }
                }
            } else {
                for sample in data.iter_mut() {
                    *sample = 0.0;
                }
            }
        },
        move |err| {
            eprintln!("audio output stream error: {}", err);
            // Notify the tokio recovery task. The receiver lives on
            // SharedState's `audio_error_tx`. Sending an unbounded mpsc is
            // non-blocking and lock-free, which is mandatory inside a cpal
            // error callback (runs on the audio thread).
            let _ = error_tx.send(());
        },
        None,
    )?;
    output_stream.play()?;

    Ok(output_stream)
}

pub fn soft_clip(x: f32) -> f32 {
    // Tanh-based soft clipping for harmonic warmth and hard-limit at 1.0
    x.tanh()
}

pub fn get_default_audio_device() -> Result<cpal::Device> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("No default output device found"))?;
    Ok(device)
}
