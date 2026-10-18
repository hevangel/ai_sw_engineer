; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; i8 constant materialization + return (abi.md sec.4.2: i8 returns in P1).
; LDM/FIM lowering of a Constant, and RETpseudo -> BBL 0.

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
