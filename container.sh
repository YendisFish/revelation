#!/bin/bash
# THIS LOGIC IS FOR THE CONTAINER, NOT THE HOST

OS="/build/ospkg"
FS="/build/fspkg"

EXTRACTED_MODS="/bld/kernel/extracted/lib/modules/6.6.142-0-virt"

# create directories inside of the ramfs
function create_ramfs {
    # create ram filesystem for initial mount
    mkdir -p $OS/{dev,sys,proc}
}

function create_diskfs {
    cd /build/
    mkdir -p $FS/{run,bin,apps,etc,lib}

    truncate -s 990M rootfs.ext4
    mkfs.ext4 -d $FS/ -F rootfs.ext4

    truncate -s 1G revelation.img

    sgdisk -o \
           -n 1:2048:0 \
           -c 1:"rootfs" \
           -u 1:12345678-ABCD-EF01-2345-6789ABCDEF01 \
           revelation.img

    dd if=rootfs.ext4 of=revelation.img bs=512 seek=2048 conv=notrunc status=none
    rm rootfs.ext4
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
cp /bld/target/release/genesis $OS/init
cp ./genesis.toml $OS/
cp ./boot.toml $OS/

cd /bld/phoebe
cargo build --release
cp /bld/target/release/phoebe $FS/bin/phoebe

create_bootable
