; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; addrspace-0 (4002 RAM) i8 load/store. The pointer is an i16 in a DR16
; double-pair (AVR DREGS analog); its low sub-pair is the SRC address byte.
; A byte is two adjacent 4002 cells (low nibble first, abi.md sec.1.2): SRC,
; RDM/WRM, advance the char (INC), SRC, RDM/WRM, restore the char (DAC).

; CHECK-LABEL: load8:
; CHECK:      src p2
; CHECK-NEXT: rdm
; CHECK:      inc r5
; CHECK:      src p2
; CHECK-NEXT: rdm
; CHECK:      jun __soft_ret
define i8 @load8(ptr %p) {
  %v = load i8, ptr %p
  ret i8 %v
}

; CHECK-LABEL: store8:
; CHECK:      src
; CHECK:      wrm
; CHECK:      inc
; CHECK:      wrm
; CHECK:      jun __soft_ret
define void @store8(ptr %p, i8 %v) {
  store i8 %v, ptr %p
  ret void
}
