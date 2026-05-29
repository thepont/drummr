# drummr-os Engineering

This directory contains the build system for the dedicated, minimalistic drummr OS image for Raspberry Pi.

## Architecture
- **Cross-Compilation:** We use a Docker-based Debian environment (`distro/build-env/Dockerfile.cross`) to compile the Rust engine for the `aarch64` (Pi 3/4/5) architecture without needing a Pi.
- **Service Management:** A custom systemd service (`distro/services/drummr.service`) handles:
    - Auto-starting on boot.
    - Setting the CPU governor to `performance` mode.
    - Elevating process priority to Real-Time (`RR` policy, priority 95).
- **Minimalism:** The goal is a < 200MB image that boots in under 5 seconds directly into the engine.

## Usage
1. **Compile Artifacts:** Run `./distro/scripts/build_artifacts.sh`. This produces the ARM binary and required assets in `distro/output`.
2. **Image Creation (WIP):** Future scripts will use `debootstrap` or `pi-gen` to wrap these artifacts into a bootable `.img` file.
