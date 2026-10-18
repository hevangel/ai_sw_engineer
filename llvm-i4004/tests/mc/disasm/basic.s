# RUN: llvm-mc -triple=i4004 --disassemble %s | FileCheck %s
#
# Disassembler: basic instructions OPR = 0x0 .. 0xD (spec/isa.md section 1).
# Input is a raw byte stream; each CHECK is the expected mnemonic. Byte values
# are the manufacturer encodings (the same table the encode tests in basic.s
# verify from the other direction).

0x00                 # CHECK: nop
0x18 0x40            # CHECK: jcn 0x8, 0x40
0x22 0x3c            # CHECK: fim p1, 0x3c
0x25                 # CHECK: src p2
0x34                 # CHECK: fin p2
0x35                 # CHECK: jin p2
0x43 0xa1            # CHECK: jun 0x3a1
0x53 0xa1            # CHECK: jms 0x3a1
0x65                 # CHECK: inc r5
0x73 0x20            # CHECK: isz r3, 0x20
0x85                 # CHECK: add r5
0x95                 # CHECK: sub r5
0xa5                 # CHECK: ld r5
0xb5                 # CHECK: xch r5
0xc0                 # CHECK: bbl 0
0xd9                 # CHECK: ldm 9
