; RUN: not llvm-mc -triple=i4004 %s 2>&1 | FileCheck %s
;
; Out-of-range immediates are rejected (asm_syntax.md: imm4 0-15, imm8 0-255).

        ldm     16
; CHECK: error: invalid operand for instruction

        bbl     16
; CHECK: error: invalid operand for instruction

        fim     p0, 256
; CHECK: error: invalid operand for instruction
