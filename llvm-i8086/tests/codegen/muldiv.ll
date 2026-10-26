; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; Native multiply and divide on AX and DX (spec/codegen.md, Arithmetic).

; CHECK-LABEL: mul16:
; CHECK:      mul dx
; CHECK-NEXT: ret
define i16 @mul16(i16 %a, i16 %b) {
  %r = mul i16 %a, %b
  ret i16 %r
}

; The high half of an unsigned product is DX.
; CHECK-LABEL: umulhi:
; CHECK:      mul dx
; CHECK-NEXT: mov ax, dx
; CHECK-NEXT: ret
define i16 @umulhi(i16 %a, i16 %b) {
  %x = zext i16 %a to i32
  %y = zext i16 %b to i32
  %p = mul i32 %x, %y
  %s = lshr i32 %p, 16
  %r = trunc i32 %s to i16
  ret i16 %r
}

; CHECK-LABEL: smulhi:
; CHECK:      imul dx
; CHECK-NEXT: mov ax, dx
define i16 @smulhi(i16 %a, i16 %b) {
  %x = sext i16 %a to i32
  %y = sext i16 %b to i32
  %p = mul i32 %x, %y
  %s = lshr i32 %p, 16
  %r = trunc i32 %s to i16
  ret i16 %r
}

; An unsigned divide clears DX; a signed one uses cwd.
; CHECK-LABEL: udiv16:
; CHECK:      mov dx, 0
; CHECK-NEXT: div cx
; CHECK-NEXT: ret
define i16 @udiv16(i16 %a, i16 %b) {
  %r = udiv i16 %a, %b
  ret i16 %r
}

; CHECK-LABEL: sdiv16:
; CHECK:      cwd
; CHECK-NEXT: idiv cx
; CHECK-NEXT: ret
define i16 @sdiv16(i16 %a, i16 %b) {
  %r = sdiv i16 %a, %b
  ret i16 %r
}

; The remainder is DX.
; CHECK-LABEL: urem16:
; CHECK:      div cx
; CHECK-NEXT: mov ax, dx
; CHECK-NEXT: ret
define i16 @urem16(i16 %a, i16 %b) {
  %r = urem i16 %a, %b
  ret i16 %r
}

; One divide gives both the quotient and the remainder.
; CHECK-LABEL: divmod:
; CHECK:      div cx
; CHECK-NOT:  div{{[ 	]}}
; CHECK:      ret
define i16 @divmod(i16 %a, i16 %b) {
  %q = udiv i16 %a, %b
  %m = urem i16 %a, %b
  %t = mul i16 %m, 3
  %r = add i16 %q, %t
  ret i16 %r
}

; A divide by a power of two is a shift.
; CHECK-LABEL: udiv8c:
; CHECK:      shr ax, cl
; CHECK-NOT:  div{{[ 	]}}
define i16 @udiv8c(i16 %a) {
  %r = udiv i16 %a, 8
  ret i16 %r
}

; A 32-bit product from 16-bit full products: three multiplies, no call.
; CHECK-LABEL: mul32:
; CHECK-NOT:  call
; CHECK:      mul
; CHECK:      mul
; CHECK:      mul
define i32 @mul32(i32 %a, i32 %b) {
  %r = mul i32 %a, %b
  ret i32 %r
}

; A 32-bit divide is still a runtime call.
; CHECK-LABEL: udiv32:
; CHECK:      call __udivsi3
define i32 @udiv32(i32 %a, i32 %b) {
  %r = udiv i32 %a, %b
  ret i32 %r
}
