; llvm-mos6502 bare-metal startup (issue hevangel/ai_engineer#58).
;
; Owns the reset entry for the qualification image so the bare
; `mos-unknown-none` target needs none of the SDK's per-platform crt
; machinery (the machine-agnostic `common` platform's INCLUDE chain does not
; parse with the image's lld 20; see report/qualification.md). Sequence:
;
;   1. software stack pointer S = 0xFF (page 1, grows down — llvm-mos ABI),
;   2. zero .bss and zp_bss (__bss_start/__bss_end, __zp_bss_start/__zp_bss_end
;      from spec/link.ld),
;   3. copy .data and zp_data initializers from ROM
;      (__data_lma -> __data_start.., __zp_data_lma -> zp_data..),
;   4. jsr main (the Rust entry), then park with interrupts disabled.
;
; Assembled by llvm-mc (-triple mos, filetype=obj) at build time; scratch
; uses __rc0..__rc3, which crt0 owns before main runs (llvm-mos imaginary
; registers, spec/link.ld).
;
; NMI/IRQ default to a local trap loop; a program may override by defining
; its own __nmi_vector / __irq_vector (spec/link.ld checks DEFINED()).

	.section .text.start,"ax",@progbits
	.globl	_start
_start:
	ldx	#0xff
	txs				; S = 0xFF, page-1 stack
	cld				; NMOS reset leaves D indeterminate; binary mode

	; --- zero .bss (16-bit range in __rc0/__rc1..__rc2/__rc3) ---
	lda	#<__bss_start
	sta	__rc0
	lda	#>__bss_start
	sta	__rc1
	lda	#<__bss_end
	sta	__rc2
	lda	#>__bss_end
	sta	__rc3
	lda	__rc3
	cmp	__rc1
	bcc	zero_bss_done		; end < start: empty
	bne	zero_bss_full
	lda	__rc2
	cmp	__rc0
	bcc	zero_bss_done
zero_bss_full:
	lda	#0x00
	tay
zero_bss_loop:
	sta	(__rc0),y
	inc	__rc0
	bne	zero_bss_next
	inc	__rc1
zero_bss_next:
	lda	__rc0
	cmp	__rc2
	lda	__rc1
	sbc	__rc3
	bcc	zero_bss_loop		; while ptr < end
zero_bss_done:

	; --- zero zp_bss (byte range, < 256 bytes by construction) ---
	lda	#<__zp_bss_start
	sta	__rc0
	lda	#<__zp_bss_end
	sta	__rc2
	lda	__rc2
	cmp	__rc0
	bcc	zp_zero_done
	lda	#0x00
	tay
zp_zero_loop:
	sta	(__rc0),y
	iny
	cpy	__rc2
	bne	zp_zero_loop		; note: start may be > 0; ptr base handles it
zp_zero_done:

	; --- copy .data initializers: ROM __data_lma -> RAM __data_start ---
	lda	#<__data_lma
	sta	__rc0
	lda	#>__data_lma
	sta	__rc1
	lda	#<__data_start
	sta	__rc2
	lda	#>__data_start
	sta	__rc3
	jsr	copy_range

	; --- copy zp_data initializers: ROM __zp_data_lma -> __zp_data_start ---
	lda	#<__zp_data_lma
	sta	__rc0
	lda	#>__zp_data_lma
	sta	__rc1
	lda	#<__zp_data_start
	sta	__rc2
	lda	#>__zp_data_start
	sta	__rc3
	jsr	copy_range

	jsr	main
park:
	jmp	park

; copy_range: copies (__rc3,__rc2 - __rc1,__rc0) bytes from (__rc1,__rc0)
; to (__rc3,__rc2), byte-wise, ascending.
copy_range:
	lda	__rc3
	cmp	__rc1
	bcc	copy_done		; end < start: empty
	bne	copy_go
	lda	__rc2
	cmp	__rc0
	bcc	copy_done
copy_go:
	ldy	#0x00
copy_loop:
	lda	(__rc0),y
	sta	(__rc2),y
	inc	__rc0
	bne	copy_inc_dst
	inc	__rc1
copy_inc_dst:
	inc	__rc2
	bne	copy_check
	inc	__rc3
copy_check:
	lda	__rc3
	cmp	__rc1
	bcc	copy_done
	bne	copy_loop
	lda	__rc2
	cmp	__rc0
	bcc	copy_done
	bne	copy_loop		; while src ptr != end
copy_done:
	rts

	.section .text.trap,"ax",@progbits
	.globl	__trap
__trap:
	jmp	__trap
