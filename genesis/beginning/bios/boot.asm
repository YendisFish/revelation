[BITS 16]
[ORG 0x7C00]

start:
    ; Disable interrupts and set up segment registers
    cli             ; Disables interrupts
    xor ax, ax      ; We need a 0 to configure the segment registers
    mov ds, ax      ; BIOS might leave junk in the data segment, so we clear it
    mov es, ax      ; Clear for the same reason as above.
    mov ss, ax      ; Tells the CPU which memory segment holds the stack.
    mov sp, 0x7C00  ; Sets the top of the stack. The BIOS loads this program at 0x7C00.
    sti             ; Turns back on hardware interrupts.

    ; Save the boot drive number given by the BIOS
    mov [BD], dl

    mov ah, 0x02    ; BIOS function to read sectors from disk
    mov al, 4       ; Number of sectors to read (1 sector = 512 bytes, stage 2 is 2kb)
    mov ch, 0       ; Cylinder number (0 for the first cylinder)
    mov cl, 35      ; LBA 34 = Sector 35 (safe from GPT sectors 0-33) 
    mov dh, 0       ; Head number (0 for the first head)
    mov bx, 0x7E00  ; Buffer to read the sector into (0x7E00 is just below the start of the stack)
    int 0x13        ; Call the BIOS disk interrupt
    jc disk_error   ; Jump to error handling if the carry flag (CF) is set

    mov dl, [BD]    ; Hand the boot drive to stage2
    jmp 0x7E00      ; Jump to the loaded sector (the next stage of the bootloader)

disk_error:
    mov si, msg_err
    call print_string
    hlt             ; Halt the CPU if there is a disk error, helps to mitigate any physical damage to the drive.

print_string:
    mov ah, 0x0E    ; BIOS teletype function
.next_char:
    lodsb
    test al, al
    jz .done
    int 0x10
    jmp .next_char
.done:
    ret

BD db 0
msg_err db 'Disk read error!', 0

times 510 - ($ - $$) db 0 ; Fill the rest of the boot sector with zeros
dw 0xAA55 ; Boot sector signature aka the magic number

