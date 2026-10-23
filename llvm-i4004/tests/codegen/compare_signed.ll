; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; Signed relations (abi.md sec.1.2: i8/i16 are two's complement). The BRCC
; expansion compares (lhs ^ 0x80..) with (rhs ^ 0x80..) unsigned: the top
; nibble of each operand has bit 3 flipped by `RAL ; CMC ; RAR`, which keeps
; the borrow chained in from the lower nibbles in CY. rhs's top nibble is
; flipped in place and restored after the SUB. Executed over the full sign
; matrix on external/chips by scripts/exec_cmp.py.

; i8 slt: x in P1 (r2:r3), y in P2 (r4:r5).
; CHECK-LABEL: slt8:
; CHECK:      clc
; CHECK-NEXT: ld r3
; CHECK-NEXT: sub r5
; CHECK-NEXT: cmc
; CHECK-NEXT: ld r4
; CHECK-NEXT: ral
; CHECK-NEXT: cmc
; CHECK-NEXT: rar
; CHECK-NEXT: xch r4
; CHECK-NEXT: ld r2
; CHECK-NEXT: ral
; CHECK-NEXT: cmc
; CHECK-NEXT: rar
; CHECK-NEXT: sub r4
; CHECK-NEXT: ld r4
; CHECK-NEXT: ral
; CHECK-NEXT: cmc
; CHECK-NEXT: rar
; CHECK-NEXT: xch r4
; CHECK-NEXT: jcn {{0x[2a]}},
define i8 @slt8(i8 %x, i8 %y) {
  %c = icmp slt i8 %x, %y
  br i1 %c, label %t, label %f
t:
  ret i8 1
f:
  ret i8 0
}

; i16 sgt -> operands swapped to slt: the flip lands on the TOP nibble only
; (x's r6 is rhs here), after three plain SUB/CMC steps.
; CHECK-LABEL: sgt16:
; CHECK:      clc
; CHECK-NEXT: ld r9
; CHECK-NEXT: sub r5
; CHECK-NEXT: cmc
; CHECK-NEXT: ld r8
; CHECK-NEXT: sub r4
; CHECK-NEXT: cmc
; CHECK-NEXT: ld r11
; CHECK-NEXT: sub r7
; CHECK-NEXT: cmc
; CHECK-NEXT: ld r6
; CHECK-NEXT: ral
; CHECK-NEXT: cmc
; CHECK-NEXT: rar
; CHECK-NEXT: xch r6
; CHECK-NEXT: ld r10
; CHECK-NEXT: ral
; CHECK-NEXT: cmc
; CHECK-NEXT: rar
; CHECK-NEXT: sub r6
; CHECK:      xch r6
; CHECK-NEXT: jcn {{0x[2a]}},
define i8 @sgt16(i16 %x, i16 %y) {
  %c = icmp sgt i16 %x, %y
  br i1 %c, label %t, label %f
t:
  ret i8 1
f:
  ret i8 0
}

; Unsigned relations keep the plain chain (no sign flip).
; CHECK-LABEL: ult8:
; CHECK:      clc
; CHECK-NEXT: ld r3
; CHECK-NEXT: sub r5
; CHECK-NEXT: cmc
; CHECK-NEXT: ld r2
; CHECK-NEXT: sub r4
; CHECK-NEXT: jcn {{0x[2a]}},
define i8 @ult8(i8 %x, i8 %y) {
  %c = icmp ult i8 %x, %y
  br i1 %c, label %t, label %f
t:
  ret i8 1
f:
  ret i8 0
}
