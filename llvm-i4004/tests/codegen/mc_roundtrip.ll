; RUN: llc -mtriple=i4004 < %s -o %t.s
; RUN: llvm-mc -triple=i4004 -filetype=obj %t.s -o %t.o
; RUN: llvm-readobj --file-headers %t.o | FileCheck %s
;
; The whole point of the backbone: llc's assembly output is valid input to our
; own independent llvm-mc assembler (closes the CodeGen -> MC contract).

; CHECK: Machine: EM_I4004 (0x4004)
define i8 @roundtrip(i8 %a, i8 %b) {
  %s = add i8 %a, %b
  %r = sub i8 %s, %b
  ret i8 %r
}
