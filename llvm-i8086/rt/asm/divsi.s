; 32-bit divide, remainder, shifts and multiply for llvm-i8086.
;
; Calling convention (spec/abi.md D3): an i32 is two registers, low word first,
; taken from AX, DX, CX and then the stack. So for (a, b): a = DX:AX, b's low
; word in CX and its high word at [sp+2] at entry. The result is DX:AX. BX, SI,
; DI and BP are preserved; AX, CX, DX are free. A shift count is the byte CL.
; Dividing by zero executes the chip's own divide error (interrupt 0).
;
; Each routine has its own section so a linker can drop the ones unused.

; ---------------------------------------------------------------------------
; __i8086_udivmodsi: unsigned divide, DX:AX / BX:CX, an internal convention.
;   in:  DX:AX = dividend, BX:CX = divisor
;   out: DX:AX = quotient, BX:CX = remainder; SI, DI, BP preserved
;
; A divisor below 65536 is two 32/16 divides. Otherwise the quotient is below
; 65536; it is estimated with one divide of the normalised operands and then
; corrected by at most one (Warren, Hacker's Delight, "divide unsigned, long by
; long", with 16-bit words), so no loop runs for the quotient itself.
; ---------------------------------------------------------------------------
	.section	.text.__i8086_udivmodsi,"ax",@progbits
	.globl	__i8086_udivmodsi
	.type	__i8086_udivmodsi,@function
__i8086_udivmodsi:
	push	si
	push	di
	test	bx, bx
	jnz	.L__i8086_udivmodsi_big
	; divisor < 65536: q_hi = a_hi / b, then (r : a_lo) / b = q_lo
	mov	si, ax			; a_lo
	mov	ax, dx			; a_hi
	xor	dx, dx
	div	cx			; ax = q_hi, dx = remainder so far
	xchg	ax, si			; ax = a_lo, si = q_hi
	div	cx			; ax = q_lo, dx = remainder
	mov	cx, dx			; remainder low
	xor	bx, bx			; remainder high
	mov	dx, si			; quotient high
	pop	di
	pop	si
	ret
.L__i8086_udivmodsi_big:
	push	bp
	mov	bp, sp
	push	dx			; [bp-2] = u_hi
	push	ax			; [bp-4] = u_lo
	push	bx			; [bp-6] = v_hi
	push	cx			; [bp-8] = v_lo
	; n = leading zeros of v_hi; si = v1 = top 16 bits of v << n
	mov	si, bx
	mov	di, cx
	xor	bx, bx
.L__i8086_udivmodsi_norm:
	test	si, si
	js	.L__i8086_udivmodsi_normdone
	shl	di, 1
	rcl	si, 1
	inc	bx
	jmp	.L__i8086_udivmodsi_norm
.L__i8086_udivmodsi_normdone:
	; q1 = (u >> 1) / v1  (fits 16 bits: u >> 1 < 2^31 and v1 >= 2^15)
	mov	dx, word ptr [bp-2]
	mov	ax, word ptr [bp-4]
	shr	dx, 1
	rcr	ax, 1
	div	si			; ax = q1
	; q0 = ((q1 << n) >> 15), then made correct or one too small
	xor	dx, dx
	mov	cx, bx
	jcxz	.L__i8086_udivmodsi_noshift
.L__i8086_udivmodsi_shl:
	shl	ax, 1
	rcl	dx, 1
	loop	.L__i8086_udivmodsi_shl
.L__i8086_udivmodsi_noshift:
	shl	ax, 1
	adc	dx, dx			; dx = q0
	mov	si, dx
	test	si, si
	jz	.L__i8086_udivmodsi_q0done
	dec	si
.L__i8086_udivmodsi_q0done:
	; p = q0 * v, low 32 bits: cx:di
	mov	ax, si
	mul	word ptr [bp-8]
	mov	di, ax
	mov	cx, dx
	mov	ax, si
	mul	word ptr [bp-6]
	add	cx, ax
	; r = u - p
	mov	ax, word ptr [bp-4]
	mov	dx, word ptr [bp-2]
	sub	ax, di
	sbb	dx, cx
	; if r >= v: q0 += 1, r -= v
	cmp	dx, word ptr [bp-6]
	ja	.L__i8086_udivmodsi_correct
	jb	.L__i8086_udivmodsi_final
	cmp	ax, word ptr [bp-8]
	jb	.L__i8086_udivmodsi_final
.L__i8086_udivmodsi_correct:
	inc	si
	sub	ax, word ptr [bp-8]
	sbb	dx, word ptr [bp-6]
.L__i8086_udivmodsi_final:
	mov	cx, ax			; remainder low
	mov	bx, dx			; remainder high
	mov	ax, si			; quotient (16 bits)
	xor	dx, dx
	mov	sp, bp
	pop	bp
	pop	di
	pop	si
	ret
	.size	__i8086_udivmodsi, .-__i8086_udivmodsi

; ---------------------------------------------------------------------------
; unsigned divide and remainder
; ---------------------------------------------------------------------------
	.section	.text.__udivsi3,"ax",@progbits
	.globl	__udivsi3
	.type	__udivsi3,@function
__udivsi3:
	push	bx
	mov	bx, sp
	mov	bx, word ptr [bx+4]	; b_hi
	call	__i8086_udivmodsi
	pop	bx
	ret
	.size	__udivsi3, .-__udivsi3

	.section	.text.__umodsi3,"ax",@progbits
	.globl	__umodsi3
	.type	__umodsi3,@function
__umodsi3:
	push	bx
	mov	bx, sp
	mov	bx, word ptr [bx+4]
	call	__i8086_udivmodsi
	mov	ax, cx
	mov	dx, bx
	pop	bx
	ret
	.size	__umodsi3, .-__umodsi3

; ---------------------------------------------------------------------------
; signed divide and remainder: divide the magnitudes, then fix the signs. The
; quotient is negative when the signs differ, the remainder takes the sign of
; the dividend.
; ---------------------------------------------------------------------------
	.section	.text.__divsi3,"ax",@progbits
	.globl	__divsi3
	.type	__divsi3,@function
__divsi3:
	push	bx
	push	si
	push	di
	mov	bx, sp
	mov	bx, word ptr [bx+8]	; b_hi
	xor	di, di			; di = -1 when the result is negative
	test	dx, dx
	jns	.L__divsi3_a
	neg	dx
	neg	ax
	sbb	dx, 0
	not	di
.L__divsi3_a:
	test	bx, bx
	jns	.L__divsi3_b
	neg	bx
	neg	cx
	sbb	bx, 0
	not	di
.L__divsi3_b:
	call	__i8086_udivmodsi
	test	di, di
	jz	.L__divsi3_done
	neg	dx
	neg	ax
	sbb	dx, 0
.L__divsi3_done:
	pop	di
	pop	si
	pop	bx
	ret
	.size	__divsi3, .-__divsi3

	.section	.text.__modsi3,"ax",@progbits
	.globl	__modsi3
	.type	__modsi3,@function
__modsi3:
	push	bx
	push	si
	push	di
	mov	bx, sp
	mov	bx, word ptr [bx+8]
	xor	di, di			; di = -1 when the dividend is negative
	test	dx, dx
	jns	.L__modsi3_a
	neg	dx
	neg	ax
	sbb	dx, 0
	not	di
.L__modsi3_a:
	test	bx, bx
	jns	.L__modsi3_b
	neg	bx
	neg	cx
	sbb	bx, 0
.L__modsi3_b:
	call	__i8086_udivmodsi
	mov	ax, cx
	mov	dx, bx
	test	di, di
	jz	.L__modsi3_done
	neg	dx
	neg	ax
	sbb	dx, 0
.L__modsi3_done:
	pop	di
	pop	si
	pop	bx
	ret
	.size	__modsi3, .-__modsi3

; ---------------------------------------------------------------------------
; shifts of DX:AX by CL (0..31)
; ---------------------------------------------------------------------------
	.section	.text.__ashlsi3,"ax",@progbits
	.globl	__ashlsi3
	.type	__ashlsi3,@function
__ashlsi3:
	and	cl, 31
	cmp	cl, 16
	jb	.L__ashlsi3_small
	mov	dx, ax
	xor	ax, ax
	sub	cl, 16
.L__ashlsi3_small:
	test	cl, cl
	jz	.L__ashlsi3_done
	xor	ch, ch
.L__ashlsi3_loop:
	shl	ax, 1
	rcl	dx, 1
	loop	.L__ashlsi3_loop
.L__ashlsi3_done:
	ret
	.size	__ashlsi3, .-__ashlsi3

	.section	.text.__lshrsi3,"ax",@progbits
	.globl	__lshrsi3
	.type	__lshrsi3,@function
__lshrsi3:
	and	cl, 31
	cmp	cl, 16
	jb	.L__lshrsi3_small
	mov	ax, dx
	xor	dx, dx
	sub	cl, 16
.L__lshrsi3_small:
	test	cl, cl
	jz	.L__lshrsi3_done
	xor	ch, ch
.L__lshrsi3_loop:
	shr	dx, 1
	rcr	ax, 1
	loop	.L__lshrsi3_loop
.L__lshrsi3_done:
	ret
	.size	__lshrsi3, .-__lshrsi3

	.section	.text.__ashrsi3,"ax",@progbits
	.globl	__ashrsi3
	.type	__ashrsi3,@function
__ashrsi3:
	and	cl, 31
	cmp	cl, 16
	jb	.L__ashrsi3_small
	mov	ax, dx
	cwd
	sub	cl, 16
.L__ashrsi3_small:
	test	cl, cl
	jz	.L__ashrsi3_done
	xor	ch, ch
.L__ashrsi3_loop:
	sar	dx, 1
	rcr	ax, 1
	loop	.L__ashrsi3_loop
.L__ashrsi3_done:
	ret
	.size	__ashrsi3, .-__ashrsi3

; ---------------------------------------------------------------------------
; 32-bit product (the low 32 bits): a_lo*b_lo in full plus the cross terms in
; the high word. The compiler expands a 32-bit multiply inline; this is for
; callers that name the helper.
; ---------------------------------------------------------------------------
	.section	.text.__mulsi3,"ax",@progbits
	.globl	__mulsi3
	.type	__mulsi3,@function
__mulsi3:
	push	bx
	push	si
	mov	bx, sp
	mov	bx, word ptr [bx+6]	; b_hi
	mov	si, ax			; a_lo
	mov	ax, dx			; a_hi
	mul	cx			; ax = low word of a_hi * b_lo
	xchg	ax, bx			; ax = b_hi, bx = a_hi * b_lo
	mul	si			; ax = low word of b_hi * a_lo
	add	bx, ax			; bx = cross terms
	mov	ax, si
	mul	cx			; dx:ax = a_lo * b_lo
	add	dx, bx
	pop	si
	pop	bx
	ret
	.size	__mulsi3, .-__mulsi3
