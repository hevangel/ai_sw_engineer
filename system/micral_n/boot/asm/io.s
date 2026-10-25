; Micral N boot ROM: reset entry and the disk driver (../platform/io_map.md
; "Floppy disk"). The `__leaf_` routines (abi.md sec.3.3) touch only A, the
; flags and what they return.

; Reset: the CPU starts here with the boot ROM mapped at page 0.
	.section .text.start,"ax",@progbits
	.globl start
start:
	cal boot_main
	rst 0               ; this fetch switches the ROM off; run the program

; command(B): OUT 23.
	.section .text.__leaf_disk_command,"ax",@progbits
	.globl __leaf_disk_command
__leaf_disk_command:
	lab
	out 23
	ret

; step(B = dir): STEP 1 then 0 with sector 31 selected (boot hal.rs).
	.section .text.__leaf_disk_step,"ax",@progbits
	.globl __leaf_disk_step
__leaf_disk_step:
	lab
	ori 0x5f
	out 23
	xri 0x40
	out 23
	ret

; read(B = sector) -> B: select it, channel pointer 0, count 131 (the channel
; takes one more than the 130 written), start, wait for pointer = 131, pop
; the checksum (the last byte in comes out first).
	.section .text.__leaf_disk_read,"ax",@progbits
	.globl __leaf_disk_read
__leaf_disk_read:
	lab
	out 23
	lai 0
	out 20
	lai 130
	out 21
	out 22
1:	lai 0xff
	inp 0
	cpi 131
	jfz 1b
	inp 5
	lba
	ret

; status() -> B: INP 0 with A = 0xFE.
	.section .text.__leaf_disk_status,"ax",@progbits
	.globl __leaf_disk_status
__leaf_disk_status:
	lai 0xfe
	inp 0
	lba
	ret

; to_stack() -> B: move the 128 data bytes from the channel buffer to the
; stack card. Both are LIFO, so the stack card then pops them in disk order.
; B counts (declared as a return value, so the leaf convention lets it go).
	.section .text.__leaf_disk_to_stack,"ax",@progbits
	.globl __leaf_disk_to_stack
__leaf_disk_to_stack:
	lbi 128
1:	inp 5
	out 30
	dcb
	jfz 1b
	ret

; stack_pop() -> B: INP 6.
	.section .text.__leaf_stack_pop,"ax",@progbits
	.globl __leaf_stack_pop
__leaf_stack_pop:
	inp 6
	lba
	ret
