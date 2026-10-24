//===-- I8008InstrInfo.td - I8008 Instruction defs ---------*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// The 8008 repertoire, one record per row of spec/isa.md, with the original
// 1972 mnemonics (register names are part of the mnemonic: `lab`, `adm`).
// tests/mc/ checks every encoding against the MCS-8 manual's table.
//
// Record names: MOV_<d><s> (Lds), MOV_<d>M (LdM), MOV_M<s> (LMs), MVI_<d>,
// MVI_M, INR_<d>, DCR_<d>, <op>_<s> / <op>_M / <op>I for the ALU group.
//
//===----------------------------------------------------------------------===//

//===----------------------------------------------------------------------===//
// Operands
//===----------------------------------------------------------------------===//

// 8-bit immediate: -128..255 or a relocatable expression (%lo/%hi/symbol).
def Imm8AsmOperand : AsmOperandClass {
  let Name = "Imm8";
  let RenderMethod = "addImmOperands";
  let PredicateMethod = "isImm8";
}
def imm8 : Operand<i8> {
  let ParserMatchClass = Imm8AsmOperand;
  let PrintMethod = "printImm8";
  let EncoderMethod = "getImm8OpValue";
}

// 14-bit absolute address (JMP/CAL/Jc/Cc).
def Addr16AsmOperand : AsmOperandClass {
  let Name = "Addr16";
  let RenderMethod = "addImmOperands";
  let PredicateMethod = "isAddr16";
}
def addr16 : Operand<i16> {
  let ParserMatchClass = Addr16AsmOperand;
  let PrintMethod = "printAddr16";
  let EncoderMethod = "getAddr16OpValue";
}

// RST vector (0..7), INP port (0..7), OUT port (8..31).
class SmallImmOperand<string name, int lo, int hi> : AsmOperandClass {
  let Name = name;
  let RenderMethod = "addImmOperands";
  let PredicateMethod = "isConstIn<" # lo # ", " # hi # ">";
}
def RstAsmOperand : SmallImmOperand<"Rst", 0, 7>;
def InpAsmOperand : SmallImmOperand<"Inp", 0, 7>;
def OutAsmOperand : SmallImmOperand<"Out", 8, 31>;
def rstvec  : Operand<i8> { let ParserMatchClass = RstAsmOperand; }
def inport  : Operand<i8> { let ParserMatchClass = InpAsmOperand; }
def outport : Operand<i8> { let ParserMatchClass = OutAsmOperand; }

include "I8008InstrFormats.td"

//===----------------------------------------------------------------------===//
// Register tables (index = DDD/SSS field, 0..6).
//===----------------------------------------------------------------------===//

defvar RegRec = ["A", "B", "C", "D", "E", "H", "L"];
defvar RegAsm = ["a", "b", "c", "d", "e", "h", "l"];

//===----------------------------------------------------------------------===//
// Index register instructions (loads leave the flags alone; INr/DCr set Z S P)
//===----------------------------------------------------------------------===//

let hasSideEffects = 0 in {

foreach d = 0-6 in {
  defvar DR = !cast<Register>(RegRec[d]);
  defvar dn = RegAsm[d];

  foreach s = 0-6 in {
    defvar SR = !cast<Register>(RegRec[s]);
    defvar sn = RegAsm[s];
    let Defs = [DR], Uses = [SR] in
    def !strconcat("MOV_", RegRec[d], RegRec[s]) : Inst1<!or(!shl(0b11, 6), !shl(d, 3), s), "l" # dn # sn>;
  }

  let Defs = [DR], Uses = [H, L], mayLoad = 1 in
  def !strconcat("MOV_", RegRec[d], "M") : Inst1<!or(!shl(0b11, 6), !shl(d, 3), 0b111), "l" # dn # "m">;

  let Uses = [DR, H, L], mayStore = 1 in
  def !strconcat("MOV_M", RegRec[d]) : Inst1<!or(!shl(0b11, 6), !shl(0b111, 3), d), "lm" # dn>;

  let Defs = [DR] in
  def !strconcat("MVI_", RegRec[d]) : InstImm8<!or(!shl(0b00, 6), !shl(d, 3), 0b110), "l" # dn # "i">;

  if !ne(d, 0) then {
    let Defs = [DR, F], Uses = [DR] in {
      def !strconcat("INR_", RegRec[d]) : Inst1<!or(!shl(0b00, 6), !shl(d, 3), 0b000), "in" # dn>;
      def !strconcat("DCR_", RegRec[d]) : Inst1<!or(!shl(0b00, 6), !shl(d, 3), 0b001), "dc" # dn>;
    }
  }
}

let Uses = [H, L], mayStore = 1 in
def MVI_M : InstImm8<0b00111110, "lmi">;

//===----------------------------------------------------------------------===//
// Accumulator group
//===----------------------------------------------------------------------===//

defvar AluRec = ["AD", "AC", "SU", "SB", "ND", "XR", "OR", "CP"];
defvar AluAsm = ["ad", "ac", "su", "sb", "nd", "xr", "or", "cp"];

foreach o = 0-7 in {
  // AC/SB read the carry; CP writes only the flags.
  defvar an = AluAsm[o];
  defvar UsesF = !or(!eq(o, 1), !eq(o, 3));
  defvar IsCmp = !eq(o, 7);
  defvar DefRegs = !if(IsCmp, [F], [A, F]);

  foreach s = 0-6 in {
    defvar SR = !cast<Register>(RegRec[s]);
    defvar sn = RegAsm[s];
    let Defs = DefRegs, Uses = !if(UsesF, [A, SR, F], [A, SR]) in
    def !strconcat(AluRec[o], "_", RegRec[s]) : Inst1<!or(!shl(0b10, 6), !shl(o, 3), s), an # sn>;
  }
  let Defs = DefRegs, Uses = !if(UsesF, [A, H, L, F], [A, H, L]), mayLoad = 1 in
  def !strconcat(AluRec[o], "_M") : Inst1<!or(!shl(0b10, 6), !shl(o, 3), 0b111), an # "m">;
  let Defs = DefRegs, Uses = !if(UsesF, [A, F], [A]) in
  def !strconcat(AluRec[o], "I") : InstImm8<!or(!shl(0b00, 6), !shl(o, 3), 0b100), an # "i">;
}

let Defs = [A, F], Uses = [A] in {
  def RLC : Inst1<0b00000010, "rlc">;
  def RRC : Inst1<0b00001010, "rrc">;
}
let Defs = [A, F], Uses = [A, F] in {
  def RAL : Inst1<0b00010010, "ral">;
  def RAR : Inst1<0b00011010, "rar">;
}

} // hasSideEffects = 0

