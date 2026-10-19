; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; Software-call convention (abi.md 4.4.1). A call to a compiled firmware
; function pushes a 3-nibble return token (offset lo, offset hi, page) through
; the scratch pair P0 and the SSP (P7), then JUNs the callee -- the hardware
; return stack is bypassed so call depth is unbounded. The callee (and this
; caller) return via JUN __soft_ret. A call to a leaf runtime helper (__*)
; keeps JMS/BBL.

declare i8 @deep(i8)
declare i8 @__mulqi3(i8, i8)

; CHECK-LABEL: softcaller:
; token staged in P0 only; jms __push_off (off), fim %page, jms __push_pg; jun.
; CHECK:      fim p0, %lo8(.LBB0_1)
; CHECK:      jms __push_off
; CHECK:      fim p0, %page(.LBB0_1)
; CHECK:      jms __push_pg
; CHECK:      jun deep
; CHECK: .LBB0_1:
; CHECK:      jun __soft_ret
define i8 @softcaller(i8 %x) {
  %r = call i8 @deep(i8 %x)
  ret i8 %r
}

; A leaf runtime helper keeps the hardware stack: JMS, no token push.
; CHECK-LABEL: helpercall:
; CHECK-NOT:  %lo8
; CHECK:      jms __mulqi3
define i8 @helpercall(i8 %a, i8 %b) {
  %r = call i8 @__mulqi3(i8 %a, i8 %b)
  ret i8 %r
}
