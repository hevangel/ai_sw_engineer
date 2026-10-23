; RUN: llc -mtriple=i4004 < %s | FileCheck %s
; RUN: llc -mtriple=i4004 -i4004-softcall < %s | FileCheck %s --check-prefix=LEGACY
;
; MachineOutliner (issue #18): a straight-line sequence repeated across
; functions becomes one body, called with JMS and ending in BBL 0. Bodies hold
; no call, so each JMS adds one hardware-stack level; a sequence that ends in
; the return is entered with JUN instead (a tail call: no stack level). Here the same three
; status-flag writes (FIM; SRC; LDM; WR3) close both functions.
; LEGACY-NOT: OUTLINED_FUNCTION

declare void @__ram_ws3(i8, i8)

; CHECK-LABEL: a:
; CHECK:       jun OUTLINED_FUNCTION_0
define void @a(i8 %v) {
  call void @__ram_ws3(i8 99, i8 %v)
  call void @__ram_ws3(i8 3, i8 0)
  call void @__ram_ws3(i8 19, i8 0)
  call void @__ram_ws3(i8 51, i8 0)
  ret void
}

; CHECK-LABEL: b:
; CHECK:       jun OUTLINED_FUNCTION_0
define void @b(i8 %v) {
  call void @__ram_ws3(i8 115, i8 %v)
  call void @__ram_ws3(i8 3, i8 0)
  call void @__ram_ws3(i8 19, i8 0)
  call void @__ram_ws3(i8 51, i8 0)
  ret void
}

; CHECK-LABEL: OUTLINED_FUNCTION_0:
; CHECK:       fim p1, 0x03
; CHECK:       wr3
; CHECK:       fim p1, 0x13
; CHECK:       wr3
; CHECK:       bbl 0
