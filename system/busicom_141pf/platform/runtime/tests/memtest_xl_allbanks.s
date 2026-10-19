; busicom_141pf xl board -- FULL 4-bank added-memory self-test (RTL only)
; ============================================================================
; Like memtest_xl.s, but sweeps ALL FOUR CM-RAM banks (0..3) x 4 chips x 4 regs
; x 16 chars = 1024 main characters = the entire 16 x 4002 main memory of the xl
; board. Each bank is selected with DCL before its sweep, proving the DCL bank
; decode and every added 4002 across banks 1..3 (which the stock board and the
; external `chips` model cannot represent -- see platform/README.md). RUN ON THE
; RTL (primary) simulator only; on chips it would FAIL because banks 1..3 do not
; exist there.
;
; Verdict on the RAM1 lamp port (sampled as "LAMPS|n", io_map.md):
;   LAMPS|7 (0b0111) = PASS (all 1024 cells across 4 banks verified)
;   LAMPS|1 (0b0001) = FAIL (>=1 mismatch)
;
; DCL note: the RAM1 lamp port we write the verdict to lives on BANK 0, so the
; verdict sequence re-selects bank 0 (DCL 0) before driving the port.
;
; Register usage:
;   P0 (R0:R1) SRC pointer   R4 FAIL flag   R6 scratch (read value)
;   R7 bank 0..3   R8 chip 0..3   R9 reg 0..3   R10 char 0..15
	.text
	.globl	main
main:
	ldm	0
	xch	r4			; FAIL = 0

	ldm	0
	xch	r7			; bank = 0
.Lbank:
	ld	r7
	dcl				; select CM-RAM bank = R7 (ACC[2:0])

	ldm	0
	xch	r8			; chip = 0
.Lchip:
	ldm	0
	xch	r9			; reg = 0
.Lreg:
	ldm	0
	xch	r10			; char = 0
.Lwr:
	ld	r8
	clc
	ral
	clc
	ral				; chip<<2
	add	r9			; + reg -> SRC hi nibble
	xch	r0
	ld	r10
	xch	r1			; char
	src	p0
	; pattern = (char+chip+reg+bank) & 0xF  (bank included so each bank differs)
	ld	r10
	add	r8
	add	r9
	add	r7
	wrm
	isz	r10, .Lwr

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
	rdm
	xch	r6			; read value
	ld	r10
	add	r8
	add	r9
	add	r7			; expected
	clc
	sub	r6			; expected - read
	jcn	z, .Lrd_ok
	ldm	1
	xch	r4			; FAIL
.Lrd_ok:
	isz	r10, .Lrd

	ld	r9
	iac
	xch	r9
	ldm	4
	clc
	sub	r9
	jcn	z, .Lchip_next
	jun	.Lreg
.Lchip_next:
	ld	r8
	iac
	xch	r8
	ldm	4
	clc
	sub	r8
	jcn	z, .Lbank_next
	jun	.Lchip
.Lbank_next:
	ld	r7
	iac
	xch	r7
	ldm	4
	clc
	sub	r7
	jcn	z, .Lverdict
	jun	.Lbank

.Lverdict:
	ldm	0
	dcl				; verdict port (RAM1) is on bank 0
	ld	r4
	jcn	z, .Lpass
	fim	p0, 0x40		; RAM1 chip select (SRC [7:6]=01)
	src	p0
	ldm	1
	wmp
	jun	.Lspin
.Lpass:
	fim	p0, 0x40
	src	p0
	ldm	7
	wmp
.Lspin:
	jun	.Lspin
