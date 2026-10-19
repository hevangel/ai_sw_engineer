//===-- I4004InstrFormats.td - I4004 Instruction Formats ---*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Encoding formats for the MCS-4 repertoire (spec/isa.md).
//
// A "word" is one 8-bit program byte: OPR is the high nibble, OPA the low
// nibble.  The MCCodeEmitter writes byte 0 = Inst{7-0} (the opcode byte) first,
// then byte 1 = Inst{15-8} (the second word) for two-byte instructions.  So an
// opcode OPR:OPA is laid out as Inst{7-4}=OPR, Inst{3-0}=OPA, and the second
// word (immediate / address) occupies Inst{15-8}.
//
//===----------------------------------------------------------------------===//

class I4004Inst<dag outs, dag ins, string asmstr, list<dag> pattern,
                int sz> : Instruction {
  let Namespace = "I4004";

  // Up to 16 bits of encoding; one or two bytes are actually emitted (Size).
  field bits<16> Inst;
  // Decoder/encoder mask for the fixed opcode bits (not used in MS2 beyond MC).
  field bits<16> SoftFail = 0;

  let Size = sz;

  dag OutOperandList = outs;
  dag InOperandList = ins;
  let AsmString = asmstr;
  let Pattern = pattern;
}

// One-byte instruction: Inst{15-8} unused (0).
class I4004Inst1<bits<4> opr, bits<4> opa, dag outs, dag ins,
                 string asmstr, list<dag> pattern = []>
    : I4004Inst<outs, ins, asmstr, pattern, 1> {
  let Inst{7-4} = opr;
  let Inst{3-0} = opa;
  let Inst{15-8} = 0;
}

// One-byte instruction whose low nibble is a register field (RRRR).
class I4004InstR<bits<4> opr, dag outs, dag ins, string asmstr,
                 list<dag> pattern = []>
    : I4004Inst<outs, ins, asmstr, pattern, 1> {
  bits<4> reg;
  let Inst{7-4} = opr;
  let Inst{3-0} = reg;
  let Inst{15-8} = 0;
}

// One-byte instruction whose low nibble is a pair field: {RRR, b} where b is a
// fixed sub-op selector bit (0 for FIM/FIN even-ops, 1 for SRC/JIN odd-ops).
class I4004InstP<bits<4> opr, bit b, dag outs, dag ins, string asmstr,
                 list<dag> pattern = []>
    : I4004Inst<outs, ins, asmstr, pattern, 1> {
  bits<3> pair;
  let Inst{7-4} = opr;
  let Inst{3-1} = pair;
  let Inst{0}   = b;
  let Inst{15-8} = 0;
}

// One-byte instruction whose low nibble is 4-bit immediate data (DDDD).
class I4004InstImm4<bits<4> opr, dag outs, dag ins, string asmstr,
                    list<dag> pattern = []>
    : I4004Inst<outs, ins, asmstr, pattern, 1> {
  bits<4> imm;
  let Inst{7-4} = opr;
  let Inst{3-0} = imm;
  let Inst{15-8} = 0;
}

// Two-byte: pair in word 1 low nibble (RRR0), imm8 in word 2 (FIM).
class I4004InstFim<bits<4> opr, dag outs, dag ins, string asmstr,
                   list<dag> pattern = []>
    : I4004Inst<outs, ins, asmstr, pattern, 2> {
  bits<3> pair;
  bits<8> imm;
  let Inst{7-4}  = opr;
  let Inst{3-1}  = pair;
  let Inst{0}    = 0;
  let Inst{15-8} = imm;
}

// Two-byte: JCN cond:4 in word1 OPA, addr8 in word2.
class I4004InstJcn<bits<4> opr, dag outs, dag ins, string asmstr,
                   list<dag> pattern = []>
    : I4004Inst<outs, ins, asmstr, pattern, 2> {
  bits<4> cond;
  bits<8> addr;
  let Inst{7-4}  = opr;
  let Inst{3-0}  = cond;
  let Inst{15-8} = addr;
}

// Two-byte: ISZ reg:4 in word1 OPA, addr8 in word2.
class I4004InstIsz<bits<4> opr, dag outs, dag ins, string asmstr,
                   list<dag> pattern = []>
    : I4004Inst<outs, ins, asmstr, pattern, 2> {
  bits<4> reg;
  bits<8> addr;
  let Inst{7-4}  = opr;
  let Inst{3-0}  = reg;
  let Inst{15-8} = addr;
}

// Two-byte: 12-bit absolute address. Word1 OPA = A3 (high nibble of addr),
// word2 = A2A1 (low byte). JUN / JMS.
class I4004InstAddr12<bits<4> opr, dag outs, dag ins, string asmstr,
                      list<dag> pattern = []>
    : I4004Inst<outs, ins, asmstr, pattern, 2> {
  bits<12> addr;
  let Inst{7-4}  = opr;
  let Inst{3-0}  = addr{11-8};   // A3
  let Inst{15-8} = addr{7-0};    // A2A1
}

//===----------------------------------------------------------------------===//
// CodeGen pseudo-instruction base (milestone 3b). No real encoding; expanded
// post-RA into real MCS-4 instruction sequences by I4004ExpandPseudos. Size is
// a conservative estimate used only for branch-range math before expansion.
//===----------------------------------------------------------------------===//
class Pseudo<dag outs, dag ins, list<dag> pattern = []>
    : I4004Inst<outs, ins, "", pattern, 0> {
  let isPseudo = 1;
  let isCodeGenOnly = 1;
  let Inst{15-0} = 0;
}
