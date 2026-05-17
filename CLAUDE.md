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

`SharedState` (`state.rs`) holds an `[AtomicU32; 16*5]` of modulation source values — 16 slots × 5 source ids (None / Envelope / Lfo1 / Lfo2 / Velocity). The audio thread stores `f32::to_bits` per tick via `set_value`; the WS broadcast loop reads them out for the UI's real-time visualizers. Note that `Voice::get_mod_values` only returns the 4 active sources (`[f32; 4]`) — `Voice::Noise` returns zeros. This is the only inter-thread channel for "live values."

### DSP layout
`src/dsp/` contains six synthesis engines, all unified through the `Voice` enum in `kit.rs`:
- `fm.rs` — FM synthesis + noise sizzle layer
- `phys.rs` — Karplus-Strong physical modeling
- `granular.rs` — granular synthesis
- `hybrid.rs` — oscillator + noise blend with metallic inharmonic partials
- `modal.rs` — parallel resonator bank (Bessel-zero mode ratios) for bells, plates, tuned percussion
- `noise.rs` — coloured-noise voice

Adding a new engine means: (1) implement the engine struct with `schema()`, `set_param`, `set_mod`, `tick`, `is_active`, etc.; (2) add a `Voice::Foo(FooEngine)` variant and wire all match arms in `kit.rs`; (3) handle the engine-type string in `KitEngine::from_config`; (4) wire the engine pill into `ui/src/views/KitEditorView.tsx`.

`dsp/modulation.rs` defines `ModSource` (None/Envelope/Lfo1/Lfo2/Velocity) and `ModulatableParam { base_value, mod_slots }`. Engines that support modulation use `modulation_engine.rs` to compose source values into a final per-tick parameter value. Per-voice post-FX (`bits`, `rate`) live in `dsp/postfx.rs` and run after the voice mix.

### Kit / mapping / state
- `KitEngine` holds a fixed `[Option<Voice>; 16]` array of voice slots and a `[Option<usize>; 128]` MIDI-note → slot map. Sounds are addressed by **slot index**, not by name.
- `DrumKit` / `DrumSound` / `DrumMapping` in `kit.rs` are the on-disk schema (TOML via serde). `DrumSound` carries `Option<f32>` for engine-specific params so a single struct serializes for all engines.
- **`SharedState::kit_snapshot: Arc<Mutex<DrumKit>>` is the source of truth for kit mutations.** All WS command handlers in `commands.rs` mutate the snapshot first and then push the resulting `DrumKit` to the persistence worker (which writes `kit.toml` via atomic rename). The audio thread reads its own `KitEngine` via `try_lock` on `SharedState::kit`; mapping changes use `KitEngine::set_mapping` in place to preserve voice state.
- Named kits live in `presets/kits/*.toml`; named sound presets in `presets/sounds/*.toml`. `kit.toml` at the repo root is the live working kit and is rewritten on every parameter change.

### WebSocket command protocol (`commands.rs`)
Messages are plain text with prefixes — not JSON envelopes. The full list lives in `handle_command` (`src/commands.rs`); the UI dispatcher lives in `ui/src/App.tsx::onmessage`. The two must stay in sync — when adding a new command, edit both sides.

Client → server (selected):
- **Kit / param edits**: `GET_KIT`, `GET_SCHEMA:<slot>`, `SET_PARAM:slot|name|value`, `SET_MOD:slot|name|source|depth`, `SET_LFO:slot|index|freq`, `SET_BITS:slot|val`, `SET_RATE:slot|val`.
- **Clock-aware fields**: `SET_DIVISION:slot|param|division` (param is `lfo1` / `lfo2` / `decay`; division is a `BeatDivision` variant name like `Quarter` or `Bar`), `CLEAR_DIVISION:slot|param`. Note: the generative-trigger fields (`trigger_probability`, `ghost_probability`, `ghost_offset_ms`, `ghost_velocity_factor`) are sent through plain `SET_PARAM:` — `commands.rs` routes them to `AudioCommand::SetGenerative` internally.
- **Presets**: `LIST_KITS`, `LOAD_KIT:<name>`, `SAVE_KIT_AS:<name>`, `LIST_SOUND_PRESETS`, `SAVE_SOUND_PRESET:<name>:<slot>`, `LOAD_SOUND_PRESET:<name>:<slot>`.
- **Mapping**: `GET_MAPPING`, `UPDATE_MAPPING:slot:note`, `SAVE_MAPPING:<json>`.
- **Devices / discovery**: `LIST_MIDI`, `LIST_AUDIO`, `SELECT_MIDI:<index>`, `SELECT_AUDIO:<index>`.
- **Diagnostics**: `ANALYZE_SLOT:<slot>` (off-thread peak/RMS/clipping/silent measurement), `TEST_TRIGGER:<slot>`.
- **Sync**: `SYNC_START`, `SYNC_STOP`, `SET_AUTO_SYNC:<bool>`, `GET_SYNC_STATUS`.
- **Preview Kit (MIDI playback)**: `LIST_MIDI_TRACKS`, `PLAY_MIDI_TRACK:<name>`, `STOP_MIDI_PLAYBACK`.

Server → client (selected):
- **Kit / schema**: `KIT: <json>` (includes all clock-aware fields — `sub_hits`, `pattern`, `mode_list`, the ghost-* and `trigger_probability`, the three `*_division` fields), `SCHEMA:<slot>|<json>`, `KIT_LIST:<csv>`, `SOUND_PRESETS:<csv>`, `KIT_ERROR:<name>:<phase>:<detail>` (emitted on `LOAD_KIT` failure).
- **Realtime state**: `MOD_STATES:<json>` (40 ms broadcast loop), `BPM: <f32>` (100 ms loop), `MIDI: <note>,<vel>`.
- **Devices**: `LIST_MIDI: <csv>`, `LIST_AUDIO: <csv>`, `PORT: <name>`, `AUDIO_DEVICE: <name>`.
- **Diagnostics**: `ANALYSIS:<slot>|<json>` (peak, rms, clipped_samples, sustained_clip, silent, engine, decay_ms), `AUDIO_LEAKS:<count>` (cpal::Stream leak count from device hot-swaps).
- **Preview Kit**: `MIDI_TRACKS:<csv>`, `MIDI_TRACK_PLAYING:<name>`, `MIDI_TRACK_STOPPED:<name>`, `MIDI_TRACK_ERROR:<name>`.
- **Sync / mapping**: `SYNC_STATUS:<Running|Stopped>`, `MAPPING: <json>`.

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
- The kit is a **live document**: the engine writes `kit.toml` on every UI parameter change (via the persistence worker, atomic rename). Don't hand-edit `kit.toml` while the engine is running — your edits will be overwritten on the next mutation from the UI.
