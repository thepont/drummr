# Percussive Synthesis Strategies for drummr

## Physical Modeling (PM)

### Karplus-Strong / Digital Waveguide
- **Principle:** A burst of noise (exciter) into a short delay line with a low-pass filter in the feedback loop (resonator).
- **Application:** Snare "body" resonance, Rimshots, or Woodblock sounds.

### Modal Synthesis
- **Principle:** Summing multiple damped sine wave oscillators, each representing a "mode" of vibration of a drum head or metallic object.
- **Application:** High-quality kicks and snares.
- **Key Parameters:** Frequency of each mode, decay time per mode, and excitation intensity.

---

## FM Synthesis (Frequency Modulation)

### Kick Drum (The "DX Kick")
- **Algorithm:** Operator 2 (Modulator) $\rightarrow$ Operator 1 (Carrier).
- **Carrier:** Fixed freq (40-60Hz) or 1.0 ratio.
- **Modulator:** 1.0 - 2.0 ratio.
- **Transient:** Use a separate operator or a fast pitch envelope (start at 1-2kHz, drop to 50Hz in 30ms).
- **Feedback:** Add to modulator for more "bite."

### Snare Drum
- **Structure:**
    - **Body:** 2-operator FM pair tuned to 180-250Hz.
    - **Wires:** White noise generator or FM operator with very high ratio (10.0+) and high feedback.
- **Filter:** Band-pass or High-pass on the "wires" component.

### Hi-Hat & Cymbals
- **Goal:** Metallic, non-repeating noise.
- **Ratios:** Use inharmonic prime-related ratios: **3.51, 5.87, 9.13**.
- **Technique:** High feedback on modulators creates "pitched noise" which is more expressive than static white noise.
- **Envelope:** Exponential decay is critical for realism.

### Cowbell
- **Ratios:** The classic "fifth-ish" interval: **1.0 : 1.48**.
- **Algorithm:** Two parallel carriers, each modulated by its own modulator to create two distinct "clangs" that beat against each other.

---

## Synthesis Implementation Roadmap
1. **Module 1: Basic Sine/Noise Oscillators.**
2. **Module 2: ADSR/Pitch Envelopes.**
3. **Module 3: FM Operator Matrix.**
4. **Module 4: Simple Karplus-Strong Resonator.**