//===----------------------------------------------------------------------===//
// Program counter and stack control
//===----------------------------------------------------------------------===//

defvar CondAsm = ["c", "z", "s", "p"];

let isBranch = 1, isTerminator = 1, isBarrier = 1, hasSideEffects = 0 in
def JMP : InstAddr<0b01000100, "jmp">;

let isBranch = 1, isTerminator = 1, Uses = [F], hasSideEffects = 0 in
foreach c = 0-3 in {
  def !strconcat("JF", !toupper(CondAsm[c])) : InstAddr<!or(!shl(0b010, 5), !shl(c, 3), 0b000), !strconcat("jf", CondAsm[c])>;
  def !strconcat("JT", !toupper(CondAsm[c])) : InstAddr<!or(!shl(0b011, 5), !shl(c, 3), 0b000), !strconcat("jt", CondAsm[c])>;
}

let isCall = 1, Defs = [A, B, C, D, E, H, L, F] in {
  def CAL : InstAddr<0b01000110, "cal">;
  let Uses = [F] in
  foreach c = 0-3 in {
    def !strconcat("CF", !toupper(CondAsm[c])) : InstAddr<!or(!shl(0b010, 5), !shl(c, 3), 0b010), !strconcat("cf", CondAsm[c])>;
    def !strconcat("CT", !toupper(CondAsm[c])) : InstAddr<!or(!shl(0b011, 5), !shl(c, 3), 0b010), !strconcat("ct", CondAsm[c])>;
  }
  def RST : I8008Inst<(outs), (ins rstvec:$n), "rst\t$n", [], 1> {
    bits<3> n;
    let Inst{7-6} = 0b00;
    let Inst{5-3} = n;
    let Inst{2-0} = 0b101;
    let Inst{23-8} = 0;
  }
}

let isReturn = 1, isTerminator = 1, hasSideEffects = 0 in {
  let isBarrier = 1 in
  def RET : Inst1<0b00000111, "ret">;
  let Uses = [F] in
  foreach c = 0-3 in {
    def !strconcat("RF", !toupper(CondAsm[c])) : Inst1<!or(!shl(0b000, 5), !shl(c, 3), 0b011), !strconcat("rf", CondAsm[c])>;
    def !strconcat("RT", !toupper(CondAsm[c])) : Inst1<!or(!shl(0b001, 5), !shl(c, 3), 0b011), !strconcat("rt", CondAsm[c])>;
  }
}

//===----------------------------------------------------------------------===//
// Input / output, halt
//===----------------------------------------------------------------------===//

let hasSideEffects = 1 in {
  // INP: the Micral N's I/O cards also decode A as a sub-address (isa.md).
  let Defs = [A], Uses = [A] in
  def INP : I8008Inst<(outs), (ins inport:$p), "inp\t$p", [], 1> {
    bits<3> p;
    let Inst{7-4} = 0b0100;
    let Inst{3-1} = p;
    let Inst{0} = 1;
    let Inst{23-8} = 0;
  }
  let Uses = [A] in
  def OUT : I8008Inst<(outs), (ins outport:$p), "out\t$p", [], 1> {
    bits<5> p;
    let Inst{7-6} = 0b01;
    let Inst{5-1} = p;
    let Inst{0} = 1;
    let Inst{23-8} = 0;
  }
  def HLT : Inst1<0b00000000, "hlt">;
}

def I8008InstrInfo : InstrInfo;
