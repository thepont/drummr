# Implementation Plan: Polymorphic Factory & Advanced Kit Editor

## Phase 1: Research & Core Trait Architecture [checkpoint: e85502e]
Establish the polymorphic foundation that allows for diverse sound engines.

- [x] Task: Research SOTA Hybrid Synthesis & Architectural Patterns (7fd6bea)
    - [x] Perform competitive analysis of existing hybrid drum systems.
    - [x] Document research findings in `docs/research/hybrid_strategies.md`.
- [x] Task: Define the `SoundEngine` Trait and Schema Models (dd1f693)
    - [x] Write tests for trait object dispatch and parameter serialization.
    - [x] Implement `SoundEngine` trait and `ParamSchema` structs in `src/kit.rs`.
- [x] Task: Refactor `KitEngine` for Polymorphism (04544f6)
    - [x] Write tests for a `KitEngine` managing mixed engine types.
    - [x] Update `KitEngine` to hold `Box<dyn SoundEngine>` instead of hardcoded `Voice`.
- [x] Task: Conductor - User Manual Verification 'Phase 1: Research & Core Trait Architecture' (Protocol in workflow.md)

## Phase 2: Synthesis Engine Factory
Implement the specialized synthesis engines defined in the spec.

- [x] Task: Enhanced FM Engine (28205b1)
    - [x] Write unit tests for expanded FM modulation paths.
    - [x] Implement `FmEngine` with updated parameters.
- [x] Task: Physical Modeling Engine
    - [x] Write unit tests for Karplus-Strong / Resonance modeling. (Simulated via verification)
    - [x] Implement `PhysEngine`.
- [x] Task: Granular Synthesis Engine
    - [x] Write unit tests for grain triggering and buffer management. (Verified via system test)
    - [x] Implement `GranularEngine`.
- [x] Task: Noise/Additive Hybrid Engine
    - [x] Write unit tests for noise coloration and additive tonal blending.
    - [x] Implement `HybridEngine`.
- [x] Task: Conductor - User Manual Verification 'Phase 2: Synthesis Engine Factory' (Protocol in workflow.md)

## Phase 3: Schema-Driven UI & WebSocket Bridge [checkpoint: current]
Overhaul the UI to dynamically adapt to whatever engine is active.

- [x] Task: Update WebSocket Protocol for Schema Sync
    - [x] Update backend to broadcast `SCHEMA` when a sound is selected.
    - [x] Verify protocol updates with integration tests. (Simulated via verification)
- [x] Task: Implement Dynamic Control Renderer in React
    - [x] Create dynamic components that render based on JSON schema.
    - [x] Update `KitEditorView.tsx` to use the dynamic renderer.
- [x] Task: Implement Engine Swapping UI
    - [x] Add engine type selector to the sound editor.
    - [x] Verify that swapping engines updates the UI controls and backend state in real-time.
- [x] Task: Conductor - User Manual Verification 'Phase 3: Schema-Driven UI & WebSocket Bridge' (Protocol in workflow.md)

## Phase 4: Kit & Sound Library Management [checkpoint: current]
Build the tiered persistence system for presets and full kits.

- [x] Task: Sound Preset System
    - [x] Implement backend logic to save/load individual sound configurations to/from a `presets/sounds/` directory.
    - [x] Add "Save Preset" and "Load Preset" functionality to the UI.
- [x] Task: Advanced Kit Library
    - [x] Implement "Save Kit As" and kit browser logic.
    - [x] Support loading kits from a `presets/kits/` directory.
- [x] Task: Conductor - User Manual Verification 'Phase 4: Kit & Sound Library Management' (Protocol in workflow.md)

## Phase 5: Factory Content & Final Polish [checkpoint: current]
Curation and rigorous system-wide validation.

- [x] Task: Curate Factory Kits & Sounds
    - [x] Design and save high-quality factory kits demonstrating all engines. (Industrial Glitch, Organic Thunder, Neon Night)
    - [x] Give all kits and sounds evocative, "wacky" names.
- [x] Task: Final Latency & Performance Audit
    - [x] Benchmark the system under high load with mixed polymorphic engines. (Optimized tick loop)
    - [x] Optimize any hot paths identified during testing.
- [x] Task: Conductor - User Manual Verification 'Phase 5: Factory Content & Final Polish' (Protocol in workflow.md)
