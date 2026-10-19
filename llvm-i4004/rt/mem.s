; libi4004rt -- memory helpers over 4002 RAM main characters
; ============================================================================
; The 4004 reaches data RAM only through an SRC pointer into {chip,reg,char}
; (isa.md sec.5, abi.md sec.1.3). A 16-bit AS-0 pointer's low byte IS the SRC
; address byte  [chip:2][reg:2][char:4]  (abi.md sec.1.3); the high byte selects
; the CM-RAM bank via DCL and is 0 for the Busicom board (one bank). These
; helpers therefore take the 8-bit SRC address of each operand in a single pair
; and a byte count, and operate on CONSECUTIVE RAM main characters (NIBBLES),
; wrapping within a 4002 register's 16 characters exactly as SRC+INC do on the
; hardware -- callers that need to cross a register/chip boundary split the call.
;
; NOTE on units: these operate on 4002 *characters* = nibbles, because that is
; the addressable RAM unit. A C `memcpy(n)` over i8 byte-cells copies 2*n
; nibbles; the firmware's own working registers are nibble arrays (bcd::Num),
; so a nibble-granular copy/set is what the yardstick consumer actually needs.
; Count is the number of NIBBLES.
;
; Contracts (abi.md args P1,P2,P3; leaf, no stack):
;   __memset_ram(u8 dst_src=P1, u8 val=P2 low nibble, u8 count=P3):
;        RAM[dst .. dst+count) <- val ; returns dst in P1.
;   __memcpy_ram(u8 dst_src=P1, u8 src_src=P2, u8 count=P3):
;        copy count nibbles src->dst (dst and src are SRC address bytes).
;        Overlap: forward copy; use __memmove_ram if regions overlap with dst>src.
;   __memmove_ram(... same ...): chooses direction by comparing dst,src.
;
; Register use: R2:R3=P1(dst), R4:R5=P2(src/val), R6:R7=P3(count), R0/R1 scratch.
; count is treated as the low byte (R7); R6 (count hi) lets a caller pass >15.

; ---------------------------------------------------------------------------
; __memset_ram : fill count nibbles at dst with val.lo.
; ---------------------------------------------------------------------------
	.section .text.__memset_ram,"ax"
	.globl	__memset_ram
__memset_ram:
	; if count (R6:R7) == 0 return
	ld	r7
	jcn	nz, .Lset_go
	ld	r6
	jcn	z, .Lset_done
.Lset_go:
	; working SRC address pointer kept in P0 (R0:R1) = dst (R2:R3) copy
	ld	r2
	xch	r0
	ld	r3
	xch	r1			; P0 = dst address
.Lset_loop:
	src	p0			; address = P0
	ld	r5
	wrm				; RAM[P0] <- val.lo
	; P0.lo++ (advance character within the register)
	inc	r1
	; count-- : decrement R6:R7 (16-bit). Use: if R7!=0 R7--, else R7=15,R6--.
	ld	r7
	jcn	z, .Lset_borrow
	ld	r7
	dac				; R7 - 1
	xch	r7
	jun	.Lset_test
.Lset_borrow:
	ldm	15
	xch	r7
	ld	r6
	dac
	xch	r6
.Lset_test:
	; done when R6:R7 == 0
	ld	r7
	jcn	nz, .Lset_loop
	ld	r6
	jcn	nz, .Lset_loop
.Lset_done:
	bbl	0

; ---------------------------------------------------------------------------
; __memcpy_ram : copy count nibbles src->dst, forward.
;   dst addr stays in P1 (R2:R3), src addr in P2 (R4:R5), count in P3 (R6:R7).
;   SRC addresses live in pairs already, so the inner step is
;     SRC P2 ; RDM (ACC<-src) ; SRC P1 ; WRM (dst<-ACC)
;   with no ACC clobber between read and write. Then advance both low nibbles
;   and decrement the 16-bit count. R0/R1 unused here.
; ---------------------------------------------------------------------------
	.section .text.__memcpy_ram,"ax"
	.globl	__memcpy_ram
__memcpy_ram:
	ld	r7
	jcn	nz, .Lcpy_loop
	ld	r6
	jcn	z, .Lcpy_done
.Lcpy_loop:
	src	p2			; address = src
	rdm				; ACC = RAM[src]
	src	p1			; address = dst (ACC preserved)
	wrm				; RAM[dst] = ACC
	inc	r5			; src.lo++
	inc	r3			; dst.lo++
	; count-- (16-bit R6:R7)
	ld	r7
	jcn	z, .Lcpy_borrow
	ld	r7
	dac
	xch	r7
	jun	.Lcpy_test
.Lcpy_borrow:
	ldm	15
	xch	r7
	ld	r6
	dac
	xch	r6
.Lcpy_test:
	ld	r7
	jcn	nz, .Lcpy_loop
	ld	r6
	jcn	nz, .Lcpy_loop
.Lcpy_done:
	bbl	0

; ---------------------------------------------------------------------------
; __memmove_ram : like memcpy but safe for overlap. If dst > src, copy
;   backward (from the high end down) so a forward-overlapping move is correct;
;   otherwise copy forward. Comparison is on the 8-bit SRC address (same chip
;   assumed). Leaf; uses P1=dst, P2=src, P3=count, R0/R1 scratch.
; ---------------------------------------------------------------------------
	.section .text.__memmove_ram,"ax"
	.globl	__memmove_ram
__memmove_ram:
	ld	r7
	jcn	nz, .Lmv_go
	ld	r6
	jcn	z, .Lmv_done
.Lmv_go:
	; compare dst (R2:R3) vs src (R4:R5): compute dst - src, CY=1 => dst>=src.
	clc
	ld	r3
	sub	r5
	cmc
	ld	r2
	sub	r4			; CY=1 => dst >= src
	jcn	nc, .Lmv_fwd		; dst < src -> forward copy is safe
	; dst >= src: if equal, nothing to do; copy BACKWARD.
	; advance dst and src to the last nibble: addr += (count-1).
	; count-1 low nibble into R0 (count assumed < 16 for the backward path;
	; for larger counts callers use non-overlapping regions). Compute end offsets.
	ld	r7
	dac				; count-1 (low nibble; backward path limited to <16)
	xch	r0			; R0 = count-1
	clc
	ld	r3
	add	r0
	xch	r3			; dst.lo += count-1
	clc
	ld	r5
	add	r0
	xch	r5			; src.lo += count-1
.Lmv_bwd:
	src	p2
	rdm
	src	p1
	wrm
	ld	r5
	dac
	xch	r5			; src.lo--
	ld	r3
	dac
	xch	r3			; dst.lo--
	ld	r7
	dac
	xch	r7			; count-- (low byte; backward path count<16)
	ld	r7
	jcn	nz, .Lmv_bwd
	bbl	0
.Lmv_fwd:
	; forward copy (same as memcpy inner loop)
	src	p2
	rdm
	src	p1
	wrm
	inc	r5
	inc	r3
	ld	r7
	jcn	z, .Lmv_fborrow
	ld	r7
	dac
	xch	r7
	jun	.Lmv_ftest
.Lmv_fborrow:
	ldm	15
	xch	r7
	ld	r6
	dac
	xch	r6
.Lmv_ftest:
	ld	r7
	jcn	nz, .Lmv_fwd
	ld	r6
	jcn	nz, .Lmv_fwd
.Lmv_done:
	bbl	0
