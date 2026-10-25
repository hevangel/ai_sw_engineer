; RUN: llc -mtriple=i8008 -O2 %s -o - | FileCheck %s
; Calling convention (abi.md sec.3): B C D E, then __args_<callee>; the
; frame and argument-area sizes are published for the linker.

declare i8 @ext(i8, i8, i8, i8, i8, i8)

; CHECK-LABEL: callmem:
; CHECK: lhi %hi(__args_ext+1)
; CHECK-NEXT: lli %lo(__args_ext+1)
; CHECK-NEXT: lmi 0x05
; CHECK-NEXT: dcl
; CHECK-NEXT: lmi 0x04
; CHECK: cal ext
; CHECK: __framesize_callmem = 1
; CHECK: __argsize_callmem = 0
define i8 @callmem(i8 %a) {
  %r = call i8 @ext(i8 %a, i8 1, i8 2, i8 3, i8 4, i8 5)
  %s = add i8 %r, %a
  ret i8 %s
}

; A callee reads its memory arguments from its own area.
; CHECK-LABEL: five:
; CHECK: lhi %hi(__args_five)
; CHECK-NEXT: lli %lo(__args_five)
; CHECK: __argsize_five = 1
define i8 @five(i8 %a, i8 %b, i8 %c, i8 %d, i8 %e) {
  ret i8 %e
}

; i16 arguments take whole pairs: BC, then DE.
; CHECK-LABEL: pairs:
; CHECK-NEXT: ; %bb.0:
; CHECK-NEXT: lbe
; CHECK-NEXT: ret
define i8 @pairs(i16 %x, i16 %y) {
  %t = trunc i16 %y to i8
  ret i8 %t
}
