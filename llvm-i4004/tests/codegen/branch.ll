; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; Unconditional branch lowers to JUN to the target block (abi.md sec.5: JUN
; reaches the whole 12-bit ROM space; page-range checking is a linker concern).

; CHECK-LABEL: spin:
; CHECK: jun .LBB0_1
define void @spin() {
  br label %l
l:
  br label %l
}
