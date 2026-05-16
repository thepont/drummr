use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::Consumer;
use std::sync::Arc;
use crate::state::{SharedState, AudioCommand, MidiEvent};

use crate::dsp::bpm_engine::BpmEngine;
use tokio::sync::Mutex;

pub fn start_audio(
    device: &cpal::Device, 
    mut event_rx: Consumer<MidiEvent>, 
    mut cmd_rx: Consumer<AudioCommand>, 
    shared_state: Arc<SharedState>,
    bpm_engine: Arc<Mutex<BpmEngine>>,
) -> Result<(cpal::Stream, Option<cpal::Stream>)> {
    let config_supported = device.default_output_config()?;
    let mut config: cpal::StreamConfig = config_supported.into();
    config.buffer_size = cpal::BufferSize::Fixed(128);
    let channels = config.channels as usize;

    // 1. Output Stream (Synthesizer)
    let output_stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            // ... (keep synthesizer logic as is)
            if let Ok(mut kit) = shared_state.kit.try_lock() {
                while let Ok(cmd) = cmd_rx.pop() {
                    match cmd {
                        AudioCommand::SetParam(slot, param, val) => { kit.set_param(slot, &param, val); }
                        AudioCommand::SetMod(slot, param, source, depth) => {
                            if let Some(voice_opt) = kit.voices.get_mut(slot) {
                                if let Some(voice) = voice_opt { voice.set_mod(&param, source, depth); }
                            }
                        }
                        AudioCommand::SetLfo(slot, index, freq) => {
                            if let Some(voice_opt) = kit.voices.get_mut(slot) {
                                if let Some(voice) = voice_opt { voice.set_lfo(index, freq); }
                            }
                        }
                    }
                }

                while let Ok(msg) = event_rx.pop() {
                    let status = msg[0];
                    let note = msg[1];
                    let velocity = msg[2] as f32 / 127.0;
                    if status >= 0x80 && status <= 0x9F {
                        kit.trigger(note, velocity);
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
                    let out = soft_clip(kit.tick() * 0.7);
                    for sample in frame.iter_mut() { *sample = out; }
                }
            } else {
                for sample in data.iter_mut() { *sample = 0.0; }
            }
        },
        |_err| {},
        None
    )?;
    output_stream.play()?;

    // 2. Input Stream (BPM Detection)
    let host = cpal::default_host();
    let input_stream = if let Some(input_device) = host.default_input_device() {
        let input_config: cpal::StreamConfig = input_device.default_input_config()?.into();
        let bpm_clone = bpm_engine.clone();
        
        let stream = input_device.build_input_stream(
            &input_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Ok(mut bpm) = bpm_clone.try_lock() {
                    bpm.process_audio(data);
                }
            },
            |_err| {},
            None
        )?;
        stream.play()?;
        Some(stream)
    } else {
        None
    };

    Ok((output_stream, input_stream))
}

pub fn soft_clip(x: f32) -> f32 {
    // Tanh-based soft clipping for harmonic warmth and hard-limit at 1.0
    x.tanh()
}

pub fn get_default_audio_device() -> Result<cpal::Device> {
    let host = cpal::default_host();
    let device = host.default_output_device()
        .ok_or_else(|| anyhow::anyhow!("No default output device found"))?;
    Ok(device)
}
