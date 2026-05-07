# Specification: UI Overhaul & Kit Editor

## Overview
Transform the current "green-on-black" interface into a modern, professional dark UI that is responsive across PC, tablet, and mobile devices. This track introduces a comprehensive Kit Editor and MIDI Mapping system, allowing users to customize their sounds and hardware assignments directly from the web interface.

## Functional Requirements
- **Modern Dark UI:** Replace existing styles with a professional dark theme using Shadcn UI, Radix UI, and Phosphor Icons.
- **Responsive Design:** Ensure the interface is optimized for desktop, tablet, and phone screen sizes.
- **MIDI Mapping Screen:**
    - **Visual Feedback:** A grid of pads that illuminate when receiving MIDI notes.
    - **MIDI Learn:** Ability to select a drum role and hit a physical pad to map it.
    - **Manual Mapping:** A searchable list of all active MIDI notes with assignment dropdowns.
- **Kit Editor Screen:**
    - **Parameter Control:** Real-time sliders for FM synthesis (Freq, Mod Ratio, Mod Index).
    - **Envelope Editor:** A visual, interactive AD envelope display with draggable points.
    - **Sound Gallery:** Browse and load presets (e.g., "Deep 808", "Crisp Snare").
    - **Test Trigger:** A UI button to preview sounds immediately.
- **Persistence:**
    - **Manual Save:** Users must explicitly click a "Save Kit" button to persist changes to `kit.toml`.
    - **Settings Management:** Save and load both kit sounds and MIDI mappings.

## Non-Functional Requirements
- **Latency:** UI interactions must not interfere with the low-latency audio engine.
- **Performance:** Smooth rendering of visual envelopes and real-time parameter updates.

## Acceptance Criteria
- [ ] UI is fully responsive and uses the new dark theme and Phosphor icons.
- [ ] Users can map any MIDI note to a drum role via "MIDI Learn" or a list.
- [ ] Users can edit FM parameters and envelopes visually with immediate audio feedback.
- [ ] Changes can be saved to and loaded from the backend.
- [ ] Visual pads react accurately to physical MIDI input.

## Out of Scope
- Adding new synthesis types (e.g., Modal) beyond FM and Noise in this specific track.
- Multi-user authentication or cloud saving.
