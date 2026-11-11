# Hardhat
This is the new build system for Revelation because shell scripts
were getting too annoying. I want to change the current packaging
process and standards with this little side quest.

Firstly, right now we have container.sh building the Rust binaries
into ``/bld/target/`` then copying them into their respective homes.
I want to change this. The build directory structure for hardhat should
look like so:

```
/build
|__ /target - Rust binaries
|__ /ospkg - initramfs
|__ /fspkg - diskfs
```

This will allow us to cache binaries in order to speed up builds. Right now
we always remove them on a rebuild which makes it really really slow.

Hardhat should also build and package a coherent disk.img that we can
boot from as well. Right now we use QEMU as a "bootloader", but since
we have our own now we should hook everything up.
