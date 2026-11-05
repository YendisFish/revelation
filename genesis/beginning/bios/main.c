#include <stdint.h>
#include <stddef.h>
#include "ext4.h"
#include "io.h"

// ext4.h
extern struct ext4_blockdev* get_ata_bdev();

void* memcpy(void *dest, const void *src, size_t n) {
    uint8_t *d = (uint8_t *)dest;
    const uint8_t *s = (const uint8_t *)src;
    for (size_t i = 0; i < n; i++) {
        d[i] = s[i];
    }
    return dest;
}

void* memset(void *s, int c, size_t n) {
    uint8_t *p = (uint8_t *)s;
    for (size_t i = 0; i < n; i++) {
        p[i] = (uint8_t)c;
    }
    return s;
}

size_t strlen(const char *s) {
    size_t len = 0;
    while (s[len]) {
        len++;
    }
    return len;
}

/*
 * https://docs.kernel.org/arch/x86/boot.html#the-real-mode-kernel-header
 * Offset/Size  Name                   Meaning
 * -----------  ---------------------  -----------------------------------------
 * 01F1/1       setup_sects            The size of the setup in sectors
 * 01F2/2       root_flags             If set, the root is mounted readonly
 * 01F4/4       syssize                The size of the 32-bit code in 16-byte paras
 * 01F8/2       ram_size               DO NOT USE: for bootsect.S use only
 * 01FA/2       vid_mode               Video mode control
 * 01FC/2       root_dev               Default root device number
 * 01FE/2       boot_flag              0xAA55 magic number
 * 0200/2       jump                   Jump instruction
 * 0202/4       header                 Magic signature "HdrS"
 * 0206/2       version                Boot protocol version supported
 * 0208/4       realmode_swtch         Boot loader hook (see below)
 * 020C/2       start_sys_seg          The load-low segment (0x1000) (obsolete)
 * 020E/2       kernel_version         Pointer to kernel version string
 * 0210/1       type_of_loader         Boot loader identifier
 * 0211/1       loadflags              Boot protocol option flags
 * 0212/2       setup_move_size        Move to high memory size (used with hooks)
 * 0214/4       code32_start           Boot loader hook (see below)
 * 0218/4       ramdisk_image          initrd load address (set by boot loader)
 * 021C/4       ramdisk_size           initrd size (set by boot loader)
 * 0220/4       bootsect_kludge        DO NOT USE: for bootsect.S use only
 * 0224/2       heap_end_ptr           Free memory after setup end
 * 0226/1       ext_loader_ver         Extended boot loader version
 * 0227/1       ext_loader_type        Extended boot loader ID
 * 0228/4       cmd_line_ptr           32-bit pointer to the kernel command line
 * 022C/4       initrd_addr_max        Highest legal initrd address
 * 0230/4       kernel_alignment       Physical addr alignment required for kernel
 * 0234/1       relocatable_kernel     Whether kernel is relocatable or not
 * 0235/1       min_alignment          Minimum alignment, as a power of two
 * 0236/2       xloadflags             Boot protocol option flags
 * 0238/4       cmdline_size           Maximum size of the kernel command line
 * 023C/4       hardware_subarch       Hardware subarchitecture
 * 0240/8       hardware_subarch_data  Subarchitecture-specific data
 * 0248/4       payload_offset         Offset of kernel payload
 * 024C/4       payload_length         Length of kernel payload
 * 0250/8       setup_data             64-bit physical pointer to linked list of struct setup_data
 * 0258/8       pref_address           Preferred loading address
 * 0260/4       init_size              Linear memory required during initialization
 * 0264/4       handover_offset        Offset of handover entry point
 * 0268/4       kernel_info_offset     Offset of the kernel_info
 */

