#include <stdint.h>
#include "io.h"

// Reads a specific num of 512-byte sectors from the drive into RAM.
int ata_lba_read(uint32_t lba, uint8_t sect_count, uint8_t* target_addr) {
    // https://wiki.osdev.org/ATA_PIO_Mode 28 bit PIO section

    // It sends 16 bits (1 word) at a time.
    uint16_t* target = (uint16_t*) target_addr;

    // Send the highest 4 bits of the LBA.
    // outb(0x1F6, 0xE0 | (slavebit << 4) | ((lba >> 24) & 0x0F));
    outb(0x1F6, 0xE0 | ((lba >> 24) & 0x0F));
    
    // Send the sector count and the rest of the LBA to the controller.
    outb(0x1F2, (unsigned char) sect_count);
    outb(0x1F3, (unsigned char) lba);
    outb(0x1F4, (unsigned char)(lba >> 8));
    outb(0x1F5, (unsigned char)(lba >> 16));

    // Send the "read sectors" command to the command port.
    outb(0x1F7, 0x20);

    // Spam the status port until the drive is ready to send data.
    // Bit 7 (0x80) = BSY (Busy). Loop when bit is 1.
    // Bit 3 (0x08) = DRQ (Data Request Ready). Loop when bit is 0.
    uint8_t status;
    while (((status = inb(0x1F7)) & 0x88) != 0x08);
    

    // Read data from the drive.
    // Sector is 512 bytes, inw() reads 2 bytes at a time, 
    // we need a loop that runs 256 times per sect_count.
    // The drive also has to be polled again before every
    // sector.
    int loop_amount = sect_count ? sect_count : 256;
    for (int i=0; i<loop_amount; i++) {
        // Bit 7 (0x80) = BSY, bit 3 (0x08) = DRQ, bit 0 (0x01) = ERR.
        uint8_t status;
        while ((status = inb(0x1F7)) & 0x80);
        while (!(status & 0x09)) status = inb(0x1F7);
        if (status & 0x01) return -1;
        
        for (int i=0; i<256; i++) {
            *target++ = inw(0x1F0);
        }
    }

    return 0;
}