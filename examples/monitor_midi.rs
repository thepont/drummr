use midir::MidiInput;
use std::io::{stdin, stdout, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut midi_in = MidiInput::new("midi-monitor")?;
    let ports = midi_in.ports();
    let port = &ports[3]; // DDTi
    println!("Monitoring port: {}", midi_in.port_name(port)?);

    let _conn = midi_in.connect(port, "monitor-read", |_ts, data, _| {
        println!("RAW MIDI: {:?}", data);
    }, ())?;

    println!("Press Enter to stop...");
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    Ok(())
}
