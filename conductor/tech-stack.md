# Tech Stack: drummr

## Core Audio Engine (Rust)
- **Audio I/O:** `cpal` - Low-level, cross-platform audio input/output for ultra-low latency.
- **MIDI Processing:** `wmidi` - Zero-copy, high-efficiency MIDI parsing and generation.
- **Asynchronous Runtime:** `tokio` - For handling concurrent tasks like WebSocket communication and configuration reloading.
- **Communication:** `tokio-tungstenite` - Robust WebSocket implementation for real-time interaction with the frontend.

## Frontend UI (HTML/JavaScript)
- **Framework:** **React** - For building a modular and interactive user interface.
- **Language:** **TypeScript** - To ensure type safety and improve maintainability in the frontend.
- **Communication:** Standard Web Browser WebSockets API for low-latency feedback from the Rust engine.

## Configuration & Data
- **Format:** **TOML** - Human-readable and specifically designed for configuration files.
- **Serialization:** `serde` - The standard Rust framework for serializing and deserializing data structures.

## Target Platforms
- **Primary:** Linux (specifically optimized for performance and low-latency).
- **Embedded:** ARM (e.g., Raspberry Pi) with support for headless operation.
- **Future:** DAW Integration (e.g., VST3/CLAP) using libraries like `nih-plug`.

## Research & Analysis Tools
- **Web Investigation:** `google_web_search` and `web_fetch` for analyzing existing systems and documentation.
- **Synthesis Research:** Academic whitepapers on physical modeling and percussive synthesis techniques.
- **Open Source Benchmarking:** Source code analysis of established projects like Hydrogen and Surge XT to identify best practices.
