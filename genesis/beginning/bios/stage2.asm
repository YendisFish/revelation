[BITS 16]
[ORG 0x7E00]
KERNEL_ENTRY equ 0x9000       ; must match the load address in linker.ld
KERNEL_LBA   equ 5            ; sect 0 = boot.bin, sect 1-4 = stage2.bin
READ_CHUNK   equ 64           ; sect per BIOS call

; KERNEL_SECTORS is passed by the Makefil (nasm -DKERNEL_SECTORS=)
%ifndef KERNEL_SECTORS
    %error "KERNEL_SECTORS is not defined, build with make!"
%endif

stage2_entry:
    mov [BD], dl ; boot.asm passes BD in dl

    mov si, msg_succ
    call print_string16

    call load_kernel

    mov si, msg_e820
    call print_string16

    ; Point ES:DI to 0x1004, which leaves 0x8000-0x8003 open
    xor ax, ax
    mov es, ax
    mov di, 0x1004

    call do_e820

    mov si, msg_done
    call print_string16

    mov si, msg_a20
    call print_string16

    call do_a20

    mov si, msg_switching
    call print_string16

    ; Switch to 32-bit protected mode
    cli                   ; Disables interrupts
    lgdt [gdt_descriptor] ; Load the GDT descriptor
    mov eax, cr0
    or eax, 1             ; Set the PE (Protection Enable) bit to 1
    mov cr0, eax          ; Enable protected mode
    jmp CODE_SEG:pme      ; "Far jump" to the protected mode entry
    ;                       This forces the CPU to flush its instruction
    ;                       queue/pipeline of any leftover 16-bit instructions.

load_kernel:
    mov cx, KERNEL_SECTORS
.next_chunk:
    mov bx, READ_CHUNK
    cmp cx, bx
    jae .read
    mov bx, cx
.read:
    mov [dap_count], bx
    mov si, dap
    mov dl, [BD]
    mov ah, 0x42
    int 0x13
    jc .failed

    add [dap_lba], bx
    sub cx, bx
    shl bx, 5           ; Sectors to 16-byte (512/16=32)
    add [dap_segment], bx
    test cx, cx
    jnz .next_chunk
    ret
.failed:
    mov si, msg_load_err
    call print_string16

halt:
    cli
    hlt
    jmp halt

print_string16:
    mov ah, 0x0E    ; BIOS teletype function
.next_char:
    lodsb
    test al, al
    jz .done

    cmp al, 0x0A    ; Check if character is a newline (\n)
    jne .print      ; If not, print it normally

    ; If it is a newline, print a Carriage Return (0x0D) first
    mov al, 0x0D
    int 0x10
    mov al, 0x0A    ; Restore the Line Feed character

.print:
    int 0x10
    jmp .next_char
.done:
    ret

msg_succ db `Stage 2 loaded successfully!\n`, 0 ; atp, no more 512-byte limit.
msg_e820 db `Building E820 map...\n`, 0
msg_done db `Done!\n`, 0
msg_a20 db `Turning on A20...\n`, 0
msg_switching db `Switching to 32-bit protected mode...\n`, 0
msg_load_err db `Kernel read failed!\n`, 0

BD db 0 ; boot drive

; DAP (Disk Address Packet) for int 0x13, AH=0x42
dap:
    db 16, 0             ; Packet size
dap_count:
    dw 0                 ; Sectors to read
    dw 0                 ; Buffer offset
dap_segment:
    dw KERNEL_ENTRY >> 4 ; Buffer segment
dap_lba:
    dq KERNEL_LBA        ; Starting LBA

; https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html
; Volume 3A
; Chapter 3: Protected-Mode Memory Management
; Section 3.4.5: Segment Descriptors
; Figure 3-8
gdt_start:
gdt_null:
    ; The first entry HAS TO BE EXACTLY 8 bytes of zeros.
    dd 0x0
    dd 0x0
gdt_unused:
    ; The linux boot protocol requires CS=0x10 and DS=0x18, so skip 0x08.
    dd 0x0
    dd 0x0
gdt_code:
    ; Code segment descriptor
    dw 0xFFFF          ; Limit (15:0)
    dw 0x0000          ; Base (15:0)
    db 0x00            ; Base (23:16)
    db 10011010b       ; Access byte
    db 11001111b       ; Flags and Limit (19:16)
    db 0x00            ; Base (31:24)
