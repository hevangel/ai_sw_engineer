; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; i8 constant materialization + return (abi.md sec.4.2: i8 returns in P1).
; LDM/FIM lowering of a Constant, and RETpseudo -> JUN __soft_ret (a compiled
; function is entered via the software-call convention, abi.md 4.4.1).

; CHECK-LABEL: ret42:
; CHECK: fim p1, 0x2a
; CHECK: jun __soft_ret
define i8 @ret42() {
  ret i8 42
}

; CHECK-LABEL: ret0:
; CHECK: fim p1, 0x0
; CHECK: jun __soft_ret
define i8 @ret0() {
  ret i8 0
}

; CHECK-LABEL: retvoid:
; CHECK: jun __soft_ret
define void @retvoid() {
  ret void
}
