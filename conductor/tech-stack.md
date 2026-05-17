# Tech Stack: drummr

## Core Audio Engine (Rust, edition 2024)
- **Audio I/O:** `cpal` 0.15 - Low-level, cross-platform audio input/output for ultra-low latency.
- **Live MIDI:** `wmidi` 4.0 - Zero-copy parsing of incoming MIDI messages on the live MIDI port.
- **MIDI ports / sync output:** `midir` 0.9 - Cross-platform MIDI port enumeration; also creates the virtual `drummr Sync Out` port on Linux (ALSA/JACK) for master-clock distribution.
- **MIDI file playback:** `midly` 0.5 - Zero-copy MIDI-file parser used by the Preview Kit feature to play back the bundled Groove MIDI tracks (`presets/midi/*.mid`).
- **Lock-free audio queue:** `rtrb` 0.3 - Single-producer / single-consumer ring buffers carrying `MidiEvent` and `AudioCommand` from the tokio runtime to the cpal callback.
- **Asynchronous Runtime:** `tokio` 1.x - WebSocket communication, MIDI input, broadcast loops, persistence-thread bridge via `mpsc::UnboundedSender`.
- **WebSocket transport:** `tokio-tungstenite` 0.26 - Server-side WebSocket implementation for the UI's text-prefix protocol.
- **Misc lock-free:** `arc-swap` 1.x, `arrayvec` 0.7, `lazy_static` 1.x.

## Frontend UI
- **Framework:** **React** + Vite - For building a modular and interactive user interface.
- **Language:** **TypeScript** - To ensure type safety and improve maintainability in the frontend.
- **Styling:** Tailwind CSS - Utility-first class-based styling, matches the dark monospace aesthetic of the audio designer.
- **Iconography:** `@phosphor-icons/react` - Phosphor icon set, used for control affordances and info tooltips.
- **Communication:** Standard Web Browser WebSockets API for low-latency feedback from the Rust engine.

## Configuration & Data
- **Format:** **TOML** - Human-readable and specifically designed for configuration files. Used for `kit.toml`, `mapping.toml`, `settings.toml`, and every preset under `presets/kits/` and `presets/sounds/`.
- **Serialization:** `serde` + `serde_with` + `toml` + `serde_json` - serde drives both the TOML on-disk schema and the JSON over-the-wire payloads (e.g. `KIT: <json>`, `ANALYSIS:<slot>|<json>`).

## Target Platforms
- **Primary:** Linux (specifically optimized for performance and low-latency).
- **Embedded:** ARM (e.g., Raspberry Pi) with support for headless operation.
- **Future:** DAW Integration (e.g., VST3/CLAP) using libraries like `nih-plug`.

## Research & Analysis Tools
- **Web Investigation:** `google_web_search` and `web_fetch` for analyzing existing systems and documentation.
- **Synthesis Research:** Academic whitepapers on physical modeling and percussive synthesis techniques.
- **Open Source Benchmarking:** Source code analysis of established projects like Hydrogen and Surge XT to identify best practices.
