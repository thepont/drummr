#!/bin/bash
set -e

# 1. Ensure artifacts are built
echo "--- Ensuring ARM64 artifacts are built ---"
./distro/scripts/build_artifacts.sh

# 2. Build the Test Environment (Virtual Pi)
echo "--- Building Virtual Pi Container ---"
# Note: This requires qemu-user-static installed on the host
docker build -t drummr-test-env:latest -f distro/build-env/Dockerfile.test-env .

# 3. Run the Virtual Pi
echo "--- Starting Virtual Pi ---"
echo "Note: The engine will look for MIDI and Audio devices."
echo "Since this is a virtual environment, it might report 'No device found',"
echo "but we can verify the software boot sequence."
echo "------------------------------------------------"

docker run --rm -it \
    --name drummr-vm \
    drummr-test-env:latest
