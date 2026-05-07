# Specification: Polymorphic Factory & Advanced Kit Editor

## 1. Overview
This track transforms "drummr" from a single-engine FM synthesizer into a polymorphic sound station. We will introduce a factory-based architecture that supports multiple synthesis engines (FM, Physical Modeling, Granular, etc.), each with its own parameter schema. The Kit Editor will be overhauled to dynamically render controls for these diverse engines and provide a multi-tiered preset system for both individual sounds and entire kits.

## 2. Functional Requirements

### 2.1 Polymorphic Sound Engine (Backend)
- **Trait-Based Architecture:** Implement a `SoundEngine` trait in Rust to abstract synthesis, parameter handling, and triggering.
- **Dynamic Parameter Schema:** Each engine must provide a schema (name, min/max, default, unit) so the UI can render controls dynamically.
- **Initial Engine Menu:**
    - **FM (Enhanced):** Expanded FM synthesis with additional modulation paths.
    - **Physical Modeling:** String/Membrane simulation for realistic and experimental resonances.
    - **Granular Synthesis:** For textures, "coral harmony," and non-traditional percussion.
    - **Noise/Additive Hybrid:** Specialized for snares and cymbals.

### 2.2 Schema-Driven Kit Editor (Frontend)
- **Dynamic UI Rendering:** The Kit Editor will no longer have a hardcoded layout. It will render sliders and toggles based on the active engine's schema.
- **Live Parameter Sync:** Maintain real-time WebSocket communication for every parameter change.
- **Engine Swapping:** Allow users to change a drum's engine type on the fly.

### 2.3 Tiered Preset & Kit Management
- **Kit Library:** Save, load, and "Save As" entire drum kits (mappings + sound parameters).
- **Sound Library:** Save and load individual sound presets (e.g., a "Granular Coral" snare) into any kit slot.
- **Factory Content:** A collection of high-quality, creatively named kits and sounds to showcase the system's "wacky" and "hybrid" potential.

### 2.4 Research & Content
- **Hybrid Drum Research:** Investigate SOTA (State of the Art) techniques for hybrid synthesis to inform engine implementation.
- **Curation:** Design and name a suite of factory kits that demonstrate the new architectural capabilities.

## 3. Non-Functional Requirements
- **Extensibility:** Adding a new engine should require zero changes to the core `KitEngine` or the UI's layout logic.
- **Performance:** Ensure that polymorphic dispatch and dynamic parameters do not compromise the <5ms latency target.

##  acceptance Criteria
- Physical MIDI triggers correctly regardless of which engine is assigned to a note.
- UI correctly renders different sets of sliders when switching between an FM drum and a Granular drum.
- Users can save an individual sound and successfully load it into a different kit.
- The "Factory" kits are available on startup and correctly named/configured.

## 5. Out of Scope
- Direct sample recording/sampling within the UI (one-shot loading only if/when Sample engine is added).
- Full DAW-style mixer with per-channel VST effects (focus remains on synthesis).
