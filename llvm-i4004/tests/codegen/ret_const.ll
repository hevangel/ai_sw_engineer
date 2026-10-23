; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; i8 constant materialization + return (abi.md sec.4.2: i8 returns in P1).
; LDM/FIM lowering of a Constant, and RETpseudo -> JUN __soft_ret (a compiled
; function is entered via the software-call convention, abi.md 4.4.1).

; CHECK-LABEL: ret42:
; CHECK: fim p1, 0x2a
; CHECK: bbl 0
define i8 @ret42() {
  ret i8 42
}

; CHECK-LABEL: ret0:
; CHECK: fim p1, 0x0
; CHECK: bbl 0
define i8 @ret0() {
  ret i8 0
}

; CHECK-LABEL: retvoid:
; CHECK: bbl 0
define void @retvoid() {
  ret void
}

; A zeroext i1 return widens to i8 and comes back in P1, where the caller's
; RetCC reads it (it used to widen to i16 and land in D1 = P2:P3).
; CHECK-LABEL: retbool:
; CHECK-NOT: fim p3
; CHECK: fim p1, 0x01
; CHECK-NEXT: bbl 0
define zeroext i1 @retbool() {
  ret i1 true
}
