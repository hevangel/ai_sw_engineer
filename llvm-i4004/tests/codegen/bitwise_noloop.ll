; RUN: llc -mtriple=i4004 < %s | FileCheck %s
;
; Regression: i16 AND/OR/XOR and i8/i16 shifts must lower to libi4004rt calls
; WITHOUT llc entering an infinite legalize/combine cycle. An earlier WIP
; (commit 7f191be) lowered i16 bitwise ops by splitting them into two i8
; __*qi3 calls built with TRUNCATE / SRL(i16) / SHL(i16) / ZERO_EXTEND nodes;
; those re-entered the i16 shift custom-lowering and legalization looped
; forever (llc spun at 100% CPU and never returned). The fix lowers each i16
; bitwise op directly to its hi3 helper (no self-referential sub-nodes). This
; test simply has to TERMINATE and emit the call; the lit timeout turns any
; recurrence of the loop back into a visible failure.
;
; abi.md sec.6: bitwise/shift helpers are __{and,or,xor}{qi,hi}3 and
; __{ashl,lshr,ashr}{qi,hi}3, two zero-extended args, result in the ABI return
; register(s).

; CHECK-LABEL: and16:
; CHECK: jms __andhi3
define i16 @and16(i16 %a, i16 %b) {
  %r = and i16 %a, %b
  ret i16 %r
}

; CHECK-LABEL: or16:
; CHECK: jms __orhi3
define i16 @or16(i16 %a, i16 %b) {
  %r = or i16 %a, %b
  ret i16 %r
}

; CHECK-LABEL: xor16:
; CHECK: jms __xorhi3
define i16 @xor16(i16 %a, i16 %b) {
  %r = xor i16 %a, %b
  ret i16 %r
}

; CHECK-LABEL: and8:
; CHECK: jms __andqi3
define i8 @and8(i8 %a, i8 %b) {
  %r = and i8 %a, %b
  ret i8 %r
}

; CHECK-LABEL: shl16:
; CHECK: jms __ashlhi3
define i16 @shl16(i16 %x, i16 %n) {
  %r = shl i16 %x, %n
  ret i16 %r
}

; CHECK-LABEL: lshr16:
; CHECK: jms __lshrhi3
define i16 @lshr16(i16 %x, i16 %n) {
  %r = lshr i16 %x, %n
  ret i16 %r
}

; CHECK-LABEL: lshr8:
; CHECK: jms __lshrqi3
define i8 @lshr8(i8 %x, i8 %n) {
  %r = lshr i8 %x, %n
  ret i8 %r
}
