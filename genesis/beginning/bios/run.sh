#qemu-system-x86_64 -drive file=bootmenu.bin,format=raw,index=0,media=disk
qemu-system-x86_64 -drive file=hdd.img,format=raw,index=0,media=disk -serial stdio -m 2G