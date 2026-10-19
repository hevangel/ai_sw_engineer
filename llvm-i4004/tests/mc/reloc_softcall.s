; RUN: llvm-mc -triple=i4004 -show-encoding %s | FileCheck %s
; RUN: llvm-mc -triple=i4004 -filetype=obj %s | llvm-readobj -r - | FileCheck %s --check-prefix=RELOC
;
; Software-call return-token materialisation (abi.md sec.4.4.1): a FIM whose
; immediate is %lo8(label) emits the low 8 bits of the code label into byte 1
; (R_I4004_LO8); %page(label) emits the label's page nibble (bits 11..8) into
; byte 1's low nibble (R_I4004_PAGE4). Both defer to a linker relocation like
; the other two-word code operands (reloc.s).

        .text
caller:
; FIM P1, %lo8(.Lret) -> fixup_lo8 in byte 1.
        fim     p1, %lo8(.Lret)
; CHECK: fim p1, %lo8(.Lret) {{.*}}encoding: [0x22,A]
; CHECK-NEXT: fixup A - offset: 1, value: %lo8(.Lret), kind: fixup_lo8

; FIM P2, %page(.Lret) -> fixup_page4 in byte 1.
        fim     p2, %page(.Lret)
; CHECK: fim p2, %page(.Lret) {{.*}}encoding: [0x24,A]
; CHECK-NEXT: fixup A - offset: 1, value: %page(.Lret), kind: fixup_page4

        jms     __push_ret
        jun     callee
.Lret:
        nop

callee:
        jun     __soft_ret

; RELOC: Relocations [
; RELOC: R_I4004_LO8
; RELOC: R_I4004_PAGE4
; RELOC: ]
