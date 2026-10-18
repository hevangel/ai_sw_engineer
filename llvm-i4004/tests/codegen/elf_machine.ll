; RUN: llc -mtriple=i4004 -filetype=obj < %s -o %t.o
; RUN: llvm-readobj --file-headers %t.o | FileCheck %s
;
; CodeGen emits an ELF object with the private i4004 machine number (abi.md
; sec.5: e_machine = 0x4004).

; CHECK: Machine: EM_I4004 (0x4004)
define i8 @f(i8 %a, i8 %b) {
  %r = add i8 %a, %b
  ret i8 %r
}
