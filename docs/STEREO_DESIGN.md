# Stereo Design & Space-Time Bending

This document outlines the architectural plan for evolving `drummr` from a "Mono in a Stereo Container" engine to a fully spatial, modulatable stereo synthesizer.

## 1. The Philosophy: Space as a Parameter

In most drum machines, "Pan" is a static mixer setting. In `drummr`, we treat **Space** as a primary synthesis parameter, just like Frequency or Decay. By making the stereo field a destination in the modulation matrix, the "Space" becomes part of the rhythmic and timbral identity of the kit.

### Key Use Cases
- **Velocity Panning**: Sounds spread wider or move to a specific side the harder they are hit.
- **Envelope Panning**: A sound starts in the center and "travels" to the left or right as it decays.
- **LFO Panning**: Each of the 16 slots can orbit the listener at independent speeds and phases.
- **Generative Panning**: Ghost notes or pattern steps can be programmed to fire at different spatial coordinates than the primary hit.

## 2. Technical Implementation

### Constant-Power Panning
To ensure that sounds don't lose perceived volume as they move from center to side, we use a **Constant-Power Pan Law** (using Sin/Cos functions).

**The Math:**
- `angle = (pan + 1.0) * (PI / 4.0)`
- `left_gain = cos(angle)`
- `right_gain = sin(angle)`

Where `pan` is a value from `-1.0` (Hard Left) to `+1.0` (Hard Right).

### Backend Changes (`src/`)
1.  **`kit.rs`**: Add `pan: Option<f32>` to `DrumSound` and `KitEngine`.
2.  **`audio.rs`**: Update the mix loop to convert the mono `tick()` into a stereo frame:
    ```rust
    let (l_gain, r_gain) = calculate_pan_gains(slot_pan);
    left_sum += voice_sample * l_gain;
    right_sum += voice_sample * r_gain;
    ```
3.  **`modulation_engine.rs`**: Add `Pan` as a modulation destination. This allows `Envelope`, `LFO1`, `LFO2`, and `Velocity` to influence the final panning value per sample.

### Protocol Changes
- `SET_PARAM:slot|pan|value`: Sets the base panning for a slot.
- `SET_MOD:slot|pan|source|depth`: Routes a modulation source to the pan parameter.

## 3. UI/UX Design

### Kit Editor
- A new **Pan Slider** will be added to the main parameter panel for the selected slot.
- Visual feedback: A small "spatial dot" or L/R meter to show where the sound is currently sitting.

### Modulation Panel
- "Pan" will appear in the destination dropdown.
- When modulated, the Pan slider should show a "ghost" indicator of the real-time modulated position.

## 4. Future Stereo Effects (Roadmap)

Once the stereo bus is established, we intend to implement the following:

1.  **Master Plate Reverb**: A shared "room" that every voice can send to.
2.  **Ping-Pong Delay**: Tempo-locked delays that bounce between the Left and Right channels.
3.  **Stereo Width / Haas**: Per-voice micro-delays to create "super-wide" sounds that take up the whole stereo field.

## 5. Summary of Advantage

By implementing panning **per-voice** inside the engine (rather than at the DAW mixer), we allow the "Location" of a sound to be reactive, generative, and intrinsically linked to the performance velocity and timing.
