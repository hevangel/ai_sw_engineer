; libi4004rt -- crt0 (reset vector), flat JMS/BBL image (issue #18, C1)
; ============================================================================
; The 4004 fetches word 0 on reset (isa.md sec.6), so .reset is placed at ROM
; 0x000 by the linker script. A flat image needs almost no startup:
;
;   * CM-RAM bank 0 is selected at reset and the firmware never issues DCL
;     (llc's default; -i4004-banked-ram is the frozen banked layout, which
;     uses crt0_banked.s);
;   * there is no software return stack -- compiled calls are JMS/BBL on the
;     3-level hardware stack, and the packer proves the static call depth;
;   * all firmware state lives in 4002 registers that `main` initialises, and
;     the linker script gives `.data`/`.bss` no room, so there is nothing to
;     copy or clear.
;
; `main` is entered by JUN, so it does not occupy a hardware-stack level and
; never returns.

	.section .reset,"ax"
	.globl	_start
_start:
	jun	main
