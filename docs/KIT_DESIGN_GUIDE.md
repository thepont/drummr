# drummr: Kit Design Guide

This guide is the definitive reference for designing sounds and kits in `drummr`. It covers the synthesis engines, the modulation system, and the generative rhythmic features that allow kits to "bend the laws of space-time."

---

## 1. Synthesis Engines

`drummr` features six specialized synthesis engines. Each slot (0–15) can be assigned any one of these engines.

### FM (Frequency Modulation)
The "bread and butter" of digital drum synthesis.
- **`freq`**: Fundamental frequency (Carrier).
- **`mod_ratio`**: Ratio of Modulator to Carrier. Integer ratios (1, 2, 3) are harmonic; non-integers (1.414, 3.51) create metallic, inharmonic clangs.
- **`mod_index`**: Depth of modulation. Higher values add more sidebands/brightness.
- **`noise_level`**: A built-in noise burst for snare "snap" or hi-hat "shimmer."

### Phys (Physical Modeling)
A Karplus-Strong waveguide resonator.
- **`freq`**: Tension/tuning of the modeled object.
- **`brightness`**: How much high-frequency content is excited.
- **`dampening`**: How quickly the physical resonance dies away.
- **`attack`**: Excitation burst length.

### Granular
Particles and clouds.
- **`freq`**: Base pitch of the grain cloud.
- **`density`**: How many grains overlap. Low = sparse clicks; High = dense swarm.
- **`grain_size`**: Length of individual grains (ms).
- **`jitter`**: Randomness in grain timing. 0 = periodic; 1 = chaotic/noisy.

### Hybrid
Pitched oscillator mixed with filtered noise.
- **`freq`**: Oscillator pitch.
- **`noise_color`**: Filter character for the noise (0 = Dark, 1 = Sharp).
- **`metallic`**: Mix between the oscillator and noise. High = metallic clang.

### Modal
Summed sine partials (Modes).
- **`freq`**: Fundamental pitch.
- **`brightness`**: Excitation level of higher modes.
- **`dampening`**: Resonance decay.
- **`inharmonicity`**: Shifts partials away from harmonic series. 0 = Marimba/Bell; 1 = Drum head/Plate.
- **`mode_list`**: (Advanced) Explicitly define up to 12 `{freq, q, gain}` partials for hardware-faithful emulations (e.g., 808 Cowbell).

### Noise
Pure filtered noise with an envelope.
- Simple, efficient noise source for classic analog-style hats and snares.

---

## 2. The Modulation Matrix

Modulation is what makes a kit feel alive. Every engine parameter can be a **destination**.

### Sources
- **Envelope**: A dedicated Attack/Decay envelope triggered on every hit.
- **LFO 1 & 2**: Internal oscillators. Can be set in **Hz** or **Beat Divisions** (e.g., `1/4`, `1/8T`, `Bar`).
- **Velocity**: The MIDI velocity of the incoming note.

### Common "Space-Time" Routings
- **Velocity → Mod Index**: Harder hits are brighter and more complex.
- **Envelope → Inharmonicity**: A sound that starts as a pure bell and "shatters" into noise as it decays.
- **LFO → Panning**: (Upcoming) Sounds that orbit the listener at tempo-locked speeds.

---

## 3. Generative Features

`drummr` allows a single MIDI note to trigger complex rhythmic events.

### Sub-Hits (Claps & Drags)
Fixed-millisecond retriggers.
- **`offset_ms`**: Time after the primary hit.
- **`velocity_factor`**: Scaling of the sub-hit's volume.
- *Use case*: Classic 4-tap LinnDrum claps (~12ms spacing).

### Patterns (Polyrhythms)
Tempo-locked sequences triggered by a single hit.
- **`division`**: `1/16`, `1/8T`, etc.
- **`multiplier`**: How many divisions to wait.
- *Use case*: A single MIDI 4/4 pulse can trigger a complex 16th-note hat pattern inside the kit.

### Ghosts & Probabilities
- **`trigger_probability`**: Chance that a hit fires at all. Cures the "machine-gun" effect.
- **`ghost_probability`**: Chance of a soft, delayed echo firing after a hit.
- **`velocity_jitter`**: Small random variations in volume for human feel.

---

## 4. Tempo Locking

Parameters like **Decay** and **LFO Rate** can be locked to the live BPM using `BeatDivision`.
- **`decay_division`**: Set a snare to ring for exactly `TwoBars`, regardless of how fast the track is.
- **`lfo_division`**: Synchronize a pulsing filter sweep to the quarter note.
