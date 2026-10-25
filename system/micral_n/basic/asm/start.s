; SCELBAL runtime: the boot ROM (or a console jump) starts the program at
; address 0.
	.section .text.start,"ax",@progbits
	.globl start
start:
	cal basic_init
	jmp basic_exec

; Errors jump here from any depth, as the original's JMP EXEC does: the
; 8008's return address stack is circular, so the addresses left on it are
; simply overwritten (layout.ld RESET).
	.section .text.basic_restart,"ax",@progbits
	.globl basic_restart
basic_restart:
	jmp basic_exec

; halt(): UDF and the safety halts end with HLT, as the original does.
	.section .text.__leaf_halt,"ax",@progbits
	.globl __leaf_halt
__leaf_halt:
	hlt
	ret
