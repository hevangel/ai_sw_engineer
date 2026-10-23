; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; Each spill slot is addressed at its own char offset in the per-function
; frame: FIM p0, __frame_<fn>+<off>. Regression for the expander passing the
; offset as addExternalSymbol's TARGET-FLAGS argument, which put every spill,
; reload and LEA_FI at __frame_<fn>+0 so all slots aliased (an i16 select
; then returned garbage on external/chips; scripts/exec_cmp.py).
;
; Three i16 values live across a call (only P6 survives it) need three
; distinct 4-char slots.

; CHECK-LABEL: spill3:
; CHECK:     fim p0, __frame_spill3{{$}}
; CHECK:     fim p0, __frame_spill3+4
; CHECK:     fim p0, __frame_spill3+8
; CHECK:     jms g
; CHECK:     __framesize_spill3 = 12
declare void @g()
define i16 @spill3(i16 %a, i16 %b) {
  %p = xor i16 %a, %b
  call void @g()
  %s = add i16 %a, %b
  %t = add i16 %s, %p
  ret i16 %t
}

; A compare result is an i8 (getSetCCResultType): the 0/1 of an i16 equality
; needs no i16 select and no frame.
; CHECK-LABEL: eq16v:
; CHECK-NOT:  __frame_eq16v
; CHECK:      __framesize_eq16v = 0
define i8 @eq16v(i16 %x, i16 %y) {
  %c = icmp eq i16 %x, %y
  %r = zext i1 %c to i8
  ret i8 %r
}
