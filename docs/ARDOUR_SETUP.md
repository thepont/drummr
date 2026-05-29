# Ardour & PipeWire Integration Guide

This document describes the routing setup used to integrate `drummr` with **Ardour** on a Linux system using **PipeWire**.

## Architecture Overview

`drummr` acts as a standalone audio engine. Instead of living inside Ardour as a plugin (VST/CLAP), it runs as a separate process and streams its audio to Ardour via PipeWire's virtual patchbay.

### Signal Chain
`drummr (libcpal)` -> `PipeWire (alsa_playback.drummr)` -> `Ardour (Audio_DRUMMR)`

## Dedicated Routing

To avoid interference with other system audio (like microphones or system alerts), `drummr` is routed to a dedicated stereo track in Ardour.

### Ardour Configuration
1. **Create a Stereo Track**: Named `Audio_DRUMMR`.
2. **Input Assignment**: The track is set to receive from its own dedicated input ports.

### PipeWire Routing (Manual/CLI)
The connections are established using the `pw-link` tool. 

**Connect drummr to Ardour:**
```bash
# Connect Left Channel
pw-link alsa_playback.drummr:output_FL "ardour:Audio_DRUMMR/audio_in 1"

# Connect Right Channel
pw-link alsa_playback.drummr:output_FR "ardour:Audio_DRUMMR/audio_in 2"
```

**Verify Connections:**
```bash
pw-link -l | grep "drummr"
```

## Troubleshooting & Maintenance

### Audio Underruns (Crackling/Random Noise)
If you hear crackling, the engine is likely struggling with a buffer size that is too small for the current CPU load. 
- **Current Buffer**: 128 samples (optimized for < 5ms latency).
- **Location**: `src/audio.rs`.
- **Note**: Increasing the buffer to 256 or 512 can improve stability but will increase latency.

### Restoring Routing after Restart
If `drummr` or Ardour is restarted, the virtual connections may be lost. You can re-run the `pw-link` commands above to restore the link. For a permanent solution, tools like **qpwgraph** or **Helvum** can be used to save and auto-restore these "patchbay" states.

### Conflict with Microphone
By routing to a dedicated track (`Audio_DRUMMR`) instead of the default `Audio 1`, we ensure that `drummr` does not overwrite or mix with your physical microphone inputs (usually found on `Audio 1/2` or `capture_AUX` ports).
