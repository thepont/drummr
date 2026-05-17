# drummr

A low-latency, polymorphic MIDI drum sound generator built in Rust with a decoupled React/TypeScript UI.

## Features

- **Six per-voice synthesis engines**, selectable per drum slot:
    - **FM** — 2-operator frequency modulation with a dynamic noise/sizzle layer.
    - **Physical modelling** — Karplus-Strong resonator for plucks, toms, and metallic ringing.
    - **Granular** — short-grain texture engine for shakers, hats, and atmospheres.
    - **Hybrid** — oscillator + noise blend with a `metallic` inharmonic partial set.
    - **Modal** — parallel resonator bank with Bessel-zero mode ratios for bells, plates, and tuned percussion.
    - **Noise** — coloured-noise voice with envelope shaping.
- **Per-voice post-FX**: bitcrusher (`bits`) and sample-rate reducer (`rate`) for lo-fi character (SP-1200 / LinnDrum / early FM-drum machine sounds).
- **Modulation matrix**: per-voice mod routing from Envelope, Velocity, LFO1, LFO2 to any engine parameter, plus two free-running LFOs per voice.
- **Clock-aware design**: per-slot tempo-locked LFO and envelope decay (`lfo1_division`, `lfo2_division`, `decay_division`), millisecond multi-tap clap envelopes (`sub_hits`), BPM-locked rhythm patterns (`pattern`), and generative trigger gates (`trigger_probability`, `ghost_probability`, `ghost_offset_ms`, `ghost_velocity_factor`). See `docs/research/clock_polyrhythm.md`.
- **30+ kit presets** in `presets/kits/`: TR-808/909 emulations, Simmons/LinnDrum/RX5 character kits, all-physical and all-granular experimental kits, themed kits (Glass Forest, Office After Hours, Kitchen Sink Symphony, Garden 3AM), and flagship clock-aware kits (Phase Mirror, Ghost Maker, Cathedral Forever, Polymeter Madness, Stutter Snare, Cathedral Bells, Glass Workshop, LoFi Crate, Servo Choir, Subzero), plus demo kits (Cowbell Demo, Clock Demo, Pattern Demo, Modal Demo).
- **BPM detection** via autocorrelation + tactus-prior tracking on incoming MIDI onsets and audio input.
- **Master clock / Auto-Sync**: deterministic tempo sync engine with sub-harmonic and downbeat awareness.
- **Preview Kit**: an in-engine MIDI track player loads any of the bundled CC-BY 4.0 Groove MIDI tracks (`presets/midi/*.mid`) to audition the active kit in a real musical context. Tracks are listed and started from the UI.
- **Per-slot analysis**: the engine renders each voice off-thread and broadcasts peak / RMS / clipping / silent diagnostics so the UI can flag dead or rail-pinned slots (`ANALYZE_SLOT` / `ANALYSIS:`).
- **MIDI mapping**: per-note routing to slot/velocity, with persistence.
- **Live-first persistence**: edits go to in-memory `SharedState` and are journalled to `kit.toml` / `mapping.toml` by a background worker using atomic rename.
- **Decoupled UI**: React + TypeScript editor talks to the backend over WebSockets; schema-driven param controls for every engine, real-time sparklines, frequency visualizer, and slot-aware modulation overlays.
- **Low latency**: targets sub-5ms round-trip on Linux/ARM standalone hardware via `cpal` direct-output streams.

## Tech Stack

- **Backend**: Rust (edition 2024), `cpal` (audio I/O), `wmidi` (live MIDI parsing), `midly` (MIDI-file parsing for Preview Kit), `midir` (live MIDI ports + virtual sync output), `tokio` + `tokio-tungstenite` (WebSocket server), `rtrb` (lock-free audio-thread command queue), `serde` + `toml` (config), `arrayvec` / `arc-swap` for lock-free state plumbing.
- **Frontend**: Vite, React, TypeScript, Tailwind CSS, `@phosphor-icons/react` (iconography).
- **Communication**: text-prefixed WebSocket protocol (`SET_PARAM:`, `SET_DIVISION:`, `LOAD_KIT:`, `KIT:`, `MIDI:`, `ANALYSIS:`, `MIDI_TRACK_PLAYING:`, etc. — see `src/commands.rs` and `ui/src/App.tsx`).

## Getting Started

### Backend
```bash
cargo run
```

### Frontend
```bash
cd ui
npm install
npm run dev
```

## Configuration

- `kit.toml` — currently loaded kit (auto-saved on edits).
- `mapping.toml` — MIDI note → slot mapping.
- `settings.toml` — last-chosen audio output and MIDI input device (gitignored; machine-local).
- `presets/kits/*.toml` — kit library shown in the UI Library sidebar.
- `presets/midi/*.mid` — curated CC-BY 4.0 drum tracks used by the **Preview Kit** feature to audition the active kit in a real musical context (see `presets/midi/LICENSE-MIDI.md` for attribution).

The backend anchors all paths to `CARGO_MANIFEST_DIR` at startup, so `cargo run` from anywhere finds the same data.

## Credits / Acknowledgements

- **Groove MIDI Dataset** (`presets/midi/*.mid`) — Gillick, J., Roberts, A., Engel, J., Eck, D., & Bamman, D. (2019). *Learning to Groove with Inverse Sequence Transformations*. ICML. Licensed under [Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/legalcode). See `presets/midi/LICENSE-MIDI.md` for full attribution.
- **Phosphor Icons** (via [`@phosphor-icons/react`](https://github.com/phosphor-icons/react)) — UI iconography. Licensed under MIT.
- **midly** (via [`midly`](https://crates.io/crates/midly)) — zero-copy MIDI file parser used by the Preview Kit playback engine. Licensed under MIT.

## License

The source code in this repository is currently **unlicensed** pending a decision by the author — i.e. all rights are reserved, and no permission is granted to copy, modify, or redistribute it. A formal open-source license may be added in the future.

The bundled MIDI files under `presets/midi/` are a separate matter: they originate from the Groove MIDI Dataset and are distributed here under **CC BY 4.0** per `presets/midi/LICENSE-MIDI.md`.
