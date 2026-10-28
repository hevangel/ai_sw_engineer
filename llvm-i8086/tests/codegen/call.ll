; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; Calls, the frame and callee-saved registers (spec/abi.md D3, D4).

declare i16 @id(i16)
declare i16 @five(i16, i16, i16, i16, i16)

; The first three arguments are in AX, DX, CX; the rest are pushed right to
; left and removed by the caller.
; CHECK-LABEL: call_five:
; CHECK:      push ax
; CHECK-NEXT: push dx
; CHECK:      call five
; CHECK-NEXT: add sp, 4
; CHECK-NEXT: ret
define i16 @call_five(i16 %x, i16 %y) {
  %r = call i16 @five(i16 %x, i16 %y, i16 %x, i16 %y, i16 %x)
  ret i16 %r
}

; Incoming stack arguments are at [bp+4], [bp+6], ... after push bp.
; CHECK-LABEL: stack_args:
; CHECK:      push bp
; CHECK-NEXT: mov bp, sp
; CHECK:      word ptr [bp+4]
; CHECK:      word ptr [bp+6]
; CHECK:      mov sp, bp
; CHECK-NEXT: pop bp
; CHECK-NEXT: ret
define i16 @stack_args(i16 %a, i16 %b, i16 %c, i16 %d, i16 %e) {
  %s1 = add i16 %a, %d
  %s2 = add i16 %s1, %e
  %s3 = add i16 %s2, %b
  %s4 = add i16 %s3, %c
  ret i16 %s4
}

; Values live across calls sit in BX, SI, DI, which the function saves and
; restores.
; CHECK-LABEL: pressure:
; CHECK:      push bx
; CHECK:      push si
; CHECK:      push di
; CHECK:      call id
; CHECK:      pop di
; CHECK-NEXT: pop si
; CHECK-NEXT: pop bx
; CHECK-NEXT: pop bp
; CHECK-NEXT: ret
define i16 @pressure(i16 %a, i16 %b, i16 %c) {
  %x = call i16 @id(i16 %a)
  %y = call i16 @id(i16 %b)
  %z = call i16 @id(i16 %c)
  %s1 = add i16 %x, %y
  %s2 = add i16 %s1, %z
  %s3 = add i16 %s2, %a
  %s4 = add i16 %s3, %b
  %s5 = add i16 %s4, %c
  ret i16 %s5
}

; A leaf with no stack objects has no prologue at all.
; CHECK-LABEL: leaf:
; CHECK-NEXT: ; %bb.0:
; CHECK-NEXT: add ax, dx
; CHECK-NEXT: ret
define i16 @leaf(i16 %a, i16 %b) {
  %r = add i16 %a, %b
  ret i16 %r
}

; A 32-bit result is DX:AX.
; CHECK-LABEL: ret32:
; CHECK:      mov dx, 1
; CHECK-NEXT: ret
define i32 @ret32(i16 %a) {
  %r = zext i16 %a to i32
  %s = or i32 %r, 65536
  ret i32 %s
}

; CHECK-LABEL: tail:
; CHECK:      call id
; CHECK-NEXT: ret
define i16 @tail(i16 %a) {
  %r = call i16 @id(i16 %a)
  ret i16 %r
}
