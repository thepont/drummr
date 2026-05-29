use midir::MidiInput;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let midi_in = MidiInput::new("drummr-list")?;
    let ports = midi_in.ports();
    println!("Found {} MIDI input ports:", ports.len());
    for (i, p) in ports.iter().enumerate() {
        println!("{}: {}", i, midi_in.port_name(p)?);
    }
    Ok(())
}
