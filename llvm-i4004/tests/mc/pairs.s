; RUN: llvm-mc -triple=i4004 -show-encoding %s | FileCheck %s
;
; All 16 register-pair forms (spec/isa.md sec.1): FIM/FIN use RRR0, SRC/JIN use
; RRR1. byte0 = OPR:(RRR<<1 | b).

; SRC Pn : 0010 RRR1
        src     p0
; CHECK: src p0 {{.*}}encoding: [0x21]
        src     p1
; CHECK: src p1 {{.*}}encoding: [0x23]
        src     p7
; CHECK: src p7 {{.*}}encoding: [0x2f]

; FIN Pn : 0011 RRR0
        fin     p0
; CHECK: fin p0 {{.*}}encoding: [0x30]
        fin     p3
; CHECK: fin p3 {{.*}}encoding: [0x36]
        fin     p7
; CHECK: fin p7 {{.*}}encoding: [0x3e]

; JIN Pn : 0011 RRR1
        jin     p0
; CHECK: jin p0 {{.*}}encoding: [0x31]
        jin     p7
; CHECK: jin p7 {{.*}}encoding: [0x3f]

; FIM Pn, imm8 : 0010 RRR0 + imm
        fim     p0, 0xff
; CHECK: fim p0, 0xff {{.*}}encoding: [0x20,0xff]
        fim     p7, 0x00
; CHECK: fim p7, 0x00 {{.*}}encoding: [0x2e,0x00]

; Pair alias r0r1 == p0, r14r15 == p7
        src     r0r1
; CHECK: src p0 {{.*}}encoding: [0x21]
        src     r14r15
; CHECK: src p7 {{.*}}encoding: [0x2f]
