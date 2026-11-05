#include "ext4_errno.h"
#include "ext4_blockdev.h"
#include <stddef.h>
#include <stdint.h>

#define ATA_SECTOR_SIZE 512
#define ATA_SECTOR_COUNT 2000000 // TODO Total sector count (can be hardcoded large enough for QEMU, but needs to be dynamic)
#define FS_START_SECTOR 2048 // Must match FS_START in the Makefile.

// ata.c
extern int ata_lba_read(uint32_t lba, uint8_t sect_count, uint8_t* target_addr);

// lwext4 callbacks (defined below)
static int ata_bdev_open(struct ext4_blockdev *bdev);
static int ata_bdev_read(struct ext4_blockdev *bdev, void *buf, uint64_t blk_id, uint32_t blk_cnt);
static int ata_bdev_write(struct ext4_blockdev *bdev, const void *buf, uint64_t blk_id, uint32_t blk_cnt);
static int ata_bdev_close(struct ext4_blockdev *bdev);

// lwext4 needs a one-sector scratch buffer
static uint8_t ata_ph_bbuf[ATA_SECTOR_SIZE];

// required by lwext4
static struct ext4_blockdev_iface ata_iface = {
    .open = ata_bdev_open,
    .bread = ata_bdev_read,
    .bwrite = ata_bdev_write,
    .close = ata_bdev_close,
    .lock = NULL,
    .unlock = NULL,
    .ph_bsize = ATA_SECTOR_SIZE, // Physical block size is always 512 on standard ATA
    .ph_bcnt = ATA_SECTOR_COUNT,
    .ph_bbuf = ata_ph_bbuf,
};

// required by lwext4
struct ext4_blockdev ata_device = {
    .bdif = &ata_iface,
    .part_offset = FS_START_SECTOR*ATA_SECTOR_SIZE,
    .part_size = (uint64_t)(ATA_SECTOR_COUNT-FS_START_SECTOR)*ATA_SECTOR_SIZE,
};

// Getter for main.c
struct ext4_blockdev* get_ata_bdev() {
    return &ata_device;
}

// lwext4 callback
static int ata_bdev_read(struct ext4_blockdev *bdev, void *buf, uint64_t blk_id, uint32_t blk_cnt) {
    uint32_t lba = (uint32_t)blk_id;
    uint8_t *dst = buf;

    while (blk_cnt > 0) {
        uint8_t count = blk_cnt > 255 ? 255 : (uint8_t)blk_cnt;
        if (ata_lba_read(lba, count, dst)) return EIO;

        lba += count;
        dst += count*ATA_SECTOR_SIZE;
        blk_cnt -= count;
    }

    return 0;
}

// lwext4 callback
static int ata_bdev_write(struct ext4_blockdev *bdev, const void *buf, uint64_t blk_id, uint32_t blk_cnt) {
    return -1; // Not implemented, beginning shouldn't be writing to the drive anyways.
}

// lwext4 callbacks
// The drive is already running, so it doesn't need to be opened.
// Since linux is on the drive, why would you need to close it?
static int ata_bdev_open(struct ext4_blockdev *bdev) { return 0; }
static int ata_bdev_close(struct ext4_blockdev *bdev) { return 0; }
