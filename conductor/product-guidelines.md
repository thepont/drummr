# Product Guidelines: drummr

## Tone & Voice
- **Technical & Precise:** Documentation and labels should be technically accurate, reflecting the high-performance Rust architecture and low-latency goals.
- **Creative & Inspiring:** While technical, the language should inspire musical exploration, focusing on the expressive potential of percussive sound design.
- **Utility-Focused & Direct:** Instructions must be concise and actionable, designed for fast operation by performers in high-pressure environments.

## Visual Identity & UI/UX
- **Decoupled HTML Frontend:** The UI is a standalone HTML/JavaScript application, completely decoupled from the Rust engine.
- **WebSocket Communication:** Uses WebSockets for low-latency, real-time bidirectional communication between the UI and the backend.
- **High-Visibility Aesthetic:** Designed for dark stages with high-contrast elements that are easy to read at a glance.
- **Real-Time Visual Feedback:** Provide immediate visual representation of MIDI triggers, velocity dynamics, and rhythmic timing.

## Interaction & Control
- **NOT A SEQUENCER:** The system is a real-time sound generator for performers. It focuses on the relationship between a trigger (e.g., hitting a pad) and the resulting sound.
- **Deep MIDI Mapping:** Every sound parameter and system control must be assignable to external MIDI CC or other control messages for tactile performance.
- **Dynamic Sound Synthesis:** Focus on algorithms that define *how* a drum sounds (synthesis, distortion, resonance) rather than *when* it plays.

## Configuration & Data
- **Human-Readable Formats:** Use TOML or YAML for all configuration (kits, mappings, global settings) to allow for easy version control and manual adjustment.
- **Integrated Visual Editors:** The HTML UI should provide intuitive tools for managing kits and tweaking synthesis parameters without requiring manual file edits.
- **Hot-Reloading:** The system must support live-reloading of configuration files, allowing for seamless sound adjustments without interrupting the audio engine.

## Research & Development Focus
- **Drum-Centric Synthesis:** Research and implement synthesis techniques specifically optimized for percussive sounds, including specialized distortions and resonance models.
- **Preset Excellence:** Curate a wide range of high-quality preset kits that demonstrate the system's expressive range across different percussive styles.
