# Implementation Plan: Polymorphic Factory & Advanced Kit Editor

## Phase 1: Research & Core Trait Architecture
Establish the polymorphic foundation that allows for diverse sound engines.

- [x] Task: Research SOTA Hybrid Synthesis & Architectural Patterns (7fd6bea)
    - [x] Perform competitive analysis of existing hybrid drum systems.
    - [x] Document research findings in `docs/research/hybrid_strategies.md`.
- [x] Task: Define the `SoundEngine` Trait and Schema Models (dd1f693)
    - [x] Write tests for trait object dispatch and parameter serialization.
    - [x] Implement `SoundEngine` trait and `ParamSchema` structs in `src/kit.rs`.
- [~] Task: Refactor `KitEngine` for Polymorphism
    - [ ] Write tests for a `KitEngine` managing mixed engine types.
    - [ ] Update `KitEngine` to hold `Box<dyn SoundEngine>` instead of hardcoded `Voice`.
- [ ] Task: Conductor - User Manual Verification 'Phase 1: Research & Core Trait Architecture' (Protocol in workflow.md)

## Phase 2: Synthesis Engine Factory
Implement the specialized synthesis engines defined in the spec.

- [ ] Task: Enhanced FM Engine
    - [ ] Write unit tests for expanded FM modulation paths.
    - [ ] Implement `FmEngine` with updated parameters.
- [ ] Task: Physical Modeling Engine
    - [ ] Write unit tests for Karplus-Strong / Resonance modeling.
    - [ ] Implement `PhysicalModelingEngine`.
- [ ] Task: Granular Synthesis Engine
    - [ ] Write unit tests for grain triggering and buffer management.
    - [ ] Implement `GranularEngine`.
- [ ] Task: Noise/Additive Hybrid Engine
    - [ ] Write unit tests for noise coloration and additive tonal blending.
    - [ ] Implement `HybridEngine`.
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Synthesis Engine Factory' (Protocol in workflow.md)

## Phase 3: Schema-Driven UI & WebSocket Bridge
Overhaul the UI to dynamically adapt to whatever engine is active.

- [ ] Task: Update WebSocket Protocol for Schema Sync
    - [ ] Update backend to broadcast `ENGINE_SCHEMA` when a sound is selected.
    - [ ] Verify protocol updates with integration tests.
- [ ] Task: Implement Dynamic Control Renderer in React
    - [ ] Create `DynamicControl` components that render based on JSON schema.
    - [ ] Update `KitEditorView.tsx` to use the dynamic renderer.
- [ ] Task: Implement Engine Swapping UI
    - [ ] Add engine type selector to the sound editor.
    - [ ] Verify that swapping engines updates the UI controls and backend state in real-time.
- [ ] Task: Conductor - User Manual Verification 'Phase 3: Schema-Driven UI & WebSocket Bridge' (Protocol in workflow.md)

## Phase 4: Kit & Sound Library Management
Build the tiered persistence system for presets and full kits.

- [ ] Task: Sound Preset System
    - [ ] Implement backend logic to save/load individual sound configurations to/from a `presets/sounds/` directory.
    - [ ] Add "Save Preset" and "Load Preset" functionality to the UI.
- [ ] Task: Advanced Kit Library
    - [ ] Implement "Save Kit As" and kit browser logic.
    - [ ] Support loading kits from a `presets/kits/` directory.
- [ ] Task: Conductor - User Manual Verification 'Phase 4: Kit & Sound Library Management' (Protocol in workflow.md)

## Phase 5: Factory Content & Final Polish
Curation and rigorous system-wide validation.

- [ ] Task: Curate Factory Kits & Sounds
    - [ ] Design and save high-quality factory kits demonstrating all engines.
    - [ ] Give all kits and sounds evocative, "wacky" names.
- [ ] Task: Final Latency & Performance Audit
    - [ ] Benchmark the system under high load with mixed polymorphic engines.
    - [ ] Optimize any hot paths identified during testing.
- [ ] Task: Conductor - User Manual Verification 'Phase 5: Factory Content & Final Polish' (Protocol in workflow.md)
