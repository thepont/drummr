# drummr

A low-latency, polymorphic MIDI drum sound generator built in Rust with a decoupled React/TypeScript UI.

## Features

- **Polymorphic Sound Engines**: 
    - **FM Engine**: Classic frequency modulation with a dynamic "Sizzle" noise layer.
    - **Physical Modeling Engine**: Karplus-Strong based resonance modeling for organic, textured sounds.
- **Real-Time Control**: Modern, responsive UI for sculpting sounds and mapping MIDI in real-time.
- **Low Latency**: Optimized for Linux/ARM standalone hardware with sub-5ms target latency.
- **Persistence**: Live-first model; all changes are instantly saved to `kit.toml`.

## Tech Stack

- **Backend**: Rust, `cpal` (audio), `midir` (MIDI), `tokio` (async/WebSockets).
- **Frontend**: Vite, React, TypeScript, Tailwind CSS.
- **Communication**: High-speed WebSockets for parameter sync and MIDI visualization.

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

Sounds and MIDI mappings are stored in `kit.toml`. The engine dynamically loads these settings on startup and reloads them when critical changes (like engine type) are made via the UI.
