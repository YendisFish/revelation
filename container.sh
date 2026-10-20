#!/bin/bash
# THIS LOGIC IS FOR THE CONTAINER, NOT THE HOST

OS="/build/ospkg"

# create directories inside of the ramfs
function create_ramfs {
    # create ram filesystem for initial mount
    mkdir -p $OS/{dev,sys,proc,run,bin,apps}
}

function create_diskfs {
    # create on disk fs
    echo ""
}

function create_bootable {
    # packages OS filesystem so it can be accessed by the kernel
    cd $OS
    find . -print0 | cpio --null --create --format=newc | gzip -9 > "/build/initramfs.cpio.gz"
}

# remove existing OS build
rm -rf /build/ospkg/
mkdir /build/ospkg

# build genesis and move it into the OS filesystem
cd /bld/genesis
cargo build --release
cp ./target/release/genesis $OS/init
cp ./genesis.toml $OS/

create_ramfs
create_bootable
