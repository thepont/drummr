# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`drummr` is a low-latency MIDI drum sound generator. A Rust audio engine (the "backend") synthesizes sound and exposes a WebSocket API; a Vite/React/TypeScript SPA (the "UI") connects to it for parameter editing and real-time visualization. Target deployment is Linux/ARM (e.g. Raspberry Pi) with sub-5ms latency.

## Commands

### Backend (Rust, edition 2024)
```bash
cargo run                  # build + run the engine; listens on ws://127.0.0.1:8080
cargo build --release      # release build (use this on the target hardware)
cargo test                 # runs unit tests in src/ AND integration tests in tests/
cargo test --test fm_engine_tests   # run a single integration test file
cargo test test_mod_amount_default  # run a single test by name
```

The engine reads `kit.toml`, `mapping.toml`, `settings.toml` from the **current working directory** at startup. Always `cargo run` from the repo root.

### Frontend (in `ui/`)
```bash
cd ui
npm install
npm run dev          # Vite dev server; connects to ws://<hostname>:8080
npm run build        # tsc -b && vite build
npm test             # vitest run (one-shot)
npm run test:coverage
npm run lint
```

The UI assumes the backend is already running. There is no proxy — it opens a raw WebSocket to port 8080 on the same hostname it was served from.

## Architecture

### Three-thread audio model
The audio callback (cpal) MUST NOT block. Communication between threads uses lock-free `rtrb` ring buffers and atomics:

- **Audio thread** (cpal callback in `audio.rs`): pops `MidiEvent`s and `AudioCommand`s from ring buffers, ticks voices, writes samples. Uses `try_lock` on `SharedState.kit`; on contention it outputs silence rather than blocking.
- **Tokio runtime** (main thread, `main.rs`): WebSocket I/O, MIDI input handling, broadcast loops (mod-state at 40ms, BPM at 100ms). Pushes commands into the ring buffers.
- **Persistence thread** (`persistence.rs`): a dedicated `std::thread` consuming a tokio mpsc; writes `kit.toml` / `mapping.toml` / presets via tmp-file + rename for atomicity. Keeps file I/O off both audio and async threads.

`SharedState` (`state.rs`) holds an `[AtomicU32; 16*5]` of modulation source values (16 slots × 5 sources). The audio thread stores `f32::to_bits` into them per tick; the WS broadcast loop reads them out for the UI's real-time visualizers. This is the only inter-thread channel for "live values."

### DSP layout
`src/dsp/` contains five synthesis engines, all unified through the `Voice` enum in `kit.rs`:
- `fm.rs` — FM synthesis + noise sizzle layer
- `phys.rs` — Karplus-Strong physical modeling
- `granular.rs` — granular synthesis
- `hybrid.rs` — hybrid engine
- `noise.rs` — noise voice

Adding a new engine means: (1) implement the engine struct with `schema()`, `set_param`, `set_mod`, `tick`, `is_active`, etc.; (2) add a `Voice::Foo(FooEngine)` variant and wire all match arms in `kit.rs`; (3) handle the engine-type string in `KitEngine::from_config`.

`dsp/modulation.rs` defines `ModSource` (None/Envelope/Lfo1/Lfo2/Velocity) and `ModulatableParam { base_value, mod_slots }`. Engines that support modulation use `modulation_engine.rs` to compose source values into a final per-tick parameter value. The UI's "16 × 5" mod grid mirrors this exactly.

### Kit / mapping / state
- `KitEngine` holds a fixed `[Option<Voice>; 16]` array of voice slots and a `[Option<usize>; 128]` MIDI-note → slot map. Sounds are addressed by **slot index**, not by name.
- `DrumKit` / `DrumSound` / `DrumMapping` in `kit.rs` are the on-disk schema (TOML via serde). `DrumSound` carries `Option<f32>` for engine-specific params so a single struct serializes for all engines.
- `kit.toml` is the live kit and is rewritten on every parameter change. Named kits live in `presets/kits/*.toml`; named sound presets in `presets/sounds/*.toml`.

### WebSocket command protocol (`commands.rs`)
Messages are plain text with prefixes — not JSON envelopes. Examples:
- Client → server: `GET_KIT`, `GET_SCHEMA:3`, `SET_PARAM:slot|name|value`, `SET_MOD:slot|name|source|depth`, `SET_LFO:slot|index|freq`, `LIST_MIDI`, `SELECT_AUDIO:<name>`, `LOAD_KIT:<name>`, `SAVE_KIT_AS:<name>`, `TEST_TRIGGER:<note>`.
- Server → client: `KIT: <json>`, `SCHEMA:<slot>|<json>`, `MOD_STATES:<json>`, `BPM: 120.0`, `MIDI: <note>,<vel>`, `PORT: <name>`, `AUDIO_DEVICE: <name>`.

When adding a new command, edit both `commands.rs` (Rust handler) and `ui/src/App.tsx` (the `onmessage` parser dispatches by prefix).

### BPM / sync
`dsp/bpm_engine.rs` infers tempo from audio input (default input device) AND MIDI note onsets. `sync.rs` creates a **virtual MIDI output port** (`drummr Sync Out`) on Linux via `midir`'s `VirtualOutput` — this is Linux-specific (ALSA/JACK); macOS will log a warning and continue without sync output.

### Tests
- **Integration tests** live in `tests/*.rs` (each file is its own crate, uses public API from `lib.rs`). Many cover DSP engines end-to-end and audio stability.
- **Unit tests** are inline `#[cfg(test)] mod tests` blocks alongside the code (see `modulation.rs`, `modulation_engine_tests.rs`).
- Frontend tests use Vitest + Testing Library with `jsdom`; setup in `ui/src/test/setup.ts`.

## Project conventions

- The `conductor/` directory contains a self-imposed TDD workflow (`workflow.md`), product docs, and per-task tracking in `tracks/`. It expects: failing test first, implementation, `git notes` summary per task, checkpoint commits per phase. The user may or may not be following this strictly — match what they ask for.
- Commits follow Conventional Commits style (`feat(ui):`, `fix:`, `refactor:`).
- Rust edition is `2024` — keep that in mind when consulting external Rust references.
- The kit is a **live document**: the engine writes `kit.toml` on every UI parameter change. Don't hand-edit `kit.toml` while the engine is running unless you want it overwritten on the next slider move.
