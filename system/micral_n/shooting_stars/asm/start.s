; Shooting Stars runtime: the boot ROM (or a console jump) starts the
; program at address 0.
	.section .text.start,"ax",@progbits
	.globl start
start:
	cal game_main
	; fall through: the game returns only after halting

; halt(): the original ends with HLT.
	.section .text.__leaf_halt,"ax",@progbits
	.globl __leaf_halt
__leaf_halt:
	hlt
	ret
