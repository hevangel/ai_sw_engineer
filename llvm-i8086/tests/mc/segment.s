# RUN: llvm-mc -triple=i8086 -filetype=obj %s -o %t.o
# RUN: llvm-readobj -r %t.o | FileCheck %s
# RUN: llvm-mc -triple=i8086 -show-encoding %s | FileCheck %s --check-prefix=ENC
#
# `sym@seg` is the segment the linker puts the symbol in (R_I8086_SEG16, spec/abi.md D8):
# an immediate, the segment half of a far call, and a data word.
  .text
  mov dx, f@seg
  mov ax, f
  call f@seg:f
  .data
  .word g@seg

# ENC: mov dx, f@seg
# ENC: mov ax, f
# ENC: call f@seg:f
# CHECK:      .rela.text {
# CHECK-NEXT:   R_I8086_SEG16 f 0x0
# CHECK-NEXT:   R_I8086_16 f 0x0
# CHECK-NEXT:   R_I8086_16 f 0x0
# CHECK-NEXT:   R_I8086_SEG16 f 0x0
# CHECK-NEXT: }
# CHECK:      .rela.data {
# CHECK-NEXT:   R_I8086_SEG16 g 0x0
