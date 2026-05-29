#!/bin/bash
# Fast Test: Run the drummr engine with REAL MIDI access from your host machine
# This uses the virtual Pi environment but maps your host's sound devices.

set -e

echo "--- Building artifacts ---"
./distro/scripts/build_artifacts.sh

echo "--- Building test environment ---"
docker build -t drummr-test-env:latest -f distro/build-env/Dockerfile.test-env .

echo "--- Starting drummr with Host MIDI Access ---"
echo "Searching for MIDI devices on host..."
amidi -l || echo "No MIDI devices found on host."

# We map /dev/snd to give the container access to your MIDI/Audio hardware
# We use --privileged to allow real-time priority (RT_PRIO) tuning
docker run --rm -it \
    --name drummr-hw-test \
    --privileged \
    --net=host \
    -v /dev/snd:/dev/snd \
    drummr-test-env:latest
