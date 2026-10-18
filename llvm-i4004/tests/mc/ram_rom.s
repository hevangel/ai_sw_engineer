; RUN: llvm-mc -triple=i4004 -show-encoding %s | FileCheck %s
;
; RAM / ROM port instructions, OPR = 0xE (spec/isa.md sec.2).
; wrm=0xE0 .. rd3=0xEF.

        wrm
; CHECK: wrm {{.*}}encoding: [0xe0]
        wmp
; CHECK: wmp {{.*}}encoding: [0xe1]
        wrr
; CHECK: wrr {{.*}}encoding: [0xe2]
        wpm
; CHECK: wpm {{.*}}encoding: [0xe3]
        wr0
; CHECK: wr0 {{.*}}encoding: [0xe4]
        wr1
; CHECK: wr1 {{.*}}encoding: [0xe5]
        wr2
; CHECK: wr2 {{.*}}encoding: [0xe6]
        wr3
; CHECK: wr3 {{.*}}encoding: [0xe7]
        sbm
; CHECK: sbm {{.*}}encoding: [0xe8]
        rdm
; CHECK: rdm {{.*}}encoding: [0xe9]
        rdr
; CHECK: rdr {{.*}}encoding: [0xea]
        adm
; CHECK: adm {{.*}}encoding: [0xeb]
        rd0
; CHECK: rd0 {{.*}}encoding: [0xec]
        rd1
; CHECK: rd1 {{.*}}encoding: [0xed]
        rd2
; CHECK: rd2 {{.*}}encoding: [0xee]
        rd3
; CHECK: rd3 {{.*}}encoding: [0xef]
