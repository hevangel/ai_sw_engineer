//===-- I8008InstrFormats.td - I8008 Instruction Formats ---*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Encoding formats (spec/isa.md). An instruction is 1, 2 or 3 bytes; byte 0
// is Inst{7-0} (the opcode), byte 1 Inst{15-8}, byte 2 Inst{23-16}. A 3-byte
// address is little-endian, so the 16-bit address field is Inst{23-8}.
//
//===----------------------------------------------------------------------===//

class I8008Inst<dag outs, dag ins, string asmstr, list<dag> pattern, int sz>
    : Instruction {
  let Namespace = "I8008";
  field bits<24> Inst;
  field bits<24> SoftFail = 0;
  let Size = sz;
  dag OutOperandList = outs;
  dag InOperandList = ins;
  let AsmString = asmstr;
  let Pattern = pattern;
}

// One byte, no operands.
class Inst1<int op, string asmstr>
    : I8008Inst<(outs), (ins), asmstr, [], 1> {
  let Inst{7-0} = op;
  let Inst{23-8} = 0;
}

// Two bytes: opcode + 8-bit immediate.
class InstImm8<int op, string mnem>
    : I8008Inst<(outs), (ins imm8:$imm), mnem # "\t$imm", [], 2> {
  bits<8> imm;
  let Inst{7-0} = op;
  let Inst{15-8} = imm;
  let Inst{23-16} = 0;
}

// Three bytes: opcode + 14-bit address (low byte, then high byte).
class InstAddr<int op, string mnem>
    : I8008Inst<(outs), (ins addr16:$addr), mnem # "\t$addr", [], 3> {
  bits<16> addr;
  let Inst{7-0} = op;
  let Inst{23-8} = addr;
}

// CodeGen pseudo: selected by ISel, expanded post-RA (I8008ExpandPseudos).
class Pseudo<dag outs, dag ins, list<dag> pattern = []>
    : I8008Inst<outs, ins, "", pattern, 0> {
  let isPseudo = 1;
  let isCodeGenOnly = 1;
  let Inst{23-0} = 0;
}
