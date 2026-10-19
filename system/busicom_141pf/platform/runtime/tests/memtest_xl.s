; busicom_141pf xl board -- added-memory self-test
; ============================================================================
; A hand-written 4004 program that EXERCISES the memory the xl board adds over
; the stock board and reports a pass/fail verdict on an observable port, so the
; extra 4001 ROM pages and 4002 RAM chips are proven addressable on both
; simulators (run_image.sh --board xl; secondary=chips, primary=RTL).
;
; It does NOT depend on the Rust firmware or the codegen backend; it links with
; crt0.s only (it drives the bus directly, no HAL) and is packed into a
; 4096-byte image (16 x 4001) by i4004_pack.py --max 4096.
;
; ---------------------------------------------------------------------------
; What it tests
; ---------------------------------------------------------------------------
; RAM, CM-RAM bank 0, all four 4002 chips (0,1,2,3), every register (0..3) and
; every main character (0..15) = 256 cells. For each cell it WRMs a pattern
; derived from the address, then RDMs it back and compares; any mismatch sets a
; FAIL flag. This covers the two ADDED bank-0 4002s (chips 2,3) the stock board
; lacks, plus the two originals. Bank 0 is the only RAM bank the external
; `chips` model can represent (its Board::new panics for chip index >= 4), so
; bank 0 is the common ground BOTH simulators run. Banks 1..3 are RTL-only (see
; platform/README.md). This probe runs at power-on, before any firmware.
;
; ROM pages: the image is 16 pages (4096 bytes). Page reach is shown
; structurally -- the 4096-byte image loads on all 16 masks and the CPU runs
; page-0 code the 16-page linker placed. A data FIN-per-page probe is page-local
; (FIN reads the CURRENT page only, isa.md sec.9) so it would need a per-page
; trampoline; the committed in-asm check is the RAM sweep + the full image fill.
;
; ---------------------------------------------------------------------------
; Verdict (RAM1 lamp port, sampled by the runner as "LAMPS|n", io_map.md)
;   LAMPS|7  (0b0111) = PASS  (all 256 bank-0 cells verified)
;   LAMPS|1  (0b0001) = FAIL  (>=1 mismatch)
; then loops forever re-driving the verdict so every sampled step shows it.
; 7 is a value the original firmware never leaves latched, so it is an
; unambiguous "this test drove RAM1" signal.
;
; Register usage:
;   P0 (R0:R1) SRC pointer {chip[7:6],reg[5:4],char[3:0]}
;   R4  FAIL flag (0 ok, 1 mismatch)   R6  scratch (read value)
;   R8  chip 0..3   R9  reg 0..3   R10 char 0..15
	.text
	.globl	main
main:
	ldm	0
	xch	r4			; FAIL = 0
	ldm	0
	dcl				; CM-RAM bank 0

	ldm	0
	xch	r8			; chip = 0
.Lchip:
	ldm	0
	xch	r9			; reg = 0
.Lreg:
	; ---- WRITE 16 chars of (chip,reg) ----
	ldm	0
	xch	r10			; char = 0
.Lwr:
	ld	r8
	clc
	ral				; chip<<1
	clc
	ral				; chip<<2  -> SRC bits [3:2] of hi nibble
	add	r9			; + reg    -> SRC bits [1:0] of hi nibble
	xch	r0			; P0.hi = {chip,reg}
	ld	r10
	xch	r1			; P0.lo = char
	src	p0
	ld	r10
	add	r8
	add	r9			; pattern = (char+chip+reg) & 0xF
	wrm
	isz	r10, .Lwr		; char++ until wrap 15->0

	; ---- READ-BACK + verify 16 chars of (chip,reg) ----
	ldm	0
	xch	r10
.Lrd:
	ld	r8
	clc
	ral
	clc
	ral
	add	r9
	xch	r0
	ld	r10
	xch	r1
	src	p0
	rdm				; ACC = stored nibble
	xch	r6			; R6 = read value
	ld	r10
	add	r8
	add	r9			; ACC = expected
	clc
	sub	r6			; ACC = expected-read (ZERO if equal)
	jcn	z, .Lrd_ok
	ldm	1
	xch	r4			; FAIL = 1
.Lrd_ok:
	isz	r10, .Lrd		; char++ until wrap

	; reg++ ; loop while reg < 4
	ld	r9
	iac
	xch	r9			; reg++
	ldm	4
	clc
	sub	r9			; ACC = 4 - reg (ZERO when reg == 4)
	jcn	z, .Lchip_next
	jun	.Lreg

.Lchip_next:
	; chip++ ; loop while chip < 4
	ld	r8
	iac
	xch	r8			; chip++
	ldm	4
	clc
	sub	r8			; ACC = 4 - chip (ZERO when chip == 4)
	jcn	z, .Lverdict
	jun	.Lchip

.Lverdict:
	ld	r4
	jcn	z, .Lpass		; FAIL==0 -> PASS
	fim	p0, 0x40		; RAM1 (chip 1): SRC hi nibble 0x4 -> [7:6]=01
	src	p0
	ldm	1			; 0b0001 = FAIL
	wmp
	jun	.Lspin
.Lpass:
	fim	p0, 0x40
	src	p0
	ldm	7			; 0b0111 = PASS
	wmp
.Lspin:
	jun	.Lspin
