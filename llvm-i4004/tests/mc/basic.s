; RUN: llvm-mc -triple=i4004 -show-encoding %s | FileCheck %s
;
; Basic instructions, OPR = 0x0 .. 0xD (spec/isa.md sec.1).
; One CHECK per row; the encoding bytes are taken from the manufacturer opcode
; table (isa.md sec.1): byte0 = OPR:OPA, byte1 = second word.

; NOP = 0000 0000
        nop
; CHECK: nop {{.*}}encoding: [0x00]

; FIM P1, 0x3c : 0010 RRR0 (pair1 -> RRR=001) + DDDDDDDD -> [0x22,0x3c]
        fim     p1, 0x3c
; CHECK: fim p1, 0x3c {{.*}}encoding: [0x22,0x3c]

; SRC P2 : 0010 RRR1 (pair2 -> RRR=010) -> 0010 0101 = 0x25
        src     p2
; CHECK: src p2 {{.*}}encoding: [0x25]

; FIN P2 : 0011 RRR0 -> 0011 0100 = 0x34
        fin     p2
; CHECK: fin p2 {{.*}}encoding: [0x34]

; JIN P2 : 0011 RRR1 -> 0011 0101 = 0x35
        jin     p2
; CHECK: jin p2 {{.*}}encoding: [0x35]

; JUN 0x3a1 : 0100 AAAA(A3=3) + A2A1(0xa1) -> [0x43,0xa1]
        jun     0x3a1
; CHECK: jun 0x3a1 {{.*}}encoding: [0x43,0xa1]

; JMS 0x3a1 : 0101 AAAA + A2A1 -> [0x53,0xa1]
        jms     0x3a1
; CHECK: jms 0x3a1 {{.*}}encoding: [0x53,0xa1]

; INC R5 : 0110 RRRR -> 0110 0101 = 0x65
        inc     r5
; CHECK: inc r5 {{.*}}encoding: [0x65]

; ISZ R3, 0x20 : 0111 RRRR(3) + A2A1(0x20) -> [0x73,0x20]
        isz     r3, 0x20
; CHECK: isz r3, 0x20 {{.*}}encoding: [0x73,0x20]

; ADD R5 : 1000 0101 = 0x85
        add     r5
; CHECK: add r5 {{.*}}encoding: [0x85]

; SUB R5 : 1001 0101 = 0x95
        sub     r5
; CHECK: sub r5 {{.*}}encoding: [0x95]

; LD R5 : 1010 0101 = 0xa5
        ld      r5
; CHECK: ld r5 {{.*}}encoding: [0xa5]

; XCH R5 : 1011 0101 = 0xb5
        xch     r5
; CHECK: xch r5 {{.*}}encoding: [0xb5]

; BBL 0 : 1100 0000 = 0xc0
        bbl     0
; CHECK: bbl 0 {{.*}}encoding: [0xc0]

; LDM 9 : 1101 1001 = 0xd9
        ldm     9
; CHECK: ldm 9 {{.*}}encoding: [0xd9]
