use anyhow::{Result, anyhow};
use midir::{MidiInput, MidiInputConnection};
use wmidi::MidiMessage;

pub struct MidiEngine {
    _connection: Option<MidiInputConnection<()>>,
}

impl MidiEngine {
    pub fn new() -> Self {
        Self { _connection: None }
    }

    pub fn list_ports() -> Result<Vec<String>> {
        let midi_in = MidiInput::new("drummr list")?;
        let ports = midi_in.ports();
        let mut names = Vec::new();
        for p in ports {
            names.push(midi_in.port_name(&p)?);
        }
        Ok(names)
    }

    pub fn start<F>(&mut self, port_index: usize, callback: F) -> Result<String>
    where
        F: Fn(MidiMessage) + Send + 'static,
    {
        // Stop existing connection if any
        self._connection = None;

        let midi_in = MidiInput::new("drummr input")?;
        let ports = midi_in.ports();

        let port = ports
            .get(port_index)
            .ok_or_else(|| anyhow!("MIDI port index {} out of bounds", port_index))?;

        let port_name = midi_in.port_name(port)?;
        println!("Connecting to MIDI port: {}", port_name);

        let _conn = midi_in
            .connect(
                port,
                "drummr-read-input",
                move |_timestamp, data, _| {
                    // Per-message stdout chatter was firing for EVERY raw MIDI
                    // byte stream (CC, pitch-bend, clock, aftertouch — none of
                    // which affects audio). On a controller streaming MIDI
                    // clock at 24 ppqn @ 120 BPM that's ~50 lines/sec into a
                    // line-buffered stdout from a foreign (midir) thread —
                    // not a leak but a real I/O-amplification issue worth
                    // killing. See docs/backend_leaks.md LOW.
                    if let Ok(message) = MidiMessage::from_bytes(data) {
                        callback(message);
                    }
                },
                (),
            )
            .map_err(|e| anyhow!("failed to connect to MIDI port: {}", e))?;

        self._connection = Some(_conn);
        Ok(port_name)
    }
}
