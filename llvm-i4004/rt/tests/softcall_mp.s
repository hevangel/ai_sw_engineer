; Software-return-stack convention — MULTI-PAGE return dispatch proof.
; Generalises softcall_core.s to return to an arbitrary 12-bit address.
;
; Return token on the P7 soft stack (3 nibbles, pushed low-addr first):
;   [SSP+0] offset low   [SSP+1] offset high   [SSP+2] page
; Call (inline): FIM P1 = return offset (IMM8 of .Lret), FIM a page nibble into
;   R4, JMS __push_ret, JUN callee.
; Return: JUN __soft_ret.
;
; __soft_ret reaches the return PAGE via a 16-entry table of `JUN page_stub[k]`
; living in __soft_ret's page; it JIN's (page-local) to table entry 2*page, that
; JUN's to the return page's stub, a one-byte `JIN P0` at a fixed offset on EVERY
; page, which lands at the return OFFSET (held in P0).
;
; Layout is pinned with .org so the per-page JIN-P0 stub sits at the SAME offset
; (0xF0) on page 0 and page 1 — this is what the packer will reserve per page.

	.section .reset,"ax"
reset:	jun start

	.section .text,"ax"
; ===== PAGE 0 =====================================================
	.org 0x000
start:
	fim	p7, 0x40		; SSP base
	; call f_p1 (lives on page 1); return to .Lr0 on page 0
	fim	p1, .Lr0		; P1 = return offset (low byte, page 0)
	fim	p2, 0x00		; R4 = return page = 0
	jms	__push_ret
	jun	f_p1
.Lr0:
	jun	halt

; __push_ret: push R3(off lo), R2(off hi), R4(page); SSP += 3.
	.org 0x020
__push_ret:
	src	p7
	ld	r3
	wrm
	inc	r15
	src	p7
	ld	r2
	wrm
	inc	r15
	src	p7
	ld	r4
	wrm
	inc	r15
	bbl	0

; __soft_ret: pop page+offset, dispatch via page table + per-page JIN stub.
	.org 0x040
__soft_ret:
	; SSP -= 1 ; read page into R8
	ld	r15
	dac
	xch	r15
	src	p7
	rdm
	xch	r8			; R8 = page
	; SSP -= 1 ; read offset high into R0
	ld	r15
	dac
	xch	r15
	src	p7
	rdm
	xch	r0			; P0 hi = offset high
	; SSP -= 1 ; read offset low into R1
	ld	r15
	dac
	xch	r15
	src	p7
	rdm
	xch	r1			; P0 lo = offset low  (P0 = return offset)
	; Compute P2 (R4:R5) = 0x80 + page*2, the byte offset of .Ltab entry[page].
	; .Ltab is at 0x80 (low nibble 0). Two-nibble add of (page*2) to 0x80:
	ld	r8			; ACC = page (0..15)
	clc
	ral				; ACC = low nibble of page*2 ; CY = carry (bit4)
	xch	r5			; R5 = low nibble
	ldm	8			; high nibble base = 8
	xch	r4
	ld	r4			; ACC = 8
	; add the carry from page*2 (if page>=8) to the high nibble:
	jcn	nc, .Lnocar
	iac				; ACC = 9
	xch	r4
.Lnocar:
	jin	p2			; JIN to .Ltab[page] (page-local, page 0)

	.org 0x080
.Ltab:
	jun	.Lstub_p0		; entry 0 -> page-0 stub
	jun	.Lstub_p1		; entry 1 -> page-1 stub

halt:	jun	halt

; per-page JIN-P0 stub at fixed offset 0xF0 on PAGE 0
	.org 0x0F0
.Lstub_p0:
	jin	p0

; ===== PAGE 1 =====================================================
	.org 0x100
f_p1:
	; leaf on page 1: write marker 0x5 to RAM[0], then return
	fim	p0, 0x00
	src	p0
	ldm	5
	wrm
	jun	__soft_ret

; per-page JIN-P0 stub at fixed offset 0xF0 on PAGE 1
	.org 0x1F0
.Lstub_p1:
	jin	p0
