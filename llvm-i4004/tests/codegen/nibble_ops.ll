; RUN: llc -mtriple=i4004 -enable-machine-outliner=never < %s | FileCheck %s
;
; (Outlining off: these pin per-function instruction patterns.)
;
; Native nibble lowering (LOGICI / SHIFTI / NIBSEL): a byte mask or a shift by
; 4 is register renaming on the 4004, and a single-bit edit happens while the
; bit sits in CY. None of these may fall back to the bit-serial __and*/__or*/
; __lshr* helpers. Executed on external/chips by scripts/exec_nibble.py.
; i8 x in P1 (r2 hi : r3 lo), y in P2 (r4:r5); i16 x in D1 (r4:r5 | r6:r7).

; CHECK-NOT: jms

; digit(): & 0x0F keeps the low register, clears the high one.
; CHECK-LABEL: digit_lo:
; CHECK:      ldm 0
; CHECK-NEXT: xch r2
; CHECK-NEXT: bbl 0
define i8 @digit_lo(i8 %x) {
  %r = and i8 %x, 15
  ret i8 %r
}

; >> 4 moves the high register down.
; CHECK-LABEL: digit_hi:
; CHECK:      ld r2
; CHECK-NEXT: xch r3
; CHECK-NEXT: ldm 0
; CHECK-NEXT: xch r2
; CHECK-NEXT: bbl 0
define i8 @digit_hi(i8 %x) {
  %r = lshr i8 %x, 4
  ret i8 %r
}

; Parity test: bit 0 via RAR ; TCC, the zero high pair via one FIM.
; CHECK-LABEL: par:
; CHECK:      ld r5
; CHECK-NEXT: rar
; CHECK-NEXT: tcc
; CHECK-NEXT: xch r5
; CHECK-NEXT: ldm 0
; CHECK-NEXT: xch r4
; CHECK-NEXT: fim p3, 0x00
define i16 @par(i16 %x) {
  %r = and i16 %x, 1
  ret i16 %r
}

; set_digit: (b & 0xF0) | (v & 0x0F) is one nibble move; the ANDs go dead.
; CHECK-LABEL: setdig:
; CHECK:      ld r5
; CHECK-NEXT: xch r3
; CHECK-NEXT: bbl 0
define i8 @setdig(i8 %x, i8 %y) {
  %a = and i8 %x, -16
  %b = and i8 %y, 15
  %r = or i8 %a, %b
  ret i8 %r
}

; Mixed mask 0x5A: the 5-step RAL ring, CLC while a cleared bit is in CY.
; CHECK-LABEL: mix:
; CHECK:      ld r3
; CHECK-NEXT: ral
; CHECK-NEXT: ral
; CHECK-NEXT: clc
; CHECK-NEXT: ral
; CHECK-NEXT: ral
; CHECK-NEXT: clc
; CHECK-NEXT: ral
; CHECK-NEXT: xch r3
define i8 @mix(i8 %x) {
  %r = and i8 %x, 90
  ret i8 %r
}

; Lone bit 3 / bit 0 edits: RAL;op;RAR and RAR;op;RAL.
; CHECK-LABEL: or8:
; CHECK:      ld r3
; CHECK-NEXT: ral
; CHECK-NEXT: stc
; CHECK-NEXT: rar
; CHECK-NEXT: xch r3
define i8 @or8(i8 %x) {
  %r = or i8 %x, 8
  ret i8 %r
}
; CHECK-LABEL: flip:
; CHECK:      ld r3
; CHECK-NEXT: rar
; CHECK-NEXT: cmc
; CHECK-NEXT: ral
; CHECK-NEXT: xch r3
define i8 @flip(i8 %x) {
  %r = xor i8 %x, 1
  ret i8 %r
}

; i16 >> 1: one CLC + RAR chain, high nibble first.
; CHECK-LABEL: half:
; CHECK:      clc
; CHECK-NEXT: ld r6
; CHECK-NEXT: rar
; CHECK-NEXT: xch r6
; CHECK-NEXT: ld r7
; CHECK-NEXT: rar
; CHECK-NEXT: xch r7
; CHECK-NEXT: ld r4
; CHECK-NEXT: rar
; CHECK-NEXT: xch r4
; CHECK-NEXT: ld r5
; CHECK-NEXT: rar
; CHECK-NEXT: xch r5
define i16 @half(i16 %x) {
  %r = lshr i16 %x, 1
  ret i16 %r
}

; ashr by width-1: sign fill (RAL ; TCC ; DAC ; CMA = 0 or 0xF).
; CHECK-LABEL: sign:
; CHECK:      ld r6
; CHECK-NEXT: ral
; CHECK-NEXT: tcc
; CHECK-NEXT: dac
; CHECK-NEXT: cma
; CHECK-NEXT: xch r5
define i16 @sign(i16 %x) {
  %r = ashr i16 %x, 15
  ret i16 %r
}

; A disjoint OR of two variables (here a masked nibble and a 0/1 flag) is an
; ADD: no __orqi3 (I4004ISelLowering LowerBitwise).
; CHECK-LABEL: ordisj:
; CHECK:      add
define i8 @ordisj(i8 %x, i8 %y) {
  %h = and i8 %x, 12
  %f = and i8 %y, 1
  %r = or i8 %h, %f
  ret i8 %r
}

; A multiply by 3 is a shift and an add, not __mulqi3 (decomposeMulByConstant).
; CHECK-LABEL: mul3:
; CHECK:      add
define i8 @mul3(i8 %x) {
  %r = mul i8 %x, 3
  ret i8 %r
}
