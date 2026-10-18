; RUN: llvm-mc -triple=i4004 -show-encoding %s | FileCheck %s
;
; Symbolic operands on the two-word instructions emit a fixup for the second
; word (abi.md sec.5.1). The opcode byte is known; the address/immediate byte(s)
; are zero until the linker resolves the relocation.

        .text
start:
; JUN to a symbol -> R_I4004_ADDR12 fixup at offset 0 (spans both bytes).
        jun     target
; CHECK: jun target {{.*}}encoding: [0x40'A',A]
; CHECK-NEXT: fixup A - offset: 0, value: target, kind: fixup_addr12

; JMS to a symbol -> R_I4004_ADDR12.
        jms     target
; CHECK: jms target {{.*}}encoding: [0x50'A',A]
; CHECK-NEXT: fixup A - offset: 0, value: target, kind: fixup_addr12

; JCN Z, label -> R_I4004_PAGE8 fixup in byte 1.
        jcn     z, target
; CHECK: jcn 0x4, target {{.*}}encoding: [0x14,A]
; CHECK-NEXT: fixup A - offset: 1, value: target, kind: fixup_page8

; ISZ R3, label -> R_I4004_PAGE8 fixup in byte 1.
        isz     r3, target
; CHECK: isz r3, target {{.*}}encoding: [0x73,A]
; CHECK-NEXT: fixup A - offset: 1, value: target, kind: fixup_page8

target:
        nop
