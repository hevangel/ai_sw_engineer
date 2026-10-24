; libi8008rt: memcpy, memmove, memset (abi.md sec.6).
;   memcpy(dst BC, src DE, n)   memmove(dst BC, src DE, n)   memset(dst BC, c DE, n)
; n (16 bits) is the third argument, a memory argument at __args_<fn>+0
; (abi.md sec.3.1). LLVM ignores the returned pointer of these intrinsics;
; BC is not preserved.

	.section .text.memcpy,"ax",@progbits
	.globl memcpy
memcpy:
1:	lhi %hi(__args_memcpy)    ; n == 0: done; else n -= 1
	lli %lo(__args_memcpy)
	lam
	inl
	orm
	rtz
	dcl
	lam
	sui 1
	lma
	inl
	lam
	sbi 0
	lma
	lhd                       ; *dst++ = *src++
	lle
	lam
	lhb
	llc
	lma
	ine
	jfz 2f
	ind
2:	inc
	jfz 1b
	inb
	jmp 1b

	.section .text.memmove,"ax",@progbits
	.globl memmove
memmove:
	lab                       ; dst < src: forward copy is safe
	cpd
	jtc 3f
	jfz 4f
	lac
	cpe
	jtc 3f
	rtz                       ; dst == src
4:	lhi %hi(__args_memmove)   ; backward: dst += n, src += n
	lli %lo(__args_memmove)
	lac
	adm
	lca
	inl
	lab
	acm
	lba
	dcl
	lae
	adm
	lea
	inl
	lad
	acm
	lda
5:	lhi %hi(__args_memmove)   ; n == 0: done; else n -= 1
	lli %lo(__args_memmove)
	lam
	inl
	orm
	rtz
	dcl
	lam
	sui 1
	lma
	inl
	lam
	sbi 0
	lma
	lae                       ; --src, --dst
	sui 1
	lea
	lad
	sbi 0
	lda
	lac
	sui 1
	lca
	lab
	sbi 0
	lba
	lhd
	lle
	lam
	lhb
	llc
	lma
	jmp 5b
3:	lhi %hi(__args_memmove)   ; forward copy: hand n to memcpy
	lli %lo(__args_memmove)
	lam
	inl
	lhi %hi(__args_memcpy)
	lli %lo(__args_memcpy)
	lma
	lhi %hi(__args_memmove+1)
	lli %lo(__args_memmove+1)
	lam
	lhi %hi(__args_memcpy+1)
	lli %lo(__args_memcpy+1)
	lma
	jmp memcpy

	.section .text.memset,"ax",@progbits
	.globl memset
memset:
1:	lhi %hi(__args_memset)
	lli %lo(__args_memset)
	lam
	inl
	orm
	rtz
	dcl
	lam
	sui 1
	lma
	inl
	lam
	sbi 0
	lma
	lhb
	llc
	lme
	inc
	jfz 1b
	inb
	jmp 1b
