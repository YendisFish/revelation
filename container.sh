#!/bin/bash
# THIS LOGIC IS FOR THE CONTAINER, NOT THE HOST

OS="/build/ospkg"

function create_ramfs {
    # create ram filesystem for initial mount
    mkdir -p $OS/{dev,sys,proc,run,bin}
}

function create_diskfs {
    # create on disk fs
    echo ""
}

function create_bootable {
    cd $OS
    find . -print0 | cpio --null --create --format=newc | gzip -9 > "/build/initramfs.cpio.gz"
}

rm -rf /build/ospkg/
mkdir /build/ospkg

cd /bld/genesis
cargo build --release
cp ./target/release/genesis $OS/init

create_ramfs
create_bootable
