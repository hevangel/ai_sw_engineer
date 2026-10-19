; libi4004rt -- BCD helper wrappers
; ============================================================================
; The yardstick firmware is a BCD calculator (firmware/src/bcd.rs); its
; digit-serial code maps to the 4004's single-instruction decimal ops DAA, KBP,
; TCS, DAC (isa.md sec.3). These wrappers expose those ops as CALLABLE helpers.
;
; !! ABI refinement (important): isa.md sec.1 says BBL loads ACC <- DDDD on
; return, so a helper CANNOT return an i4 result in ACC -- the return
; instruction itself overwrites it. abi.md sec.4.2 names ACC as the i4 return
; register, but that only holds for code that returns by falling through / JUN,
; not for a JMS/BBL callee. We therefore define the runtime i4 return register
; as P1 low nibble (R3): each wrapper leaves its result there and does BBL 0.
; The backend that INLINES these single instructions keeps the result in ACC as
; abi.md sec.4.2 says; only the out-of-line callable form uses R3. Preferred use
; is inline emission (one instruction vs a 5-instruction call), so these exist
; mainly for completeness and for a path the selector cannot match.
;
; Arg i4 arrives in P1.lo (R3). CY is passed straight through (not an ABI reg).

; __bcd_adjust : DAA -- decimal-adjust R3 using incoming CY; result -> R3.
	.section .text.__bcd_adjust,"ax"
	.globl	__bcd_adjust
__bcd_adjust:
	ld	r3
	daa
	xch	r3			; result -> R3 (BBL would clobber ACC)
	bbl	0

; __key_decode : KBP -- one-hot keyboard nibble -> binary (0/1/2/4/8 -> 0/1/2/3/4,
; else 0xF). Arg in R3, result -> R3.
	.section .text.__key_decode,"ax"
	.globl	__key_decode
__key_decode:
	ld	r3
	kbp
	xch	r3
	bbl	0

; __tens_comp : TCS -- ACC <- 9 + CY (CY=0->9, CY=1->10), CY<-0. The arg is the
; incoming CY only; result (9 or 10) -> R3. (10 does not fit a nibble: TCS
; yields 0xA, the ten's-complement step the firmware's sub_mag uses.)
	.section .text.__tens_comp,"ax"
	.globl	__tens_comp
__tens_comp:
	tcs
	xch	r3
	bbl	0

; __dec : DAC -- decrement: ACC <- R3 - 1 (ACC + 0xF); CY=0 => borrow. Result
; -> R3, and CY carries the borrow out (passed through BBL untouched).
	.section .text.__dec,"ax"
	.globl	__dec
__dec:
	ld	r3
	dac
	xch	r3
	bbl	0
