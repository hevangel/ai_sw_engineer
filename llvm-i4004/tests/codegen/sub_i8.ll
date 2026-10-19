; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; i8 subtract. 4004 SUB computes {CY,ACC} <- ACC + ~R + ~CY (isa.md sec.7).
; CLC once, subtract the low nibble, then CMC to flip the carry-out's polarity
; so the high nibble's ~CY injects the correct borrow. Verified 8/8 on the real
; external/chips CPU (scripts/exec_fn.py) -- an STC-once form was wrong on every
; case, caught by the execution test.

; CHECK-LABEL: sub8:
; CHECK:      clc
; CHECK-NEXT: ld r3
; CHECK-NEXT: sub r5
; CHECK-NEXT: xch r3
; CHECK-NEXT: cmc
; CHECK-NEXT: ld r2
; CHECK-NEXT: sub r4
; CHECK-NEXT: xch r2
; CHECK-NEXT: jun __soft_ret
define i8 @sub8(i8 %a, i8 %b) {
  %r = sub i8 %a, %b
  ret i8 %r
}
