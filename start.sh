#!/bin/bash

qemu-system-x86_64 \
    -kernel ./kernel/extracted/boot/vmlinuz-virt \
    -initrd ./build/initramfs.cpio.gz \
    -append "console=ttyS0 rdinit=/init" \
    -cpu max \
    -m 512M \
    -nographic
