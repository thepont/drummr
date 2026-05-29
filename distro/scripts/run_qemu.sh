#!/bin/bash
# Boots the drummr OS in full QEMU emulation
# This emulates the entire Pi machine, including the kernel and systemd boot.

KERNEL_URL="https://github.com/dhruvvyas90/qemu-rpi-kernel/raw/master/kernel-qemu-5.10.63-bullseye"
DTB_URL="https://github.com/dhruvvyas90/qemu-rpi-kernel/raw/master/versatile-pb-bullseye-5.10.63.dtb"

mkdir -p distro/qemu
cd distro/qemu

# 1. Download QEMU-compatible kernel if missing
if [ ! -f "kernel-qemu" ]; then
    echo "--- Downloading QEMU Kernel ---"
    curl -L $KERNEL_URL -o kernel-qemu
fi

if [ ! -f "versatile.dtb" ]; then
    echo "--- Downloading DTB ---"
    curl -L $DTB_URL -o versatile.dtb
fi

# 2. Assemble a temporary rootfs image for QEMU
# (In a real scenario, we'd use a .img file, but for rapid dev we use 
# the Docker container we already built as our source)
echo "--- Preparing Virtual Disk ---"
docker run --rm -v "$(pwd)":/output drummr-test-env:latest \
    sh -c "tar -cvzf /output/rootfs.tar.gz / --exclude=/proc --exclude=/sys --exclude=/dev"

# 3. Boot QEMU
echo "--- Launching QEMU ---"
echo "To pass a USB MIDI device: add '-device usb-host,vendorid=0xXXXX,productid=0xXXXX'"
echo "------------------------------------------------"

qemu-system-aarch64 \
    -M virt \
    -cpu cortex-a53 \
    -m 1G \
    -kernel kernel-qemu \
    -dtb versatile.dtb \
    -append "root=/dev/vda rw console=ttyAMA0 rootwait" \
    -drive file=rootfs.tar.gz,if=none,id=vda,format=raw \
    -device virtio-blk-device,drive=vda \
    -netdev user,id=net0,hostfwd=tcp::8080-:8080,hostfwd=tcp::80-:80 \
    -device virtio-net-device,netdev=net0 \
    -nographic
