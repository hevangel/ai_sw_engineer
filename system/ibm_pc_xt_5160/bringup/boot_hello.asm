; Hand-written bring-up baseline: boot sector that prints one line via BIOS
; INT 10h teletype and halts. NOT compiler output. Purpose: qualify the
; execution oracles (ai_hw_engineer XT RTL = primary, MartyPC = secondary)
; and the boot-image packaging path before the Rust backend exists.
;
; Build: nasm -f bin boot_hello.asm -o boot_hello.bin   (512 bytes, AA55)
; Expected: boots on the XT profile, prints HELLO line top-left, halts with
; interrupts disabled. See expected.txt and ../../plans/testplan.md.

        BITS 16
        ORG 0x7C00

start:
        cli                     ; deterministic flags/stack setup first
        xor ax, ax
        mov ds, ax
        mov es, ax
        mov ss, ax
        mov sp, 0x7C00          ; stack grows down below the loaded sector
        sti
        cld                     ; XT-A3: DF=0 across calls; lodsb needs it

        mov si, msg
print:
        lodsb
        test al, al
        jz done
        mov ah, 0x0E            ; INT 10h teletype, page 0
        xor bx, bx
        int 0x10
        jmp print

done:
        cli
halt:   hlt
        jmp halt

msg:    db 'XT-BOOT-HELLO', 0x0D, 0x0A, 0

        times 510-($-$$) db 0
        dw 0xAA55
