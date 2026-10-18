# RUN: llvm-mc -triple=i4004 --filetype=obj %s -o %t.o
# RUN: llvm-readobj -r %t.o | FileCheck %s
#
# R_I4004_FIN8 (abi.md section 5.1) fixes the pair-0 index of a FIN constant
# load; it is page-local and 8-bit. No instruction encoder emits it yet (that
# is CodeGen-era work), so it was previously untested. The AsmBackend's
# getFixupKind(StringRef) lets a `.reloc` directive name it directly, which is
# how we exercise the ELF writer's fixup_fin8 -> R_I4004_FIN8 mapping here.
# The same mechanism covers the other three target relocations, so all four are
# now assertable from assembly.

        .text
start:
        fin     p0
        .reloc ., R_I4004_FIN8, fintarget
        nop
        .reloc ., R_I4004_PAGE8, fintarget
        nop
        .reloc ., R_I4004_ADDR12, fintarget
        nop
        .reloc ., R_I4004_IMM8, fintarget
        nop
fintarget:
        nop

# CHECK: Relocations [
# CHECK: .rela.text {
# CHECK-DAG: R_I4004_FIN8 fintarget
# CHECK-DAG: R_I4004_PAGE8 fintarget
# CHECK-DAG: R_I4004_ADDR12 fintarget
# CHECK-DAG: R_I4004_IMM8 fintarget
# CHECK: ]
