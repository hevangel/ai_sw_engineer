; libi4004rt -- crt0 (reset vector and C-runtime startup)
; ============================================================================
; The 4004 fetches word 0 on reset (isa.md sec.6), so .reset is placed at ROM
; 0x000 by platform/link.ld. crt0 performs, in order (abi.md sec.4.4, link.ld):
;   1. select RAM bank 0 (DCL) -- reset already selects bank 0; done explicitly
;      so state is defined after a warm restart too;
;   2. initialise the software-stack pointer SSP (P7 = R14:R15) to __sstack_base;
;   3. clear .bss (__bss_size nibbles from __bss_start) in 4002 RAM;
;   4. copy .data's ROM load image (__data_load) to its RAM run address
;      (__data_size nibbles);  -- see the page-local FIN limitation below;
;   5. JMS main;  if main returns, spin forever (no OS to return to).
;
; Loops count DOWN a nibble counter (the arena is <=64 nibbles; link.ld's RAM
; region is 64 nibbles, so a size fits two nibbles, but the Busicom firmware's
; .bss/.data are EMPTY -> both sizes are 0 and the loops run zero times). The
; counters here are 8-bit (R8=hi,R9=lo via P4) so the general <=255-nibble case
; is handled; the zero pre-check makes an empty section a no-op.
;
; .data COPY is page-local: FIN indexes the CURRENT ROM page by pair 0
; (isa.md sec.9), so this copies a .data load image on ROM page 0 only. A .data
; image on another page needs a per-page FIN trampoline (abi.md sec.7). The
; yardstick firmware has no .data, so this path is exercised only by the crt0
; self-test (a tiny on-page .data). Documented limit, not a silent bug.
;
; Linker symbols (emitted by llvm-i4004/asm/i4004_pack.py):
;   __sstack_base __bss_start __bss_size __data_load __data_start __data_size

	.section .reset,"ax"
	.globl	_start
_start:
	; -- 1. RAM bank 0 --
	ldm	0
	dcl

	; -- 1b. active-bank mirror = 0 (xl bank switching, 8-bit latch). The ROM
	; bank latch is now TWO 4001 output-port nibbles: low on chip 3 (SRC 0x30),
	; high on chip 4 (SRC 0x40), giving an 8-bit (256-bank) selector that the
	; >16-bank firmware needs. The trampolines/__soft_ret keep a 2-char RAM
	; mirror of the active bank: low nibble @ 0x0F, high nibble @ 0x0E of
	; chip0/reg0. Clear both mirror chars AND both latch ports so the reset
	; state is bank 0 and the halt return token (pushed below via __push_pg,
	; which reads the mirror) carries bank 0. Harmless on the stock board.
	fim	p0, 0x0f
	src	p0
	ldm	0
	wrm			; __cur_bank.lo = 0
	fim	p0, 0x0e
	src	p0
	ldm	0
	wrm			; __cur_bank.hi = 0
	fim	p0, 0x30
	src	p0
	ldm	0
	wrr			; ROM bank latch low nibble (chip 3) = 0
	fim	p0, 0x40
	src	p0
	ldm	0
	wrr			; ROM bank latch high nibble (chip 4) = 0

	; -- 2. SSP = __sstack_base --
	fim	p7, __sstack_base

	; -- 3. clear .bss --
	fim	p4, __bss_size		; P4 = byte count (R8 hi, R9 lo)
	; if count == 0, skip
	ld	r8
	jcn	nz, .Lbss_run
	ld	r9
	jcn	z, .Lbss_skip
.Lbss_run:
	fim	p6, __bss_start		; P6 (R12:R13) = dst SRC address
.Lbss_loop:
	src	p6
	ldm	0
	wrm				; RAM[dst] = 0
	inc	r13			; dst.char++
	; count-- (8-bit P4): if R9!=0 R9--, else R9=15,R8--
	ld	r9
	jcn	z, .Lbss_hi
	ld	r9
	dac
	xch	r9
	jun	.Lbss_chk
.Lbss_hi:
	ldm	15
	xch	r9
	ld	r8
	dac
	xch	r8
.Lbss_chk:
	ld	r9
	jcn	nz, .Lbss_loop
	ld	r8
	jcn	nz, .Lbss_loop
.Lbss_skip:

	; -- 4. copy .data (ROM load image, page 0) to RAM --
	fim	p4, __data_size
	ld	r8
	jcn	nz, .Ldata_run
	ld	r9
	jcn	z, .Ldata_skip
.Ldata_run:
	; P0 (R0:R1) = ROM source index for FIN (pair 0 indexes current page).
	; __data_load is a ROM byte address; its page must be 0 (see note). The
	; FIM IMM8 relocation keeps only the low byte, which is the on-page index.
	fim	p0, __data_load
	fim	p6, __data_start	; P6 = RAM dst SRC address
.Ldata_loop:
	fin	p2			; P2 <- ROM[{page0, R0, R1}]  (byte -> R4:R5? no: FIN loads the pair named)
	; FIN p2 loads R4:R5 from ROM byte {page,R0,R1}: R4=hi nibble, R5=lo nibble.
	; store both nibbles to RAM dst and dst+1.
	src	p6
	ld	r5
	wrm				; RAM[dst] = ROM byte low nibble
	inc	r13
	src	p6
	ld	r4
	wrm				; RAM[dst+1] = ROM byte high nibble
	inc	r13
	inc	r1			; ROM source index++ (next byte)
	; count -= 2 bytes-as-2-nibbles? __data_size counts nibbles; we wrote 2.
	; decrement count by 2 (two nibbles per ROM byte).
	ld	r9
	dac
	xch	r9			; -1
	ld	r9
	dac
	xch	r9			; -2 (low nibble; data copy limited to <16 nibbles on-page)
	ld	r9
	jcn	nz, .Ldata_loop
.Ldata_skip:

	; -- 5. call main via the SOFTWARE-call convention --
	; main is a compiled function: it returns via `jun __soft_ret`, which pops
	; the P7 soft stack, so it must be ENTERED the soft way (push a return token
	; for .Lhalt, then JUN main) -- NOT `jms main`. Use the same P0-only leaf
	; push helpers the backend emits (abi.md 4.4.1).
	fim	p0, %lo8(.Lhalt)
	jms	__push_off
	fim	p0, %page(.Lhalt)
	jms	__push_pg
	jun	main
.Lhalt:
	jun	.Lhalt			; main returned: spin forever
