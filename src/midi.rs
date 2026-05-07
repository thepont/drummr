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
        
        let port = ports.get(port_index)
            .ok_or_else(|| anyhow!("MIDI port index {} out of bounds", port_index))?;
        
        let port_name = midi_in.port_name(port)?;
        println!("Connecting to MIDI port: {}", port_name);

        let _conn = midi_in.connect(
            port,
            "drummr-read-input",
            move |_timestamp, data, _| {
                if !data.is_empty() {
                    println!("MIDI BYTES: {:?}", data);
                }
                if let Ok(message) = MidiMessage::from_bytes(data) {
                    callback(message);
                }
            },
            (),
        ).map_err(|e| anyhow!("failed to connect to MIDI port: {}", e))?;

        self._connection = Some(_conn);
        Ok(port_name)
    }
}
