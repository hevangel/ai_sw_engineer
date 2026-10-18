; RUN: llvm-mc -triple=i4004 -show-encoding %s | FileCheck %s
;
; Accumulator-group instructions, OPR = 0xF (spec/isa.md sec.3).
; 14 defined: clb=0xF0 .. dcl=0xFD. 0xFE/0xFF are undefined (isa.md sec.8) and
; have no mnemonic (see errors/err_undefined.s).

        clb
; CHECK: clb {{.*}}encoding: [0xf0]
        clc
; CHECK: clc {{.*}}encoding: [0xf1]
        iac
; CHECK: iac {{.*}}encoding: [0xf2]
        cmc
; CHECK: cmc {{.*}}encoding: [0xf3]
        cma
; CHECK: cma {{.*}}encoding: [0xf4]
        ral
; CHECK: ral {{.*}}encoding: [0xf5]
        rar
; CHECK: rar {{.*}}encoding: [0xf6]
        tcc
; CHECK: tcc {{.*}}encoding: [0xf7]
        dac
; CHECK: dac {{.*}}encoding: [0xf8]
        tcs
; CHECK: tcs {{.*}}encoding: [0xf9]
        stc
; CHECK: stc {{.*}}encoding: [0xfa]
        daa
; CHECK: daa {{.*}}encoding: [0xfb]
        kbp
; CHECK: kbp {{.*}}encoding: [0xfc]
        dcl
; CHECK: dcl {{.*}}encoding: [0xfd]
