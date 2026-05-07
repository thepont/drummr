# State-of-the-Art Review: Low-Latency Audio & Drum Synthesis

## Low-Latency Rust Audio Libraries

| Library | Pros | Cons | Latency |
| :--- | :--- | :--- | :--- |
| **CPAL** | Pure Rust, Cross-platform, Active. | Backend-dependent latency. | Low (WASAPI/ALSA) |
| **JACK** | Pro-audio standard, Inter-app routing. | Requires external server. | Ultra-Low |
| **ASIO** | Windows pro-audio standard. | Licensing, requires drivers. | Ultra-Low |
| **PortAudio** | Mature, Stable. | C-wrapper, harder to deploy. | Low |

### Recommendation for drummr
- **Primary:** `cpal` for its Rust-native ecosystem and ease of use.
- **Advanced:** Enable `jack` and `asio` features in `cpal` for professional-grade performance on Linux and Windows.

## Drum Synthesis Logic Analysis

### Core Techniques
1. **Subtractive Synthesis:** Filtering noise (white/pink) for snares and cymbals.
2. **Frequency Modulation (FM):** Creating metallic, inharmonic textures for cowbells and cymbals.
3. **Physical Modeling (Karplus-Strong):** Using delay lines to simulate drum head vibrations.
4. **Wavetable:** Pre-computed waveforms for complex percussive attacks.

### Per-Instrument Strategies
- **Kick:** Sine wave + extremely fast pitch envelope (attack) + fast amplitude decay.
- **Snare:** Sine wave (body) + White noise burst (snap) + High-pass filter.
- **Hi-Hat:** High-passed white noise or FM with metallic ratios + short decay.

## Reference Projects
- **Hydrogen:** Excellent layered approach (samples + synthesis).
- **Surge XT:** Implements Mutable Instruments' Plaits models (Physical Modeling).
- **Rudiments:** A Rust-based drum machine implementation.
