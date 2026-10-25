; MIC 01 monitor: entry points, holds and `go` (spec/spec.md).
; The Rust core is monitor_main(hold: u8) in src/main.rs.

; Cold entry 0x3B30 (layout.ld PLACE): zero the monitor's RAM, then warm.
; The RAM is one page (layout.ld DATAPAGE), so H stays; the loop fits in
; the 25 bytes before the warm entry.
	.section .text.mon_cold,"ax",@progbits
	.globl mon_cold
mon_cold:
	lhi %hi(__data_start)
	lli %lo(__data_start)
1:	lmi 0
	inl
	lal
	cpi %lo(__frames_end)
	jfz 1b
	jmp mon_warm

; Warm entry 0x3B49: keep the holds and saved registers.
	.section .text.mon_warm,"ax",@progbits
	.globl mon_warm
mon_warm:
	lbi 0
	jmp monitor_main

; A cancelled command (`?`) jumps here from any depth: back to the command
; loop as the warm entry does (layout.ld RESET; the 8008's stack is
; circular, so the return addresses left behind are overwritten).
	.section .text.mon_restart,"ax",@progbits
	.globl mon_restart
mon_restart:
	lbi 0
	jmp monitor_main

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
	lai 0               ; push the flags as 0/1, carry first (loads and OUT
	jfc 1f              ; leave the flags alone)
	lai 1
1:	out 30
	lai 0
	jfz 1f
	lai 1
1:	out 30
	lai 0
	jfs 1f
	lai 1
1:	out 30
	lai 0
	jfp 1f
	lai 1
1:	out 30
	lhi %hi(mon_regs)   ; mon_regs is one page-bounded object:
	lli %lo(mon_regs+5) ; C Z S P A B C D E H L
	lmb
	inl
	lmc
	inl
	lmd
	inl
	lme
	lli %lo(mon_regs+3) ; the flags, popped parity first
	inp 6
	lma
	dcl
	inp 6
	lma
	dcl
	inp 6
	lma
	dcl
	inp 6
	lma
	lli %lo(mon_regs+10)
	inp 6
	lma                 ; L
	dcl
	inp 6
	lma                 ; H
	inp 6
	lba                 ; hold number
	lli %lo(mon_regs+4)
	inp 6
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
	inl
	lmc
	inl
	lmb
	lhi %hi(mon_regs)
	lli %lo(mon_regs+9)
	lbm                 ; user H
	inl
	lcm                 ; user L
	lhi %hi(mon_tramp)
	lli %lo(mon_tramp+1)
	lmb
	lli %lo(mon_tramp+3)
	lmc
	lhi %hi(mon_regs)   ; B = (zero ? 0 : 1) | (carry ? 0x80 : 0)
	lli %lo(mon_regs+1)
	lam
	xri 1
	lba
	dcl
	lam
	rrc                 ; carry bit to bit 7
	ndi 0x80
	orb
	ada                 ; sets carry and zero
	lli %lo(mon_regs+5)
	lbm
	inl
	lcm
	inl
	ldm
	inl
	lem
	lli %lo(mon_regs+4)
	lam
	jmp mon_tramp

	.section .bss.mon_regs,"aw",@nobits
	.globl mon_regs
mon_regs:
	.zero 11
	.section .bss.mon_tramp,"aw",@nobits
mon_tramp:
	.zero 7

; The holds (spec "G"). Slot n (0, 1) is 5 bytes of mon_holds: the hold's
; address (low, high) and the three program bytes the patched CAL covers.
; mon_nhold says how many slots are patched in. All of it is RAM that the
; cold entry zeroes and a warm entry keeps. Normal calling convention
; (llvm-i8008 abi.md sec.3): these clobber every register.

; mon_patch(n B, a DE): keep the 3 bytes at a in slot n, write
; `CAL mon_holdN` over them, and count slots 0..n as patched. The bytes to
; write wait on the stack card, last first.
	.section .text.mon_patch,"ax",@progbits
	.globl mon_patch
mon_patch:
	lab
	ora
	lai %hi(mon_hold0)
	jtz 1f
	lai %hi(mon_hold1)
1:	out 30
	lab
	ora
	lai %lo(mon_hold0)
	jtz 1f
	lai %lo(mon_hold1)
1:	out 30
	lai 0x46            ; CAL
	out 30
	lhi %hi(mon_nhold)  ; n + 1 slots patched
	lli %lo(mon_nhold)
	lab
	adi 1
	lma
	lhi %hi(mon_holds)
	lli %lo(mon_holds)
	sui 1
	jtz 1f
	lli %lo(mon_holds+5)
1:	lme                 ; slot address
	inl
	lmd
	inl
	lcl                 ; C -> the saved bytes (one page-bounded object)
	lbi 3
2:	lhd                 ; keep the program's byte
	lle
	lam
	lhi %hi(mon_holds)
	llc
	lma
	inc
	lhd                 ; write the patch byte
	lle
	inp 6
	lma
	ine
	jfz 3f
	ind
3:	dcb
	jfz 2b
	ret

; mon_clear(): put the program's bytes back under every patched slot.
	.section .text.mon_clear,"ax",@progbits
	.globl mon_clear
mon_clear:
	lhi %hi(mon_nhold)
	lli %lo(mon_nhold)
	lam
	lmi 0
	ora
	rtz
	lba                 ; B = slots to restore (1 or 2)
	lhi %hi(mon_holds)
	lli %lo(mon_holds)
	cal mon_restore
	dcb
	rtz
	lhi %hi(mon_holds)
	lli %lo(mon_holds+5)
	jmp mon_restore

; Restore the slot at H:L (keeps B). Its own section: the linker's call
; graph is per section.
	.section .text.mon_restore,"ax",@progbits
mon_restore:
	lem                 ; DE = the hold's address
	inl
	ldm
	inl
	lcl                 ; C -> saved bytes
	lai 3
1:	out 30              ; the count waits on the stack card
	lhi %hi(mon_holds)
	llc
	lam
	lhd
	lle
	lma
	inc
	ine
	jfz 2f
	ind
2:	inp 6
	sui 1
	jfz 1b
	ret

	.section .bss.mon_holds,"aw",@nobits
	.globl mon_holds
mon_holds:
	.zero 10
	.section .bss.mon_nhold,"aw",@nobits
	.globl mon_nhold
mon_nhold:
	.zero 1
