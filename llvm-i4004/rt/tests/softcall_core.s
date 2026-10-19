; Software-return-stack calling convention — CORE PROOF (abi.md 4.4).
; Proves push-return-token / JUN-call / pop-token / JIN-return on chips, for a
; multi-level call chain that would overflow the 3-level HARDWARE stack. All
; code here is page 0 (<=256 B) so the return dispatch is a single JIN P0 to the
; return offset on page 0; the full per-page table is a later step.
;
; Return token on the P7 soft stack: ONE nibble pair = the 8-bit return OFFSET
; (page 0 only in this proof). Push = WRM offset-lo, INC, WRM offset-hi, INC.
; Call macro (inline): load return offset into P1, JMS __push_ret, JUN callee.
; Return: JUN __soft_ret -> pop offset into P0 -> JIN P0 (page 0).
;
; Test: main calls f3 (soft), f3 calls f2 (soft), f2 calls f1 (soft), f1 writes
; a marker and returns; each returns through the soft stack. If the chain
; returns correctly, RAM char0 holds the final marker. Depth 4 (> 3 hardware
; levels) proves the hardware stack is bypassed.

	.section .reset,"ax"
reset:	jun start

	.section .text,"ax"
; --- soft-call helpers (page 0) ---------------------------------------------
; __push_ret: push P1 (8-bit return offset, R2 hi:R3 lo) onto SSP (P7).
__push_ret:
	src	p7
	ld	r3
	wrm			; [SSP] = offset low nibble
	inc	r15
	src	p7
	ld	r2
	wrm			; [SSP+1] = offset high nibble
	inc	r15
	bbl	0		; returns to the CALLER of __push_ret (hardware, depth 1)

; __soft_ret: pop the return offset into P0 and JIN to it (page 0).
__soft_ret:
	ld	r15
	dac
	xch	r15		; SSP--
	src	p7
	rdm
	xch	r0		; P0 hi = offset high nibble
	ld	r15
	dac
	xch	r15		; SSP--
	src	p7
	rdm
	xch	r1		; P0 lo = offset low nibble
	jin	p0		; PC[7:0] = P0, page unchanged (page 0) -> return site

start:
	; SSP = 0x40 (soft stack base, 4002 chip 1)
	fim	p7, 0x40
	; --- call f3 (soft) ---
	fim	p1, .Lr0	; return offset (page-0 low byte)
	jms	__push_ret
	jun	f3
.Lr0:
	; f3 chain returned here. Done: spin.
halt:	jun	halt

; f3: soft-call f2, then return
f3:
	fim	p1, .Lr3
	jms	__push_ret
	jun	f2
.Lr3:
	jun	__soft_ret

; f2: soft-call f1, then return
f2:
	fim	p1, .Lr2
	jms	__push_ret
	jun	f1
.Lr2:
	jun	__soft_ret

; f1 (leaf): write marker 0x7 to RAM chip0 reg0 char0, then return
f1:
	fim	p0, 0x00
	src	p0
	ldm	7
	wrm			; RAM[0] = 7
	jun	__soft_ret
