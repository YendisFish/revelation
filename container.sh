#!/bin/bash
# THIS LOGIC IS FOR THE CONTAINER, NOT THE HOST

OS="/build/ospkg"
FS="/build/fspkg"

EXTRACTED_MODS="/bld/kernel/extracted/lib/modules/6.6.142-0-virt"

# create directories inside of the ramfs
function create_ramfs {
    # create ram filesystem for initial mount
    mkdir -p $OS/lib
}

function create_diskfs {
    # create on disk fs
    cd /build/

    mkdir -p $FS/{dev,sys,sys/fs,sys/fs/cgroup/,proc,run,bin,apps,etc,lib}

    truncate -s 1G revelation.img
    mkfs.ext4 -F revelation.img

    e2cp -r -G 0 -O 0 /build/fspkg/* revelation.img:/
}

function create_bootable {
    # packages OS filesystem so it can be accessed by the kernel
    cd $OS
    find . -print0 | cpio --null --create --format=newc | gzip -9 > "/build/initramfs.cpio.gz"
}

function prepare_kernel_modules {
    # extract
    mkdir -p "$OS/lib/modules/"
    find $EXTRACTED_MODS -type f -name "*.ko.gz" -exec gzip -d {} +
    cp -r $EXTRACTED_MODS/* "$OS/lib/modules/"
}

# remove existing OS build
rm -rf /build/ospkg/
mkdir /build/ospkg/

rm -rf /build/fspkg/
mkdir /build/fspkg/

prepare_kernel_modules

create_ramfs
create_diskfs

# build genesis and move it into the OS filesystem
cd /bld/genesis
cargo build --release
cp ./target/release/genesis $OS/init
cp ./genesis.toml $OS/

cd /bld/phoebe
cargo build --release
cp ./target/release/phoebe $FS/bin/phoebe

create_bootable
