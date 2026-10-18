; RUN: llvm-mc -triple=i4004 -filetype=obj %s -o %t.o
; RUN: llvm-readobj --file-headers --relocations %t.o | FileCheck %s
;
; The ELF object writer stamps the private e_machine 0x4004 (abi.md sec.5) and
; emits R_I4004_* relocations for unresolved JUN/JMS/JCN/FIM operands.

; CHECK: Format: elf32-i4004
; CHECK: Machine: EM_I4004 (0x4004)

        .text
        jun     ext_target      ; R_I4004_ADDR12
        jms     ext_target      ; R_I4004_ADDR12
        jcn     z, ext_target   ; R_I4004_PAGE8
        fim     p1, ext_imm     ; R_I4004_IMM8

; CHECK: Relocations [
; CHECK: R_I4004_ADDR12 ext_target
; CHECK: R_I4004_ADDR12 ext_target
; CHECK: R_I4004_PAGE8 ext_target
; CHECK: R_I4004_IMM8 ext_imm
; CHECK: ]
