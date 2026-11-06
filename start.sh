#!/bin/bash

qemu-system-x86_64 \
    -kernel ./kernel/extracted/boot/vmlinuz-virt \
    -initrd ./build/initramfs.cpio.gz \
    -drive file=./build/revelation.img,format=raw,if=none,id=rootdisk \
    -device ide-hd,drive=rootdisk \
    -append "console=ttyS0 rdinit=/init" \
    -cpu max \
    -m 512M \
    -nographic
