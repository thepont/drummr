# Track Specification: Project Foundation & Research

## Overview
This track focuses on the foundational research and architectural setup for drummr. It involves investigating existing drum synthesis technologies, benchmarking performance on target hardware (where possible), and establishing the core communication and engine structure.

## Functional Requirements
- **State-of-the-Art Report:** Document findings on existing Rust audio engines, MIDI libraries, and synthesis techniques.
- **Synthesis Research:** Analyze physical modeling and FM synthesis models for percussive sound generation.
- **Audio Engine:** Initialize `cpal` to handle audio output with minimal latency.
- **MIDI Input:** Implement `wmidi` to parse incoming MIDI messages.
- **WebSocket Bridge:** Set up a `tokio-tungstenite` server for backend-frontend communication.
- **React UI:** A basic interface to visualize research findings and real-time MIDI activity.

## Technical Constraints
- **Evidence-Based Design:** Implementation choices must be backed by the findings in the research phase.
- **Low Latency:** Maintain the core focus on real-time performance.
- **ARM Compatibility:** Ensure all research and initial code consider Raspberry Pi/ARM limitations.
