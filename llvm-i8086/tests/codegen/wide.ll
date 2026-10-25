; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; 32-bit values are pairs of 16-bit registers (spec/codegen.md, Arithmetic).

; Low words in AX and CX, high words in DX and on the stack.
; CHECK-LABEL: and32:
; CHECK:      and ax, cx
; CHECK:      and dx,
; CHECK:      ret
define i32 @and32(i32 %a, i32 %b) {
  %r = and i32 %a, %b
  ret i32 %r
}

; CHECK-LABEL: hi:
; CHECK-NEXT: ; %bb.0:
; CHECK-NEXT: mov ax, dx
; CHECK-NEXT: ret
define i16 @hi(i32 %a) {
  %s = lshr i32 %a, 16
  %r = trunc i32 %s to i16
  ret i16 %r
}

; CHECK-LABEL: ult32:
; CHECK:      cmp
; CHECK:      ret
define i16 @ult32(i32 %a, i32 %b) {
  %c = icmp ult i32 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}

; Add: there is no add-with-carry in the selector yet, so the carry is a
; compare.
; CHECK-LABEL: add32:
; CHECK:      add {{[a-d]x}}, {{[a-d]x}}
; CHECK:      cmp
; CHECK-NEXT: jb
; CHECK:      add dx, word ptr [bp+4]
; CHECK:      ret
define i32 @add32(i32 %a, i32 %b) {
  %r = add i32 %a, %b
  ret i32 %r
}
