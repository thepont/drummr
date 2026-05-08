# Specification: Track modulation_matrix_20260508

## Overview
This track introduces a comprehensive **Universal Modulation System** to drummr, allowing every synthesis parameter to be modulated by per-note envelopes, LFOs, and velocity. It also includes a critical bug fix for the **Laser Kick** UI visibility issue.

## Functional Requirements

### 1. Universal Modulation Engine (Backend)
- **Modulation Sources:**
    - **Per-Note AD Envelope:** Triggered on every hit.
    - **Global LFOs:** Continuous cyclic modulation.
    - **Velocity:** Linking hit strength to parameter depth.
- **Modulation Targets:** ALL parameters across all engines (FM, Phys, Granular, Hybrid) must be modulatable.
- **Modulation Matrix:** Implementation of a flexible routing system where parameters can have 1-2 "Mod Slots" assigned to different sources with dedicated depth controls.

### 2. Enhanced UI Controls (Frontend)
- **Dedicated Modulation Panel:** A detailed view for deep-diving into sound movement.
- **Visual Envelope Editor:** A draggable, interactive graph for shaping attack and decay.
- **Inline Mini-Graphs:** Real-time visual feedback of parameter movement directly in the editor view.
- **Per-Parameter Mod Slots:** Direct source selection and depth control integrated into every parameter slider.

### 3. Bug Fix: Laser Kick Visibility
- Investigate and resolve the issue where the Laser Kick's parameters are missing from the UI while other FM sounds function correctly.

## Non-Functional Requirements
- **Real-Time Safety:** Modulation calculations must be performed in the audio thread without allocations or blocking.
- **Low Latency:** The additional complexity must not push the processing time beyond the sub-5ms target.

## Acceptance Criteria
- [ ] Every parameter slider in the UI has at least one "Mod Slot" for assignment.
- [ ] Modulating the 'Metallic' parameter on a Physical modeling sound produces audible, decaying changes.
- [ ] Visual graphs accurately reflect the movement of parameters in real-time.
- [ ] The Laser Kick's parameters are fully visible and controllable in the Kit Editor.
- [ ] All automated tests for existing engines pass with modulation active.

## Out of Scope
- Multi-stage envelopes (ADSR).
- Cross-engine modulation (e.g., Slot 1 modulating Slot 2).
- User-definable LFO waveforms (start with basic Sine/Triangle).