gdt_data:
    ; Data segment descriptor
    dw 0xFFFF          ; Limit (15:0)
    dw 0x0000          ; Base (15:0)
    db 0x00            ; Base (23:16)
    db 10010010b       ; Access byte
    db 11001111b       ; Flags and Limit (19:16)
    db 0x00            ; Base (31:24)
gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1  ; Limit (size of GDT - 1)
    dd gdt_start                ; Base address of GDT

; Calculate the offsets for our segment registers
CODE_SEG equ gdt_code - gdt_start
DATA_SEG equ gdt_data - gdt_start

; A20 = address line 20, the 21st address wire coming out of the CPU.
; Back during the 8086 days, it only had 20 address lines, so addresses
; past 1MB wrapped back to 0, and some older software relied on that.
; Later on, IBM added a gate that can hold A20 at 0, so those programs
; kept working on newer CPUs.
;
; TL;DR: It's a compatibility thing.
do_a20:
    in al, 0x92
    test al, 2                  ; Checks to see is A20 is already on
    jnz .a20_done
    or al, 2                    ; Set the A20 bit
    and al, 0xFE                ; Bit 0 is fast reset, NEVER write 1 to it
    out 0x92, al
.a20_done:
    ret

; https://wiki.osdev.org/Detecting_Memory_(x86)#Getting_an_E820_Memory_Map
mmap_ent equ 0x1000             ; the number of entries will be stored at 0x1000
do_e820:
    mov di, 0x1004              ; Set di to 0x1004. Otherwise this code will get stuck in `int 0x15` after some entries are fetched 
	xor ebx, ebx		        ; ebx must be 0 to start
	xor bp, bp		            ; keep an entry count in bp
	mov edx, 0x0534D4150	    ; Place "SMAP" into edx
	mov eax, 0xe820
	mov [es:di + 20], dword 1	; force a valid ACPI 3.X entry
	mov ecx, 24		            ; ask for 24 bytes
	int 0x15
	jc short .failed	        ; carry set on first call means "unsupported function"
	mov edx, 0x0534D4150	    ; Some BIOSes apparently trash this register?
	cmp eax, edx		        ; on success, eax must have been reset to "SMAP"
	jne short .failed
	test ebx, ebx		        ; ebx = 0 implies list is only 1 entry long (worthless)
	je short .failed
	jmp short .jmpin
.e820lp:
	mov eax, 0xe820		        ; eax, ecx get trashed on every int 0x15 call
	mov [es:di + 20], dword 1	; force a valid ACPI 3.X entry
	mov ecx, 24		            ; ask for 24 bytes again
	int 0x15
	jc short .e820f		        ; carry set means "end of list already reached"
	mov edx, 0x0534D4150	    ; repair potentially trashed register
.jmpin:
	jcxz .skipent	        	; skip any 0 length entries
	cmp cl, 20		            ; got a 24 byte ACPI 3.X response?
	jbe short .notext
	test byte [es:di + 20], 1	; if so: is the "ignore this data" bit clear?
	je short .skipent
.notext:
	mov ecx, [es:di + 8]	    ; get lower uint32_t of memory region length
	or ecx, [es:di + 12]	    ; "or" it with upper uint32_t to test for zero
	jz .skipent		            ; if length uint64_t is 0, skip entry
	inc bp			            ; got a good entry: ++count, move to next storage spot
	add di, 24
.skipent:
	test ebx, ebx		        ; if ebx resets to 0, list is complete
	jne short .e820lp
.e820f:
	mov [es:mmap_ent], bp	    ; store the entry count
	clc			                ; there is "jc" on end of list to this point, so the carry must be cleared
	ret
.failed:
	stc			                ; "function unsupported" error exit
	ret

; =============================
;     32-bit protected mode
; =============================

[BITS 32]
pme:
    ; Set up segment registers for 32-bit protected mode
    mov ax, DATA_SEG
    mov ds, ax
    mov ss, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    ; Update stack pointer to a safe, high memory address
    mov ebp, 0x90000
    mov esp, ebp

    ; Cant use interrupts anymore, so we print using the VGA text buffer.
    ; [ASCII byte][colorbyte]
    mov al, '.'
    mov ah, 0x0F            ; White text (F) on black background (0)
    mov [0xB8000], ax

    ; kmain lives in kernel.bin (a separate binary linked at 0x9000), so NASM
    ; can't resolve it by name. linker.ld places kmain first in the image.
    call KERNEL_ENTRY         ; kmain in main.c

    cli
    hlt

; Pad to exactly four sectors so kernel.bin starts on a sector boundary at LBA 5.
times 2048 - ($ - $$) db 0
