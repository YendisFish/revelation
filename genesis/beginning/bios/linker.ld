ENTRY(kmain)
OUTPUT_FORMAT("binary")

SECTIONS {
    . = 0x9000;

    .text : ALIGN(4K) {
        *(.text.start)   /* kmain, so the image's first byte is the entry point */
        *(.text)
    }

    .rodata : ALIGN(4K) {
        *(.rodata)
    }

    .data : ALIGN(4K) {
        *(.data)
    }

    .bss : ALIGN(4K) {
        __bss_start = .;
        *(COMMON)
        *(.bss)
        __bss_end = .;
    }
}
