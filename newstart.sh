#!/bin/bash

# Boot Revelation OS using the assembled disk image containing the 'beginning' bootloader.
# No -kernel, -initrd, or -append parameters are passed to QEMU because the custom
# bootloader (beginning) loads the kernel (vmlinuz) and initramfs (genesis) directly from disk.

qemu-system-x86_64 \
    -drive file=./build/revelation.img,format=raw,if=none,id=rootdisk \
    -device ide-hd,drive=rootdisk,bus=ide.0,unit=0 \
    -serial stdio \
    -m 1G \
    -cpu max \
    "$@"
