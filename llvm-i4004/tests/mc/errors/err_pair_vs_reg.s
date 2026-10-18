; RUN: not llvm-mc -triple=i4004 %s 2>&1 | FileCheck %s
;
; A single-register instruction rejects a pair operand and vice versa
; (asm_syntax.md: FIM/SRC/FIN/JIN take a pair; INC/ISZ/ADD/SUB/LD/XCH take Rn).

; SRC takes a pair, not a single register.
        src     r3
; CHECK: error: invalid operand for instruction

; ADD takes a single register, not a pair.
        add     p2
; CHECK: error: invalid operand for instruction

; FIN takes a pair, not a single register.
        fin     r5
; CHECK: error: invalid operand for instruction
