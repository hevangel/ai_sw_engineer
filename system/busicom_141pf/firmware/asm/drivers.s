; Busicom 141-PF firmware -- keyboard and printer drivers (issue #18, B5)
; ============================================================================
; The port loops of `src/driver_keyboard.rs` (`KeyboardDriver::scan_latch`) and
; `src/driver_printer.rs` (`Printer::print`) on the target. The Rust drivers are
; the host twins, proven against the board model (`tests/port_level.rs`); these
; are proven end to end by the 42-example tape on the simulators
; (`platform/scripts/run_image.sh`), whose drum is driven by time.
;
; Port map (src/hal.rs, platform/io_map.md): ROM0 out = bit 0 keyboard clock,
; bit 1 serial data, bit 2 printer clock (clocks active low); ROM1 in = the
; keyboard column nibble; ROM2 in bit 0 = drum index; RAM0 out = bit 0 red,
; bit 1 fire (rising edge), bit 3 paper advance (rising edge); TEST = drum
; half-spin. The keyboard chain selects a row with a 0; the printer chain fires
; a hammer with a 1.
;
; Both are entries: called from the main loop or a handler (call depth 1-2),
; they use one more level for their local bit routine. They clobber P0-P5 and
; keep P6.

; ---------------------------------------------------------------------------
; __kb_scan() -> row << 4 | column: one keyboard scan pass. Walk a selection bit
; through 4003 #0 (ten 0s, then the marker), read each row's column nibble:
; Q0-Q7 decode a key (one bit set) and return its matrix position (code - 129
; = 4 * row + column; 0xFF: none; row 3 columns 2-3 carry no key and the
; dispatcher ignores them;
; the target loop scans only between keys, so a pass never holds more than one
; and the host twin's 8-key FIFO is never deeper than one); Q8/Q9 latch the
; DP and rounding selectors into 0xC0 S2/S3 (rounding: 1 -> 1 = 5/4, 8 -> 2 =
; IN, else 0 = FL). A key is taken only when the previous pass saw no key
; down (status 0xF0 S0), so a held key is one event. P1 = result, R7 = data
; for .Lkb_bit, R8 = a key is down, R9 = row, R10 = columns, R11 = key down on
; the last pass.
	.section .text.__kb_scan,"ax"
	.globl	__kb_scan
__kb_scan:
	fim	p0, 0xF0
	src	p0
	rd0
	xch	r11
	fim	p1, 0xFF	; no key yet
	ldm	2		; ten 0s clear the selection
	xch	r7
	fim	p4, 0x06	; R8 = no key down, R9 = 6..15
1:	jms	.Lkb_bit
	isz	r9, 1b
	ldm	0		; then the marker: Q0 selected
	xch	r7
	jms	.Lkb_bit
	ldm	2
	xch	r7
	ldm	0
	xch	r9
2:	fim	p0, 0x10	; ROM1: the selected row's columns
	src	p0
	rdr
	xch	r10
	ld	r9		; Q8, Q9: the selectors
	ral
	jcn	c, 6f
	ld	r10
	jcn	z, 5f		; no key in this row
	ldm	1
	xch	r8
	ld	r11
	jcn	nz, 5f		; held since the last pass
	ldm	0		; R6 = column of the lowest set bit
	xch	r6
3:	clc
	ld	r10
	rar
	xch	r10
	jcn	c, 4f
	inc	r6
	jun	3b
4:	ld	r10		; a second bit: rollover, ignored
	jcn	nz, 5f
	ld	r9		; the key: row in R2, column in R3
	xch	r2
	ld	r6
	xch	r3
	jun	5f
6:	fim	p0, 0xC0
	src	p0
	ld	r9
	rar
	ld	r10
	jcn	c, 61f
	wr2		; Q8: DP selector
	jun	5f
61:	ldm	1		; Q9: rounding selector
	xch	r6
	ld	r10
	dac
	jcn	z, 62f
	ldm	2
	xch	r6
	clc
	ldm	8
	sub	r10
	jcn	z, 62f
	ldm	0
	xch	r6
62:	ld	r6
	wr3
5:	jms	.Lkb_bit	; walk to the next row
	inc	r9
	clc
	ldm	10
	sub	r9
	jcn	nz, 2b
	fim	p0, 0xF0
	src	p0
	ld	r8
	wr0
	bbl	0

.Lkb_bit:			; shift R7 (2: a 0, 0: the marker) into 4003 #0
	fim	p0, 0
	src	p0
	ld	r7
	iac
	wrr
	ld	r7
	wrr
	iac
	wrr
	bbl	0

; ---------------------------------------------------------------------------
; __pr_print(): print the row in the print register (PR = 0xE0, print.rs:
; main chars 0-14 = each numeric column's drum sector, S0/S1 = the sectors of
; symbol columns A/B, S2 bit 0 = red) on one drum revolution, then advance the
; paper. Drum timing: TEST changes every half-spin, a sector is two half-spins,
; and the index (ROM2 bit 0) rises one half-spin before sector 0. Only TEST
; edges are counted, never its level: external/chips and the RTL present the
; same pin with opposite JCN polarity (failure_notes/2026-10-22-busicom_141pf-
; drum-phase.md). Sector 0 starts at the first edge after the index is seen;
; each later sector two edges on. Per sector: shift the 20-bit hammer word MSB first (bits
; 19, 18, 2 idle; 17..3 = columns 14..0; 1 = B; 0 = A; a bit is 1 when that
; column's sector is the current one) and fire. R6 = sector, R7 = data for
; .Lpr_bit, R10 = red, P1 = column pointer.
;
; __pr_row(src, syms): fill the print register as `print::Row` does, then print
; it. src = R2:R3 = a number's register (0..7) and 0: Row::number -- columns
; BLANK (15), the max(digits, scale + 1) digits right-aligned with POINT (10)
; before digit `scale` when scale > 0, red when negative (width and scale are
; taken mod 16: printed numbers are rounded to <= 14 digits first); src =
; 0xF0 + v: every column v (Row::blank: 15, Row::overflow: 10). syms = R4:R5 =
; the sectors of symbol columns A and B (BLANK for none).
	.section .text.__pr_print,"ax"
	.globl	__pr_row
	.globl	__pr_print
__pr_row:
	fim	p0, 0xE0	; the symbols
	src	p0
	ld	r4
	wr0
	ld	r5
	wr1
	ld	r2
	xch	r8		; R8 = the number's register, or 15
	ldm	15
	xch	r9		; R9 = the fill value
	ld	r8
	iac
	jcn	nz, 11f
	ld	r3
	xch	r9
11:	ldm	1		; fill the 15 columns
	xch	r11
	fim	p0, 0xE0
12:	src	p0
	ld	r9
	wrm
	inc	r1
	isz	r11, 12b
	ldm	0		; R6 = red
	xch	r6
	ld	r8
	iac
	jcn	z, 15f		; a fill row
	ld	r8		; R9 = width
	xch	r2
	ldm	0
	xch	r3
	jms	__leaf_n_width
	ld	r3
	xch	r9
	ld	r8		; R10 = scale, R6 = sign
	xch	r0
	ldm	0
	xch	r1
	src	p0
	rd1
	xch	r10
	rd0
	xch	r6
	ldm	0		; R11 = digit i, P2 = column (pre-decremented)
	xch	r11
	fim	p2, 0xEF
13:	clc			; while i < width
	ld	r11
	sub	r9
	jcn	c, 15f
	ld	r10		; the point before digit `scale`
	jcn	z, 14f
	clc
	ld	r11
	sub	r10
	jcn	nz, 14f
	ld	r5
	dac
	xch	r5
	src	p2
	ldm	10
	wrm
14:	ld	r5
	dac
	xch	r5
	ld	r8
	xch	r0
	ld	r11
	xch	r1
	src	p0
	rdm
	src	p2
	wrm
	inc	r11
	jun	13b
15:	fim	p0, 0xE0	; flags: red
	src	p0
	ld	r6
	wr2
	xch	r10
	jun	.Lpr_go

__pr_print:
	fim	p1, 0xE0
	src	p1
	rd2
	rar
	tcc
	xch	r10
.Lpr_go:
	ld	r10
	jcn	z, 1f
	fim	p0, 0		; red ink for the whole line
	src	p0
	wmp
1:	fim	p0, 0x20	; wait for the drum index
	src	p0
2:	rdr
	rar
	jcn	nc, 2b
	ldm	0
	xch	r6
	jms	.Lpr_edge	; the next TEST edge starts sector 0
3:	ldm	0		; bits 19, 18
	xch	r7
	jms	.Lpr_bit
	jms	.Lpr_bit
	fim	p1, 0xEE	; bits 17..3: columns 14..0
4:	src	p1
	rdm
	jms	.Lpr_cmp
	ld	r3
	dac
	xch	r3
	jcn	c, 4b
	ldm	0		; bit 2
	xch	r7
	jms	.Lpr_bit
	src	p1		; bit 1: column B, bit 0: column A
	rd1
	jms	.Lpr_cmp
	src	p1
	rd0
	jms	.Lpr_cmp
	fim	p0, 0		; fire: RAM0 bit 1 rising edge
	src	p0
	ld	r10
	wmp
	clc
	ldm	2
	add	r10
	wmp
	ld	r10
	wmp
	inc	r6
	clc
	ldm	13
	sub	r6
	jcn	z, 6f
	jms	.Lpr_edge	; two TEST edges: the next sector
	jms	.Lpr_edge
	jun	3b
6:	jms	.Lpr_edge	; let sector 12's hammers finish first
	fim	p0, 0		; advance the paper: RAM0 bit 3 rising edge
	src	p0
	ld	r10
	wmp
	clc
	ldm	8
	add	r10
	wmp
	ld	r10
	wmp
	bbl	0

.Lpr_edge:			; wait for the TEST level to change
	jcn	t, 2f
1:	jcn	nt, 1b
	bbl	0
2:	jcn	t, 2b
	bbl	0

.Lpr_cmp:			; shift a 1 when ACC is the current sector R6
	clc
	sub	r6
	jcn	nz, .Lpr_idle
	ldm	2
	jun	.Lpr_set
.Lpr_idle:
	ldm	0
.Lpr_set:
	xch	r7
.Lpr_bit:			; shift R7 (2: fire, 0: idle) into the printer chain
	fim	p0, 0
	src	p0
	clc
	ldm	4
	add	r7
	wrr
	ld	r7
	wrr
	clc
	ldm	4
	add	r7
	wrr
	bbl	0

; ---------------------------------------------------------------------------
; __bus_lamps(): the lamps to RAM1's port, as `Calculator::lamp_nibble`: bit 0
; memory (flag 0x53 S3), bit 1 overflow (0x63 S3), bit 2 minus (WR's sign,
; 0x00 S0), bit 3 when overflow or minus (spec A9). P0-P3 only (a `__bus_`
; leaf). R2 = the nibble.
	.section .text.__bus_lamps,"ax"
	.globl	__bus_lamps
__bus_lamps:
	fim	p1, 0
	fim	p0, 0x00	; minus: 4 + 8
	src	p0
	rd0
	jcn	z, 1f
	ldm	12
	xch	r2
1:	fim	p0, 0x60	; overflow: 2 + 8
	src	p0
	rd3
	jcn	z, 3f
	ld	r2
	jcn	z, 2f
	ldm	14
	jun	21f
2:	ldm	10
21:	xch	r2
3:	fim	p0, 0x50	; memory: 1
	src	p0
	rd3
	jcn	z, 4f
	inc	r2
4:	fim	p0, 0x40	; RAM chip 1 (bank 0)
	src	p0
	ld	r2
	wmp
	bbl	0
