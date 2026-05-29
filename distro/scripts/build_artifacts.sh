#!/bin/bash
set -e

# 1. Build the cross-compilation environment
echo "--- Building Cross-Compilation Container ---"
docker build -t drummr-cross:latest -f distro/build-env/Dockerfile.cross distro/build-env/

# 2. Compile the backend for ARM64
echo "--- Compiling drummr for aarch64 ---"
docker run --rm -v "$(pwd)":/build drummr-cross:latest \
    cargo build --release --target aarch64-unknown-linux-gnu

# 3. TODO: Assemble the OS Image
# For now, we collect the artifacts into a deployable folder
mkdir -p distro/output/opt/drummr
cp target/aarch64-unknown-linux-gnu/release/drummr distro/output/opt/drummr/
cp -r presets distro/output/opt/drummr/
cp kit.toml distro/output/opt/drummr/
cp mapping.toml distro/output/opt/drummr/

echo "--- Artifacts ready in distro/output ---"
echo "Binary for Pi: distro/output/opt/drummr/drummr"
