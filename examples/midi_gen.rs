use midir::{MidiOutput, MidiOutputConnection};
use std::thread::sleep;
use std::time::Duration;
use anyhow::Result;

fn main() -> Result<()> {
    let midi_out = MidiOutput::new("drummr-generator")?;
    let out_ports = midi_out.ports();
    
    // Look for "Midi Through" or just take the first one
    let port = out_ports.iter()
        .find(|p| midi_out.port_name(p).unwrap_or_default().contains("Through"))
        .unwrap_or_else(|| &out_ports[0]);

    let name = midi_out.port_name(port)?;
    println!("Generating MIDI on: {}", name);

    let mut conn: MidiOutputConnection = midi_out.connect(port, "drummr-gen-conn")
        .map_err(|e| anyhow::anyhow!("Failed to connect: {}", e))?;

    println!("Sending Note On/Off every second... (Ctrl+C to stop)");
    loop {
        // Note On (Channel 10, Note 36 (Kick), Velocity 100)
        // Note On on Channel 10 is 0x99 (0x90 + 9)
        conn.send(&[0x99, 36, 100])?;
        println!("Sent Note On");
        sleep(Duration::from_millis(100));
        
        // Note Off
        conn.send(&[0x89, 36, 0])?;
        sleep(Duration::from_millis(900));
    }
}
