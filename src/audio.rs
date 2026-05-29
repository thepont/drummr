use crate::state::{AudioCommand, MidiEvent, SharedState};
use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::Consumer;
use std::sync::Arc;

pub fn start_audio(
    device: &cpal::Device,
    mut event_rx: Consumer<MidiEvent>,
    mut cmd_rx: Consumer<AudioCommand>,
    mut kit: crate::kit::KitEngine,
    shared_state: Arc<SharedState>,
    error_tx: tokio::sync::mpsc::UnboundedSender<()>,
    requested_buffer_size: Option<u32>,
) -> Result<cpal::Stream> {
    let config_supported = device.default_output_config()?;
    
    // Use requested buffer size or default to 256.
    let target_size = requested_buffer_size.unwrap_or(256);
    let buffer_size = match config_supported.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max } => {
            cpal::BufferSize::Fixed(target_size.clamp(*min, *max))
        }
        _ => cpal::BufferSize::Default,
    };

    let mut config: cpal::StreamConfig = config_supported.into();
    config.buffer_size = buffer_size;
    let channels = config.channels as usize;

    let output_stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            // One-time elevation of thread priority to Realtime (FIFO 95)
            // and CPU affinity pinning to shield from browser/OS jitter.
            static PRIORITY_SET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !PRIORITY_SET.load(std::sync::atomic::Ordering::Relaxed) {
                // Thread optimizations disabled for baseline stability
                PRIORITY_SET.store(true, std::sync::atomic::Ordering::Relaxed);
            }

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
                    AudioCommand::SetPan(slot, val) => {
                        if slot < 16 {
                            kit.pans[slot] = val.clamp(-1.0, 1.0);
                        }
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
                    AudioCommand::LoadKit(new_kit) => {
                        kit = *new_kit;
                    }
                    AudioCommand::LoadMapping(mappings) => {
                        kit.set_mapping(&mappings);
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

            let mut block_peak = 0.0f32;
            for frame in data.chunks_mut(channels) {
                let (out_l, out_r) = kit.tick();
                block_peak = block_peak.max(out_l.abs()).max(out_r.abs());
                
                // If the hardware is mono, sum them back.
                if channels == 1 {
                    frame[0] = (out_l + out_r) * 0.5;
                } else {
                    // Otherwise, interleaved stereo. 
                    // Most audio hardware has 2 channels. 
                    // If it has more, we duplicate L/R pairs or just 0 them.
                    if let Some(l) = frame.get_mut(0) { *l = out_l; }
                    if let Some(r) = frame.get_mut(1) { *r = out_r; }
                    for extra in 2..channels {
                        if let Some(s) = frame.get_mut(extra) { *s = 0.0; }
                    }
                }
            }
            shared_state.store_peak(block_peak);
        },
        move |err| {
            let err_str = format!("{}", err).to_lowercase();
            // Ignore informational changes and non-fatal xruns
            if err_str.contains("buffer size changed") || 
               err_str.contains("sample rate changed") ||
               err_str.contains("xrun") ||
               err_str.contains("buffer over or under run") {
                println!("[audio] Informational: {}", err_str);
                return;
            }
            
            eprintln!("audio output stream error: {}", err);
            // Notify the tokio recovery task for real fatal errors.
            let _ = error_tx.send(());
        },
        None,
    )?;
    output_stream.play()?;

    Ok(output_stream)
}

pub fn get_default_audio_device() -> Result<cpal::Device> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("No default output device found"))?;
    Ok(device)
}
