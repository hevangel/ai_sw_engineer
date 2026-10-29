//===-- I8086InstrFormats.td - I8086 Instruction Formats ---*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// An 8086 instruction is a variable-length byte string: [prefixes] opcode
// [ModRM [displacement]] [immediate]. TableGen cannot express that as a fixed
// bit pattern, so each instruction carries its encoding in TSFlags and the
// MC code emitter, decoder and parser interpret it (spec/isa.md). The layout
// below must match MCTargetDesc/I8086BaseInfo.h bit for bit.
//
//   TSFlags{7-0}    primary opcode byte
//   TSFlags{10-8}   ModRM reg-field digit (/n), or the segment register for
//                   the push/pop-segment opcodes
//   TSFlags{14-11}  form (below)
//   TSFlags{16-15}  immediate: 0 none, 1 one byte, 2 two bytes
//   TSFlags{17}     RegFirst: MCInst operands are (reg, r/m...) not (r/m..., reg)
//   TSFlags{18}     RmReg:    the r/m operand is a register (ModRM mod = 11)
//   TSFlags{19}     NoDecode: for the assembler only; the decoder skips it
//   TSFlags{20}     RegOp1:   the register encoded in the opcode is operand 1
//   TSFlags{21}     SExtImm:  the one-byte immediate is sign-extended to 16 bits
//   TSFlags{22}     HasDigit: ModRM reg field is the fixed digit
//   TSFlags{23}     HasFixed: a fixed second opcode byte follows
//   TSFlags{31-24}  that fixed byte
//
//===----------------------------------------------------------------------===//

// Forms.
defvar FNone     = 0;  // fixed bytes, no ModRM; operands past the opcode are not encoded
defvar FRegOpc   = 1;  // opcode | register number (low three bits)
defvar FSegOpc   = 2;  // opcode | segment register number << 3
defvar FModRM    = 3;  // ModRM with a register operand in the reg field
defvar FDigit    = 4;  // ModRM with /digit in the reg field
defvar FModRMSeg = 5;  // ModRM with a segment register in the reg field
defvar FRel8     = 6;  // 8-bit PC-relative displacement
defvar FRel16    = 7;  // 16-bit PC-relative displacement
defvar FFar      = 8;  // 16-bit offset then 16-bit segment
defvar FMoffs    = 9;  // 16-bit absolute offset (the accumulator moves)
defvar FJccFar   = 10; // inverted Jcc over a near JMP: the relaxed conditional jump

// Immediate sizes.
defvar INone = 0;
defvar I8    = 1;
defvar I16   = 2;

// Attribute bits (TSFlags{22-17}).
defvar RegFirst = 1;
defvar RmReg    = 2;
defvar NoDecode = 4;
defvar RegOp1   = 8;
defvar SExtImm  = 16;
defvar HasDigit = 32;

class I8086Inst<dag outs, dag ins, string asmstr, bits<8> opc, int form,
                int imm = INone, int attrs = 0, bits<3> digit = 0>
    : Instruction {
  let Namespace = "I8086";
  let OutOperandList = outs;
  let InOperandList = ins;
  let AsmString = asmstr;
  let Size = 0;
  let TSFlags{7-0} = opc;
  let TSFlags{10-8} = digit;
  let TSFlags{14-11} = form;
  let TSFlags{16-15} = imm;
  let TSFlags{22-17} = attrs;
  let TSFlags{23} = 0;
  let TSFlags{31-24} = 0;
}

// An instruction followed by a fixed second opcode byte (AAM, AAD).
class I8086InstFixed<dag outs, dag ins, string asmstr, bits<8> opc, bits<8> fixed>
    : I8086Inst<outs, ins, asmstr, opc, FNone> {
  let TSFlags{23} = 1;
  let TSFlags{31-24} = fixed;
}