struct setup_header {
    uint8_t	    setup_sects;
    uint16_t	root_flags;
    uint32_t	syssize;
    uint16_t	ram_size;
    uint16_t	vid_mode;
    uint16_t	root_dev;
    uint16_t	boot_flag;      // 0xaa55
    uint16_t	jump;           // Jump instruction
    uint32_t	header;         // Magic signature: "HdrS" (0x53726448)
    uint16_t	version;        // Boot protocol version
    uint32_t	realmode_swtch;
    uint16_t	start_sys_seg;
    uint16_t	kernel_version;
    uint8_t	    type_of_loader;
    uint8_t	    loadflags;
    uint16_t	setup_move_size;
    uint32_t	code32_start;
    uint32_t	ramdisk_image;
    uint32_t	ramdisk_size;
    uint32_t	bootsect_kludge;
    uint16_t	heap_end_ptr;
    uint8_t	    ext_loader_ver;
    uint8_t	    ext_loader_type;
    uint32_t	cmd_line_ptr;
    uint32_t	initrd_addr_max;
    uint32_t	kernel_alignment;
    uint8_t	    relocatable_kernel;
    uint8_t 	min_alignment;
    uint16_t	xloadflags;
    uint32_t	cmdline_size;
    uint32_t	hardware_subarch;
    uint64_t	hardware_subarch_data;
    uint32_t	payload_offset;
    uint32_t	payload_length;
    uint64_t	setup_data;
    uint64_t	pref_address;
    uint32_t	init_size;
    uint32_t	handover_offset;
    uint32_t	kernel_info_offset;
} __attribute__((packed));

struct e820_entry {
    uint64_t    addr;
    uint64_t    size;
    uint32_t    type;
} __attribute__((packed));

// https://docs.kernel.org/arch/x86/zero-page.html
struct boot_params {
    // 0x000 to 0x1E7, Legacy hardware info (zeroed out)
    uint8_t     legacy_padding[0x1E8];
    // 0x1E8, How many entries are in your E820 memory map (0 if not using)
    uint8_t     e820_entries;
    // 0x1E9 to 0x1F0, More legacy padding (EDD info, etc.)
    uint8_t     padding2[8];
    // 0x1F1, The all-important setup_header (starts exactly at 0x1F1)
    struct      setup_header hdr;
    // 0x268 to 0x2CF, Padding between header and E820 map
    uint8_t     padding3[0x2D0 - 0x1F1 - sizeof(struct setup_header)];
    // 0x2D0, The E820 memory map array (up to 128 entries)
    struct      e820_entry e820_map[128];
    // Pad out the rest to make the struct exactly 4096 bytes (4KB)
    uint8_t     padding4[4096 - 0x2D0 - (sizeof(struct e820_entry) * 128)];
    // Other fields can be added as needed.
} __attribute__((packed));

// libc.c
void* memmove(void *dest, const void *src, size_t n);

// Print to COM1 and stop. Without this, a failure just runs on into garbage.
static void die(const char *msg) {
    while (*msg) outb(0x3F8, *msg++);
    outb(0x3F8, '\n');
    while (1) { __asm__ __volatile__ ("cli; hlt"); }
}

static void check(int err, const char *msg) {
    if (err) die(msg);
}

// TODO clean up this file at a later date.
#define VGA_TEXT ((volatile uint16_t*)0xB8000)
#define VGA_COLS 80
#define VGA_ROWS 25

static void vga_clear(void) {
    for (int i=0; i<VGA_COLS*VGA_ROWS; i++) VGA_TEXT[i] = 0x0F00 | ' ';
}

static void vga_print(int row, const char *msg) {
    for (int col=0; msg[col] && col<VGA_COLS; col++) {
        VGA_TEXT[row*VGA_COLS+col] = 0x0F00 | (uint8_t)msg[col];
    }
}

// Since BIOS interrupts dont exist in 32-bit protected mode,
// this polls the PS/2 controller directly.
// TODO Add USB support
static void wait_for_enter(void) {
    while (inb(0x64) & 0x01) inb(0x60); // Get rid of any old keypresses

    while (1) {
        uint8_t status = inb(0x64);
        if (!(status & 0x01)) continue; // Bit 0: no data waiting

        uint8_t scancode = inb(0x60);
        if (status & 0x20) continue;   // Bit 5: that byte came from the mouse
        if (scancode == 0x1C) return;  // Enter key
    }
}

