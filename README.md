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
- **27 kit presets** in `presets/kits/`: TR-808/909 emulations, Simmons/LinnDrum/RX5 character kits, all-physical and all-granular experimental kits, plus themed kits (Glass Forest, Office After Hours, Kitchen Sink Symphony, Garden 3AM, etc.).
- **BPM detection** via autocorrelation + tactus tracking on incoming MIDI onsets.
- **Master clock / Auto-Sync**: deterministic tempo sync engine with sub-harmonic and downbeat awareness.
- **MIDI mapping**: per-note routing to slot/velocity, with persistence.
- **Live-first persistence**: edits go to in-memory `SharedState` and are journalled to `kit.toml` / `mapping.toml` by a background worker using atomic rename.
- **Decoupled UI**: React + TypeScript editor talks to the backend over WebSockets; schema-driven param controls for every engine, real-time sparklines, frequency visualizer, and slot-aware modulation overlays.
- **Low latency**: targets sub-5ms round-trip on Linux/ARM standalone hardware via `cpal` direct-output streams.

## Tech Stack

- **Backend**: Rust, `cpal` (audio), `midir` (MIDI), `tokio` + `tokio-tungstenite` (WebSocket server), `rtrb` (lock-free audio-thread command queue).
- **Frontend**: Vite, React, TypeScript, Tailwind CSS.
- **Communication**: text-prefixed WebSocket protocol (`SET_PARAM:`, `LOAD_KIT:`, `KIT:`, `MIDI:`, etc.).

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

The backend anchors all paths to `CARGO_MANIFEST_DIR` at startup, so `cargo run` from anywhere finds the same data.
