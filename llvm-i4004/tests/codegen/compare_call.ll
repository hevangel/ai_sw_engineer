; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; Compare-and-branch (BR_CC -> SUB chain + JCN), select (CFG diamond), and a
; call (JMS). These exercise the control-flow + call lowering the firmware needs.

; icmp eq + branch: per-nibble CLC;LD;SUB;JCN.
; CHECK-LABEL: breq:
; CHECK:      clc
; CHECK:      sub
; CHECK:      jcn
define i8 @breq(i8 %a, i8 %b) {
entry:
  %c = icmp eq i8 %a, %b
  br i1 %c, label %x, label %y
x:
  ret i8 1
y:
  ret i8 0
}

; unsigned less-than branch -> SUB chain, JCN on carry.
; CHECK-LABEL: brult:
; CHECK:      sub
; CHECK:      jcn
define i8 @brult(i8 %a, i8 %b) {
entry:
  %c = icmp ult i8 %a, %b
  br i1 %c, label %x, label %y
x:
  ret i8 1
y:
  ret i8 0
}

; select -> compare + branch diamond (lowered via a PHI).
; CHECK-LABEL: sel:
; CHECK:      jcn
define i8 @sel(i8 %a, i8 %b, i8 %t, i8 %f) {
  %c = icmp ult i8 %a, %b
  %r = select i1 %c, i8 %t, i8 %f
  ret i8 %r
}

; call -> software-call sequence (abi.md 4.4.1): stage the 3-nibble return token
; in P0 and push it via the __push_off/__push_pg leaf helpers (8-bit SSP), then
; JUN the callee (NOT jms -- hardware stack bypassed).
; CHECK-LABEL: caller:
; CHECK:      fim p0, %lo8(
; CHECK:      jms __push_off
; CHECK:      fim p0, %page(
; CHECK:      jms __push_pg
; CHECK:      jun callee
declare i8 @callee(i8, i8)
define i8 @caller(i8 %a, i8 %b) {
  %r = call i8 @callee(i8 %a, i8 %b)
  ret i8 %r
}
