; Smoke-test runtime: start at 0, call main, halt. putc writes the teletype.
	.section .text.__start,"ax",@progbits
	.globl __start
__start:
	cal main
	hlt

	.section .text.putc,"ax",@progbits
	.globl putc
putc:
	lab
	out 8
	ret
