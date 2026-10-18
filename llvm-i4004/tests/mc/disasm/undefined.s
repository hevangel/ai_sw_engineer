# RUN: llvm-mc -triple=i4004 --disassemble %s 2>&1 | FileCheck %s
#
# Undefined accumulator opcodes 0xFE and 0xFF (spec/isa.md section 8) have no
# defined function; the disassembler must NOT invent a mnemonic for them. It
# reports an invalid-encoding warning and emits no instruction (a real image
# packer renders them as .byte). 0x01 (an OPR=0x0 low-nibble != 0) is likewise
# not a valid opcode and must be rejected the same way.

0xfe
# CHECK: warning: invalid instruction encoding
0xff
# CHECK: warning: invalid instruction encoding
0x01
# CHECK: warning: invalid instruction encoding
