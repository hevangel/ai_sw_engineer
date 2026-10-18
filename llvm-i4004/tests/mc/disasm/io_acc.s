# RUN: llvm-mc -triple=i4004 --disassemble %s | FileCheck %s
#
# Disassembler: RAM/ROM port group OPR = 0xE (spec/isa.md section 2) and
# accumulator group OPR = 0xF (section 3). OPA selects the op in both groups.

# --- OPR 0xE : 16 I/O ops, OPA 0x0..0xF ---
0xe0   # CHECK: wrm
0xe1   # CHECK: wmp
0xe2   # CHECK: wrr
0xe3   # CHECK: wpm
0xe4   # CHECK: wr0
0xe5   # CHECK: wr1
0xe6   # CHECK: wr2
0xe7   # CHECK: wr3
0xe8   # CHECK: sbm
0xe9   # CHECK: rdm
0xea   # CHECK: rdr
0xeb   # CHECK: adm
0xec   # CHECK: rd0
0xed   # CHECK: rd1
0xee   # CHECK: rd2
0xef   # CHECK: rd3

# --- OPR 0xF : 14 defined accumulator ops, OPA 0x0..0xD ---
0xf0   # CHECK: clb
0xf1   # CHECK: clc
0xf2   # CHECK: iac
0xf3   # CHECK: cmc
0xf4   # CHECK: cma
0xf5   # CHECK: ral
0xf6   # CHECK: rar
0xf7   # CHECK: tcc
0xf8   # CHECK: dac
0xf9   # CHECK: tcs
0xfa   # CHECK: stc
0xfb   # CHECK: daa
0xfc   # CHECK: kbp
0xfd   # CHECK: dcl
