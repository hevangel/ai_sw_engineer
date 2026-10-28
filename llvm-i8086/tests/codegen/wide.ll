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

; Add with carry: the carry stays in the flags between the pieces.
; CHECK-LABEL: add32:
; CHECK:      add ax, cx
; CHECK-NEXT: adc dx, word ptr [bp+4]
; CHECK:      ret
define i32 @add32(i32 %a, i32 %b) {
  %r = add i32 %a, %b
  ret i32 %r
}

; CHECK-LABEL: sub32:
; CHECK:      sub ax, cx
; CHECK-NEXT: sbb dx, word ptr [bp+4]
; CHECK:      ret
define i32 @sub32(i32 %a, i32 %b) {
  %r = sub i32 %a, %b
  ret i32 %r
}

; A 64-bit add is one add and three adc; the result goes through a hidden
; pointer in AX.
; CHECK-LABEL: add64:
; CHECK:      add dx, word ptr [bp+8]
; CHECK-NEXT: adc cx, word ptr [bp+0xa]
; CHECK-NEXT: adc {{[a-d]x|si|di}}, word ptr [bp+0xc]
; CHECK-NEXT: adc {{[a-d]x|si|di}}, word ptr [bp+0xe]
define i64 @add64(i64 %a, i64 %b) {
  %r = add i64 %a, %b
  ret i64 %r
}

; The carry out of an add, as a value: a branch makes the boolean.
; CHECK-LABEL: carry:
; CHECK:      mov al, 1
; CHECK:      add
; CHECK-NEXT: jb
define zeroext i1 @carry(i16 %a, i16 %b) {
  %p = call { i16, i1 } @llvm.uadd.with.overflow.i16(i16 %a, i16 %b)
  %c = extractvalue { i16, i1 } %p, 1
  ret i1 %c
}

declare { i16, i1 } @llvm.uadd.with.overflow.i16(i16, i16)