__attribute__((section(".text.start")))
void kmain() {
    extern uint8_t __bss_start[], __bss_end[];
    memset(__bss_start, 0, __bss_end - __bss_start);

    struct boot_params* bp = (struct boot_params*)0x90000; // 576KB mark for boot_params

    uint8_t* kernel_buf = (uint8_t*)0x100000; // 1MB mark for bzImage
    // TODO: have this be dynamically determined. something something E820 memory map, look into it more.
    uint8_t* initramfs_addr = (uint8_t*)0x10000000; // 256MB mark, which helps avoid modern massive, bloated kernels from overwriting the fs
    size_t bytes_read;

    // Init disk and mount fs
    check(ext4_device_register(get_ata_bdev(), NULL, "sda"), "register failed");
    check(ext4_mount("sda", "/", true), "mount failed");

    // Placeholder for the boot menu.
    // List the options of bootable entries on the drive.
    // TODO actual boot menu
    vga_clear();
    vga_print(0, "Press Enter to boot...");
    wait_for_enter();
    vga_clear();
    vga_print(0, "Booting...");

    // Load vmlinuz
    ext4_file kernel_file;
    check(ext4_fopen(&kernel_file, "/vmlinuz", "r"), "open /vmlinuz failed");
    uint32_t kernel_size = ext4_fsize(&kernel_file);
    check(ext4_fread(&kernel_file, kernel_buf, ext4_fsize(&kernel_file), &bytes_read), "read /vmlinuz failed");
    ext4_fclose(&kernel_file);

    // Load initramfs
    ext4_file initramfs_file;
    check(ext4_fopen(&initramfs_file, "/initramfs.cpio", "r"), "open /initramfs.cpio failed");
    uint32_t initramfs_size = ext4_fsize(&initramfs_file);
    check(ext4_fread(&initramfs_file, initramfs_addr, initramfs_size, &bytes_read), "read /initramfs.cpio failed");
    ext4_fclose(&initramfs_file);

    // Files are now in memory.

    char* cmdline = "root=/dev/ram0 rw console=ttyS0 console=tty0"; // Boot args

    memset(bp, 0, sizeof(struct boot_params)); // zero out 4KB block
    memcpy(&bp->hdr, kernel_buf + 0x1F1, sizeof(struct setup_header)); // copy setup_header from the vmlinuz file

    uint16_t entry_count = *(uint16_t*)0x1000;
    if (entry_count > 128) entry_count = 128;

    for (uint16_t i=0; i<entry_count; i++) {
        memcpy(&bp->e820_map[i], (uint8_t*)0x1004 + i * 24, sizeof(struct e820_entry));
    }
    bp->e820_entries = (uint8_t)entry_count;

    // Only the protected-mod part of the bzImage belongs at 1MB, so move it on
    // over the real-mode setup code.
    // setup_sects == 0 is actually 4.
    uint32_t setup_sects = bp->hdr.setup_sects ? bp->hdr.setup_sects : 4;
    uint32_t setup_size = (setup_sects+1) * 512;
    memmove(kernel_buf, kernel_buf+setup_size, kernel_size-setup_size);

    // screen_info, normally filled in by the kernel's real-mode setup code.
    // TODO Do that ^^^^^
    // Without it, the kernel doesnt know about the VGA text screen.
    uint8_t* screen_info = (uint8_t*)bp;
    screen_info[0x06] = 3;              // orig_video_mode: 80x25 color text
    screen_info[0x07] = VGA_COLS;       // orig_video_cols
    screen_info[0x0E] = VGA_ROWS;       // orig_video_lines
    screen_info[0x0F] = 1;              // orig_video_isVGA
    screen_info[0x10] = 16;             // orig_video_points: font height

    bp->hdr.type_of_loader = 0xFF; // 0xFF means custom loader
    bp->hdr.ramdisk_image = (uint32_t)initramfs_addr;
    bp->hdr.ramdisk_size = initramfs_size;

    char* cmdline_dest = (char*)0x91000; // 576KB + 4KB for boot_params
    memcpy(cmdline_dest, cmdline, strlen(cmdline) + 1); // copy cmdline to memory
    bp->hdr.cmd_line_ptr = (uint32_t)cmdline_dest;

    // Hand off to the linux kernel.
    // The 32-bit entry is defined by the kernel in setup_header, usually it's 0x10000 (code32_start)
    uint32_t entry_point = bp->hdr.code32_start;

    // The linux boot protocol wants esi = boot_params struct and ebp, edi, and ebx = 0;
    __asm__ __volatile__ (
        "cli\n\t"
        "xor %%ebx, %%ebx\n\t"
        "xor %%edi, %%edi\n\t"
        "xor %%ebp, %%ebp\n\t"
        "jmp *%%eax\n\t"
        :
        : "S"(bp), "a"(entry_point) // S is esi, a is eax
    );

    // The CPU is now running the linux kernel.
    while (1) {}
    
}