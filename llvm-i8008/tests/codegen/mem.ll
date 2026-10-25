; RUN: llc -mtriple=i8008 -O2 %s -o - | FileCheck %s
; Memory goes through H:L (codegen.md sec.2, sec.5; abi.md sec.4).

@g = global i8 0
@w = global i16 0

; CHECK-LABEL: ld:
; CHECK: lhb
; CHECK-NEXT: llc
; CHECK-NEXT: lbm
define i8 @ld(ptr %p) {
  %v = load i8, ptr %p
  ret i8 %v
}

; A pointer may point anywhere, so the i16 step carries into H.
; CHECK-LABEL: st16:
; CHECK: lhb
; CHECK-NEXT: llc
; CHECK-NEXT: lme
; CHECK-NEXT: inl
; CHECK-NEXT: jfz [[L:.Ltmp[0-9]+]]
; CHECK-NEXT: inh
; CHECK-NEXT: [[L]]:
; CHECK-NEXT: lmd
define void @st16(ptr %p, i16 %v) {
  store i16 %v, ptr %p
  ret void
}

; A small global never crosses a page: INL alone.
; CHECK-LABEL: incw:
; CHECK: lhi %hi(w)
; CHECK-NEXT: lli %lo(w)
; CHECK-NEXT: lcm
; CHECK-NEXT: inl
; CHECK-NEXT: lbm
define void @incw() {
  %x = load i16, ptr @w
  %y = add i16 %x, 1
  store i16 %y, ptr @w
  ret void
}

; Constant store, symbolic address.
; CHECK-LABEL: stg:
; CHECK: lhi %hi(g)
; CHECK-NEXT: lli %lo(g)
; CHECK-NEXT: lmi 0x2a
define void @stg() {
  store i8 42, ptr @g
  ret void
}

; Address of a global: two LdI with %lo / %hi.
; CHECK-LABEL: addr:
; CHECK: lci %lo(w)
; CHECK-NEXT: lbi %hi(w)
define ptr @addr() {
  ret ptr @w
}

; memset's value is an i8 argument: after the pointer (BC) it takes D,
; which rt/mem.s reads (abi.md sec.6).
; CHECK-LABEL: fill5:
; CHECK: ldi 0x00
; CHECK: cal memset
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
define void @fill5(ptr %p) {
  call void @llvm.memset.p0.i16(ptr %p, i8 0, i16 5, i1 false)
  ret void
}

; A 256-aligned table: &t[i] is H = the page, L = i, no 16-bit add
; (computeKnownBitsForTargetNode, codegen.md sec.2).
@t = global [256 x i8] zeroinitializer, align 256
; CHECK-LABEL: ldt:
; CHECK-NOT: adb
; CHECK-NOT: aci
; CHECK: lbm
define i8 @ldt(i8 %i) {
  %x = zext i8 %i to i16
  %a = getelementptr inbounds i8, ptr @t, i16 %x
  %v = load i8, ptr %a
  ret i8 %v
}
