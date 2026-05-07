#[cfg(test)]
mod tests {
    use wmidi::*;

    #[test]
    fn test_parse_note_on() {
        let bytes = [0x90, 0x3C, 0x64]; // Note On, Channel 1, C3, Velocity 100
        let message = MidiMessage::from_bytes(&bytes).unwrap();
        
        if let MidiMessage::NoteOn(channel, note, velocity) = message {
            assert_eq!(channel, Channel::Ch1);
            assert_eq!(note, Note::C4);
            assert_eq!(velocity, 100u8.try_into().unwrap());
        } else {
            panic!("Expected NoteOn message");
        }
    }
}
