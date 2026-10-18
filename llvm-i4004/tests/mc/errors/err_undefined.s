; RUN: not llvm-mc -triple=i4004 %s 2>&1 | FileCheck %s
;
; The two undefined OPR=0xF opcodes (0xFE/0xFF, isa.md sec.8) have no mnemonic;
; the assembler must not provide a way to emit them. A bogus mnemonic errors.

        skl
; CHECK: error: invalid instruction mnemonic

        wpx
; CHECK: error: invalid instruction mnemonic
