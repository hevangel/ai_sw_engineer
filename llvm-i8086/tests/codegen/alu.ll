; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; Register ALU operations and immediates (spec/codegen.md, Arithmetic).

; CHECK-LABEL: add16:
; CHECK:      add ax, dx
; CHECK-NEXT: ret
define i16 @add16(i16 %a, i16 %b) {
  %r = add i16 %a, %b
  ret i16 %r
}

; CHECK-LABEL: sub16:
; CHECK:      sub ax, dx
; CHECK-NEXT: ret
define i16 @sub16(i16 %a, i16 %b) {
  %r = sub i16 %a, %b
  ret i16 %r
}

; CHECK-LABEL: logic16:
; CHECK:      and ax, dx
; CHECK-NEXT: or ax, cx
; CHECK-NEXT: xor ax, 0x1234
; CHECK-NEXT: ret
define i16 @logic16(i16 %a, i16 %b, i16 %c) {
  %x = and i16 %a, %b
  %y = or i16 %x, %c
  %z = xor i16 %y, 4660
  ret i16 %z
}

; A one-byte increment, and the sign-extended byte form for a small constant.
; CHECK-LABEL: inc16:
; CHECK:      inc ax
; CHECK-NEXT: ret
define i16 @inc16(i16 %a) {
  %r = add i16 %a, 1
  ret i16 %r
}

; CHECK-LABEL: small_imm:
; CHECK:      add ax, -5
; CHECK-NEXT: ret
define i16 @small_imm(i16 %a) {
  %r = sub i16 %a, 5
  ret i16 %r
}

; CHECK-LABEL: neg16:
; CHECK:      neg ax
; CHECK-NEXT: ret
define i16 @neg16(i16 %a) {
  %n = sub i16 0, %a
  ret i16 %n
}

; CHECK-LABEL: not16:
; CHECK:      not ax
; CHECK-NEXT: ret
define i16 @not16(i16 %a) {
  %n = xor i16 %a, -1
  ret i16 %n
}

; Bytes stay in byte registers.
; CHECK-LABEL: add8:
; CHECK:      add al, dl
; CHECK-NEXT: mov ah, 0
; CHECK-NEXT: ret
define zeroext i8 @add8(i8 zeroext %a, i8 zeroext %b) {
  %r = add i8 %a, %b
  ret i8 %r
}

; The accumulator forms: add al, imm and add ax, imm16.
; CHECK-LABEL: acc_forms:
; CHECK:      add al, 7
; CHECK:      add ax, 0x3e8
define i16 @acc_forms(i8 %a, i16 %b) {
  %x = add i8 %a, 7
  %y = add i16 %b, 1000
  %z = zext i8 %x to i16
  %r = add i16 %z, %y
  ret i16 %r
}

; Shifts: by one, by two (two shifts), by a larger count (in CL).
; CHECK-LABEL: shl1:
; CHECK:      shl ax, 1
; CHECK-NEXT: ret
define i16 @shl1(i16 %a) {
  %r = shl i16 %a, 1
  ret i16 %r
}

; CHECK-LABEL: shl2:
; CHECK:      shl ax, 1
; CHECK-NEXT: shl ax, 1
; CHECK-NEXT: ret
define i16 @shl2(i16 %a) {
  %r = shl i16 %a, 2
  ret i16 %r
}

; CHECK-LABEL: sar5:
; CHECK:      mov cl, 5
; CHECK-NEXT: sar ax, cl
; CHECK-NEXT: ret
define i16 @sar5(i16 %a) {
  %r = ashr i16 %a, 5
  ret i16 %r
}

; A variable count goes through CL.
; CHECK-LABEL: shl_var:
; CHECK:      mov cx, dx
; CHECK-NEXT: and cl, 0xf
; CHECK:      shl ax, cl
; CHECK-NEXT: ret
define i16 @shl_var(i16 %a, i16 %n) {
  %m = and i16 %n, 15
  %r = shl i16 %a, %m
  ret i16 %r
}

; A shift by eight moves a byte.
; CHECK-LABEL: shr8:
; CHECK:      mov al, ah
; CHECK-NEXT: mov ah, 0
; CHECK-NEXT: ret
define i16 @shr8(i16 %a) {
  %r = lshr i16 %a, 8
  ret i16 %r
}

; i8 -> i16: zero-extension clears the high byte, sign-extension is cbw.
; CHECK-LABEL: zext8:
; CHECK:      mov ah, 0
; CHECK-NEXT: ret
define i16 @zext8(i8 %a) {
  %r = zext i8 %a to i16
  ret i16 %r
}

; CHECK-LABEL: sext8:
; CHECK:      cbw
; CHECK-NEXT: ret
define i16 @sext8(i8 %a) {
  %r = sext i8 %a to i16
  ret i16 %r
}
