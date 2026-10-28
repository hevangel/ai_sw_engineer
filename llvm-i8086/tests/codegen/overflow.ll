; RUN: llc -mtriple=i8086 -O2 %s -o - | FileCheck %s
; Signed multiply with overflow detection above 16 bits is a call (compiler-rt's
; __mulosi4 / __mulodi4 / __muloti4); the unsigned forms and the 16-bit forms are inline.

declare {i32,i1} @llvm.smul.with.overflow.i32(i32, i32)
declare {i64,i1} @llvm.smul.with.overflow.i64(i64, i64)
declare {i64,i1} @llvm.umul.with.overflow.i64(i64, i64)

; CHECK-LABEL: smul32:
; CHECK:      call __mulosi4
define i1 @smul32(i32 %a, i32 %b) {
  %r = call {i32,i1} @llvm.smul.with.overflow.i32(i32 %a, i32 %b)
  %o = extractvalue {i32,i1} %r, 1
  ret i1 %o
}

; CHECK-LABEL: smul64:
; CHECK:      call __mulodi4
define i1 @smul64(i64 %a, i64 %b) {
  %r = call {i64,i1} @llvm.smul.with.overflow.i64(i64 %a, i64 %b)
  %o = extractvalue {i64,i1} %r, 1
  ret i1 %o
}

; CHECK-LABEL: umul64:
; CHECK-NOT:  __mulodi4
define i1 @umul64(i64 %a, i64 %b) {
  %r = call {i64,i1} @llvm.umul.with.overflow.i64(i64 %a, i64 %b)
  %o = extractvalue {i64,i1} %r, 1
  ret i1 %o
}
