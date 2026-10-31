/* llvm-mos6502 bare-metal qualification link script (issue hevangel/ai_engineer#58).
 *
 * Self-contained (the installed llvm-mos SDK's `common` INCLUDE chain does not
 * parse with the image's lld 20 — see report/qualification.md). Layout:
 *
 *   $0002-$00xx  imaginary zero-page registers __rc0..__rc31, then zp region
 *   $0100        hardware stack page (S fixed at page 1 by the ABI)
 *   $0200-$7FFF  RAM: .data (LMA copied from ROM by libcopy-data), .bss
 *   $8000-$FFF8  ROM: .text, .rodata, .data initializers, vectors
 *   $FFFA-$FFFF  NMI / RESET / IRQ vectors (RESET = crt0 `_start`)
 *
 * Only two files from the pinned SDK's common platform are consumed, both
 * plain symbol assignments: `crt0.o` (entry: set up, call main, park) via the
 * target's default `-l:crt0.o -lcrt0 -lcrt -lc` chain, and `imag-regs.ld`.
 *
 * Build (inside the ai-sw-engineer container):
 *   RUSTFLAGS="-C link-arg=-L<repo>/llvm-mos6502/spec -C link-arg=-L/usr/local/mos-platform/common/lib"
 *   cargo +mos build --release --target mos-unknown-none -Z build-std=core
 */

__rc0 = 0x0002;
INCLUDE imag-regs.ld

MEMORY {
    zp  (rw) : ORIGIN = __rc31 + 1, LENGTH = 0x00FD - (__rc31 + 1)
    ram (rw) : ORIGIN = 0x0200, LENGTH = 0x7E00
    rom (rx) : ORIGIN = 0x8000, LENGTH = 0x8000
}

/* Software stack: page 1, grows down from 0x01FF (llvm-mos init-stack). */
__stack = 0x01FF;

REGION_ALIAS("c_readonly", rom)
REGION_ALIAS("c_writeable", ram)

SECTIONS {
    .text : { *(.text .text.*) } > rom
    .rodata : { *(.rodata .rodata.*) } > rom

    .init_array : {
        __init_array_start = .;
        KEEP(*(.init_array .init_array.*))
        __init_array_end = .;
    } > rom
    .fini_array : {
        __fini_array_start = .;
        KEEP(*(.fini_array .fini_array.*))
        __fini_array_end = .;
    } > rom

    /* RAM sections whose initializers live in ROM; libcopy-data.a copies
     * .data, libcopy-zp-data.a copies zp_data at startup. LMAs are computed
     * explicitly (the image's lld does not accept `AT>region`). */
    __data_lma = ORIGIN(rom) + SIZEOF(.text) + SIZEOF(.rodata)
               + SIZEOF(.init_array) + SIZEOF(.fini_array);
    .data : AT(__data_lma) { *(.data .data.*) } > ram
    __data_start = ADDR(.data);
    __data_end = ADDR(.data) + SIZEOF(.data);
    __data_load_start = __data_lma;
    __data_load_end = __data_lma + SIZEOF(.data);

    __zp_data_lma = __data_lma + SIZEOF(.data);
    zp_data : AT(__zp_data_lma) { *(.zp.data .zp.data.*) } > zp
    __zp_data_start = ADDR(zp_data);
    __zp_data_load_start = __zp_data_lma;
    __zp_data_load_end = __zp_data_lma + SIZEOF(zp_data);

    .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } > ram
    __bss_start = ADDR(.bss);
    __bss_end = ADDR(.bss) + SIZEOF(.bss);

    zp_bss (NOLOAD) : { *(.zp.bss .zp.bss.*) } > zp
    __zp_bss_start = ADDR(zp_bss);
    __zp_bss_end = ADDR(zp_bss) + SIZEOF(zp_bss);

    .noinit (NOLOAD) : { *(.noinit .noinit.*) } > ram

    /* NMI / RESET / IRQ. RESET enters our crt0 `_start`. NMI/IRQ default to
     * the crt0 trap loop unless the program defines its own vectors. */
    __nmi_vector = DEFINED(__nmi_vector) ? __nmi_vector : __trap;
    __irq_vector = DEFINED(__irq_vector) ? __irq_vector : __trap;
    .vectors 0xFFFA : {
        SHORT(__nmi_vector)
        SHORT(_start)
        SHORT(__irq_vector)
    } > rom
}

ENTRY(_start)
