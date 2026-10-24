; MIC 01 monitor: entry points, holds and `go` (spec/spec.md).
; The Rust core is monitor_main(hold: u8) in src/main.rs.

; Cold entry 0x3B30 (layout.ld PLACE): zero the monitor's RAM, then warm.
	.section .text.mon_cold,"ax",@progbits
	.globl mon_cold
mon_cold:
	jmp mon_cold_body

; Warm entry 0x3B49: keep the holds and saved registers.
	.section .text.mon_warm,"ax",@progbits
	.globl mon_warm
mon_warm:
	lbi 0
	jmp monitor_main

	.section .text.mon_cold_body,"ax",@progbits
mon_cold_body:
	lhi %hi(__data_start)
	lli %lo(__data_start)
1:	lmi 0
	inl
	jfz 2f
	inh
2:	lah
	cpi %hi(__frames_end)
	jfz 1b
	lal
	cpi %lo(__frames_end)
	jfz 1b
	jmp mon_warm

; Holds. The program reaches one through `CAL mon_holdN` patched over its
; code. Every register is the program's, so the stack card (platform
; io_map.md) keeps A, H, L and the flags while H:L is freed for the save area.
	.section .text.mon_hold0,"ax",@progbits
	.globl mon_hold0
mon_hold0:
	out 30              ; push A
	lai 1               ; monitor_main(hold = 1)
	jmp mon_hold

	.section .text.mon_hold1,"ax",@progbits
	.globl mon_hold1
mon_hold1:
	out 30
	lai 2

	.section .text.mon_hold,"ax",@progbits
mon_hold:
	out 30              ; push the hold number
	lah
	out 30              ; push H
	lal
	out 30              ; push L
	lai 0               ; push the flags, one byte each (loads and OUT
	jfc 1f              ; leave the flags alone)
	lai 1
1:	out 30
	lai 0
	jfz 1f
	lai 2
1:	out 30
	lai 0
	jfs 1f
	lai 4
1:	out 30
	lai 0
	jfp 1f
	lai 8
1:	out 30
	lhi %hi(mon_regs)   ; mon_regs is one page-bounded object
	lli %lo(mon_regs+1)
	lmb
	lli %lo(mon_regs+2)
	lmc
	lli %lo(mon_regs+3)
	lmd
	lli %lo(mon_regs+4)
	lme
	inp 6               ; flags: P S Z C, add them up
	lba
	inp 6
	adb
	lba
	inp 6
	adb
	lba
	inp 6
	adb
	lli %lo(mon_regs+7)
	lma
	inp 6
	lli %lo(mon_regs+6)
	lma                 ; L
	inp 6
	lli %lo(mon_regs+5)
	lma                 ; H
	inp 6
	lba                 ; hold number
	inp 6
	lli %lo(mon_regs)
	lma                 ; A
	jmp monitor_main

; mon_go(addr BC): run the program at addr with the saved registers. The
; jump is a RAM trampoline `LHI h; LLI l; JMP addr`, so H and L are loaded
; last. Carry and zero come from one `ADA` (ASSUMPTION spec MON-11); after
; it only loads run, which keep the flags.
	.section .text.mon_go,"ax",@progbits
	.globl mon_go
mon_go:
	lhi %hi(mon_tramp)
	lli %lo(mon_tramp)
	lmi 0x2e            ; LHI
	lli %lo(mon_tramp+2)
	lmi 0x36            ; LLI
	lli %lo(mon_tramp+4)
	lmi 0x44            ; JMP
	lli %lo(mon_tramp+5)
	lmc
	lli %lo(mon_tramp+6)
	lmb
	lhi %hi(mon_regs)
	lli %lo(mon_regs+5)
	lbm                 ; user H
	lli %lo(mon_regs+6)
	lcm                 ; user L
	lhi %hi(mon_tramp)
	lli %lo(mon_tramp+1)
	lmb
	lli %lo(mon_tramp+3)
	lmc
	lhi %hi(mon_regs)
	lli %lo(mon_regs+7)
	lam                 ; B = (zero ? 0 : 1) | (carry ? 0x80 : 0)
	ndi 2
	lbi 1
	jtz 1f
	lbi 0
1:	lam
	ndi 1
	jtz 2f
	lab
	ori 0x80
	lba
2:	lab
	ada                 ; sets carry and zero
	lli %lo(mon_regs+1)
	lbm
	lli %lo(mon_regs+2)
	lcm
	lli %lo(mon_regs+3)
	ldm
	lli %lo(mon_regs+4)
	lem
	lli %lo(mon_regs)
	lam
	jmp mon_tramp

	.section .bss.mon_regs,"aw",@nobits
	.globl mon_regs
mon_regs:
	.zero 8
	.section .bss.mon_tramp,"aw",@nobits
mon_tramp:
	.zero 7
