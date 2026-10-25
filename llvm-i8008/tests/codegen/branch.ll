; RUN: llc -mtriple=i8008 -O2 %s -o - | FileCheck %s
; Compares: borrow for unsigned, Z for equality, sign-flip for signed
; (codegen.md sec.2).

; CHECK-LABEL: umax:
; CHECK: lac
; CHECK-NEXT: cpb
; CHECK-NEXT: jtc
define i8 @umax(i8 %a, i8 %b) {
  %c = icmp ugt i8 %a, %b
  %r = select i1 %c, i8 %a, i8 %b
  ret i8 %r
}

; CHECK-LABEL: smin16:
; CHECK: lad
; CHECK-NEXT: xri 0x80
; CHECK-NEXT: lha
; CHECK-NEXT: lab
; CHECK-NEXT: xri 0x80
; CHECK-NEXT: lla
; CHECK-NEXT: lac
; CHECK-NEXT: sue
; CHECK-NEXT: lal
; CHECK-NEXT: sbh
; CHECK-NEXT: jtc
define i16 @smin16(i16 %a, i16 %b) {
  %c = icmp slt i16 %a, %b
  %r = select i1 %c, i16 %a, i16 %b
  ret i16 %r
}

; x == 0 on 16 bits as a value: OR the halves, then the carry of A - 1
; is the result (SETCC16ri, no branch).
; CHECK-LABEL: iszero:
; CHECK: lac
; CHECK-NEXT: orb
; CHECK-NEXT: sui 0x01
; CHECK-NEXT: lai 0x00
; CHECK-NEXT: aci 0x00
define i8 @iszero(i16 %x) {
  %c = icmp eq i16 %x, 0
  %r = zext i1 %c to i8
  ret i8 %r
}

; x < 0 tests the sign bit.
; CHECK-LABEL: isneg:
; CHECK: lac
; CHECK-NEXT: ora
; CHECK-NEXT: jts
define i8 @isneg(i8 %x) {
  %c = icmp slt i8 %x, 0
  %r = select i1 %c, i8 7, i8 9
  ret i8 %r
}

; x <= 9 unsigned is x < 10.
; CHECK-LABEL: le9:
; CHECK: cpi 0x0a
; CHECK-NEXT: jtc
define i8 @le9(i8 %x) {
  %c = icmp ule i8 %x, 9
  %r = select i1 %c, i8 1, i8 2
  ret i8 %r
}

; a < b unsigned as a value: the borrow of a - b (SETCC8rr).
; CHECK-LABEL: ult8:
; CHECK: lab
; CHECK-NEXT: suc
; CHECK-NEXT: lai 0x00
; CHECK-NEXT: aci 0x00
define i8 @ult8(i8 %a, i8 %b) {
  %c = icmp ult i8 %a, %b
  %r = zext i1 %c to i8
  ret i8 %r
}

; a != b on 16 bits: XOR the halves into A; carry of A + 0xFF is nonzero.
; CHECK-LABEL: ne16:
; CHECK: lac
; CHECK-NEXT: xre
; CHECK-NEXT: lla
; CHECK-NEXT: lab
; CHECK-NEXT: xrd
; CHECK-NEXT: orl
; CHECK-NEXT: adi 0xff
; CHECK-NEXT: lai 0x00
; CHECK-NEXT: aci 0x00
define i8 @ne16(i16 %a, i16 %b) {
  %c = icmp ne i16 %a, %b
  %r = zext i1 %c to i8
  ret i8 %r
}
