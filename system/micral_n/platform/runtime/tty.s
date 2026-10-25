; Micral N teletype driver (platform/io_map.md "Teletype"), shared by every
; title. abi.md calling convention: the character is in B. Both are
; `__leaf_` routines (abi.md sec.3.3): they keep C D E H L, and putc keeps B.

; __leaf_tty_getc() -> B: wait for a received character, return it with bit 7 clear
; (ASSUMPTION platform IO-2).
	.section .text.__leaf_tty_getc,"ax",@progbits
	.globl __leaf_tty_getc
__leaf_tty_getc:
	lai 1               ; status
	inp 0
	ndi 0x01            ; input ready
	jtz __leaf_tty_getc
	lai 0               ; data (reading it clears "input ready")
	inp 0
	ndi 0x7f
	lba
	ret

; __leaf_tty_putc(B): wait for "output ready", send B.
	.section .text.__leaf_tty_putc,"ax",@progbits
	.globl __leaf_tty_putc
__leaf_tty_putc:
	lai 1
	inp 0
	ndi 0x80
	jtz __leaf_tty_putc
	lab
	out 8
	ret
