# This was made by Opus 5.5

# Filesystem starts at 1MiB. Must match FS_START_SECTOR in ata_bdev.c
FS_START = 2048
VMLINUZ = /boot/vmlinuz-6.8.0-147-generic
INITRAMFS = initramfs.cpio

# Compilers and flags
CC = gcc
ASM = nasm
CFLAGS = -m32 -ffreestanding -O2 -nostdlib -Wall -Wextra \
         -I. \
         -I./lib/lwext4/include \
         -DCONFIG_USE_DEFAULT_CFG=1 \
         -fno-pic -fno-pie -fno-stack-protector -U_FORTIFY_SOURCE \
         -DCONFIG_DEBUG_PRINTF=0 -DCONFIG_DEBUG_ASSERT=0

# Source files
C_SOURCES = main.c ata.c ata_bdev.c libc.c
# Complete list of lwext4 source files based on your tree structure
LWEXT4_SOURCES = lib/lwext4/src/ext4.c \
                 lib/lwext4/src/ext4_balloc.c \
                 lib/lwext4/src/ext4_bcache.c \
                 lib/lwext4/src/ext4_bitmap.c \
                 lib/lwext4/src/ext4_block_group.c \
                 lib/lwext4/src/ext4_blockdev.c \
                 lib/lwext4/src/ext4_crc32.c \
                 lib/lwext4/src/ext4_debug.c \
                 lib/lwext4/src/ext4_dir.c \
                 lib/lwext4/src/ext4_dir_idx.c \
                 lib/lwext4/src/ext4_extent.c \
                 lib/lwext4/src/ext4_fs.c \
                 lib/lwext4/src/ext4_hash.c \
                 lib/lwext4/src/ext4_ialloc.c \
                 lib/lwext4/src/ext4_inode.c \
                 lib/lwext4/src/ext4_journal.c \
                 lib/lwext4/src/ext4_mbr.c \
                 lib/lwext4/src/ext4_mkfs.c \
                 lib/lwext4/src/ext4_super.c \
                 lib/lwext4/src/ext4_trans.c \
                 lib/lwext4/src/ext4_xattr.c

# Object files substitution
OBJ = $(C_SOURCES:.c=.o) $(LWEXT4_SOURCES:.c=.o)

all: hdd.img

# 1. Compile 16-bit Bootloader Stages
boot.bin: boot.asm
	$(ASM) -f bin boot.asm -o boot.bin

stage2.bin: stage2.asm kernel.bin
	$(ASM) -f bin -DKERNEL_SECTORS=$$(( ($$(stat -c %s kernel.bin) + 511) / 512 )) stage2.asm -o stage2.bin

# 2. Generic rule for compiling C files
%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

# 3. Link the C code and lwext4 objects using the linker script
kernel.bin: $(OBJ) linker.ld
	$(CC) -T linker.ld -m32 -nostdlib -ffreestanding -no-pie -Wl,--build-id=none -o kernel.bin $(OBJ) -lgcc

# 4. Assemble the final hard drive image
hdd.img: boot.bin stage2.bin kernel.bin rootfs.img
	test $$(cat boot.bin stage2.bin kernel.bin | wc -c) -le $$(($(FS_START) * 512))
	cat boot.bin stage2.bin kernel.bin > hdd.img
	# Pad up to the filesystem's start, append it, then pad to exactly 2000000 sectors
	# (approx 1GB) so lwext4 doesn't panic
	truncate -s $$(($(FS_START) * 512)) hdd.img
	cat rootfs.img >> hdd.img
	truncate -s $$((2000000 * 512)) hdd.img
	# Partition table so Linux sees the filesystem as /dev/sda1. Leaves the boot code alone.
	echo '$(FS_START),,83' | sfdisk -q hdd.img

# 5. Build the ext4 filesystem the boot menu reads. mke2fs -d copies the folder in.
#    Features lwext4 can't handle are turned off.
rootfs.img: $(VMLINUZ) $(INITRAMFS)
	rm -rf rootfs rootfs.img
	mkdir rootfs
	# /boot/vmlinuz-* is only readable by root on Ubuntu
	cat $(VMLINUZ) > rootfs/vmlinuz 2>/dev/null || sudo cat $(VMLINUZ) > rootfs/vmlinuz
	cp $(INITRAMFS) rootfs/initramfs.cpio
	# The real root: a static busybox provides /bin/sh, and init mounts over dev, proc and sys
	mkdir -p rootfs/bin rootfs/dev rootfs/proc rootfs/sys
	cp $$(command -v busybox) rootfs/bin/busybox
	ln -s busybox rootfs/bin/sh
	mke2fs -q -t ext4 -b 4096 -O ^has_journal,^metadata_csum,^metadata_csum_seed,^orphan_file,^64bit -d rootfs rootfs.img 256M
	rm -rf rootfs

clean:
	rm -f *.o *.bin *.img *.elf *.map *.d
	find lib/lwext4 -name "*.o" -delete

.PHONY: all clean
