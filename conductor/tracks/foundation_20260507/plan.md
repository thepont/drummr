# Implementation Plan: Project Foundation & Research

## Phase 0: Research & Discovery
- [ ] Task: Conduct State-of-the-Art Review.
    - [ ] Research low-latency Rust audio libraries (`cpal`, `jack`, `asiodrv`).
    - [ ] Investigate existing open-source drum synthesizers (Hydrogen, Surge XT) for synthesis logic.
    - [ ] Document findings in `docs/research/sota_review.md`.
- [ ] Task: Research Percussive Synthesis Techniques.
    - [ ] Analyze physical modeling and FM synthesis for kick, snare, and hi-hats.
    - [ ] Document synthesis strategies in `docs/research/synthesis_strategies.md`.
- [ ] Task: Hardware & Performance Benchmarking.
    - [ ] Research baseline performance for Rust audio on Linux/ARM environments.
- [ ] Task: Conductor - User Manual Verification 'Phase 0: Research & Discovery' (Protocol in workflow.md)

## Phase 1: Project Scaffolding & Core Engine
- [ ] Task: Initialize Rust project and React frontend.
    - [ ] Run `cargo init` for the backend.
    - [ ] Set up React/TypeScript frontend using Vite.
- [ ] Task: Implement basic Audio Output with `cpal`.
    - [ ] Create an audio stream and output a simple sine wave to verify low-latency playback.
- [ ] Task: Implement MIDI Input with `wmidi`.
    - [ ] Set up MIDI device discovery and message parsing.
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Project Scaffolding & Core Engine' (Protocol in workflow.md)

## Phase 2: Communication Bridge & Data Structures
- [ ] Task: Implement WebSocket server using `tokio-tungstenite`.
    - [ ] Create a bridge that broadcasts MIDI events and research-derived state to the UI.
- [ ] Task: Define Drum Kit TOML structure.
    - [ ] Implement `serde` models for kits and mappings based on research.
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Communication Bridge & Data Structures' (Protocol in workflow.md)

## Phase 3: UI Foundation & Integration
- [ ] Task: Build React UI for MIDI Visualization & Research Dashboard.
    - [ ] Implement a real-time "trigger" indicator and a display for active synthesis parameters.
- [ ] Task: Integrate UI with Rust WebSocket server.
    - [ ] Verify low-latency visual feedback for MIDI events.
- [ ] Task: Conductor - User Manual Verification 'Phase 3: UI Foundation & Integration' (Protocol in workflow.md)
