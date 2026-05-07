use midir::{MidiInput, MidiInputConnection};
use wmidi::MidiMessage;
use anyhow::{Result, anyhow};

pub struct MidiEngine {
    _connection: Option<MidiInputConnection<()>>,
}

impl MidiEngine {
    pub fn new() -> Self {
        Self { _connection: None }
    }

    pub fn start<F>(&mut self, callback: F) -> Result<()>
    where
        F: Fn(MidiMessage) + Send + 'static,
    {
        let midi_in = MidiInput::new("drummr input")?;
        let ports = midi_in.ports();
        
        if ports.is_empty() {
            return Err(anyhow!("no MIDI input ports available"));
        }

        let port = &ports[0];
        println!("Connecting to MIDI port: {}", midi_in.port_name(port)?);

        let _conn = midi_in.connect(
            port,
            "drummr-read-input",
            move |_timestamp, data, _| {
                if let Ok(message) = MidiMessage::from_bytes(data) {
                    callback(message);
                }
            },
            (),
        ).map_err(|e| anyhow!("failed to connect to MIDI port: {}", e))?;

        self._connection = Some(_conn);
        Ok(())
    }
}
