# RUN: llvm-mc -triple=i8086 -filetype=obj %s -o %t.o
# RUN: llvm-readobj -r %t.o | FileCheck %s
# RUN: llvm-readobj -h %t.o | FileCheck %s --check-prefix=HDR
#
# Relocation kinds and addends (spec/abi.md): 16-bit absolute, 8-bit absolute,
# PC-relative 8 and 16 (the addend carries the -1 / -2 field bias).
  .text
  mov ax, sym ; imm16
  mov al, sym ; imm8
  mov ax, [sym+4] ; direct address
  mov ax, [bx+sym] ; displacement
  call sym
  jmp sym
  loop sym
  .data
  .word sym
  .byte 7

# CHECK:      .rela.text {
# CHECK-NEXT:   R_I8086_16 sym 0x0
# CHECK-NEXT:   R_I8086_8 sym 0x0
# CHECK-NEXT:   R_I8086_16 sym 0x4
# CHECK-NEXT:   R_I8086_16 sym 0x0
# CHECK-NEXT:   R_I8086_PC16 sym 0xFFFFFFFE
# CHECK-NEXT:   R_I8086_PC16 sym 0xFFFFFFFE
# CHECK-NEXT:   R_I8086_PC8 sym 0xFFFFFFFF
# CHECK-NEXT: }
# CHECK:      .rela.data {
# CHECK-NEXT:   R_I8086_16 sym 0x0
# HDR: Format: elf32-i8086
# HDR: Arch: i8086
# HDR: Machine: EM_I8086 (0x8086)
