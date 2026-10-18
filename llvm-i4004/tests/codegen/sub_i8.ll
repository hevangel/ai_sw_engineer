; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; i8 subtract. The 4004 SUB uses an inverted borrow, so a correct multi-nibble
; subtract sets carry (STC = no borrow) before the low nibble, then SUBs each
; nibble low-first (abi.md sec.7 open-question 1 / isa.md).

; CHECK-LABEL: sub8:
; CHECK:      stc
; CHECK-NEXT: ld r3
; CHECK-NEXT: sub r5
; CHECK-NEXT: xch r3
; CHECK-NEXT: ld r2
; CHECK-NEXT: sub r4
; CHECK-NEXT: xch r2
; CHECK-NEXT: bbl 0
define i8 @sub8(i8 %a, i8 %b) {
  %r = sub i8 %a, %b
  ret i8 %r
}
