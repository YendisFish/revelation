qemu-system-x86_64 -kernel /boot/vmlinuz-6.8.0-147-generic -initrd initramfs.cpio -m 512 -nographic -append console=ttyS0 -drive file=hdd.img,format=raw,index=0,media=disk
