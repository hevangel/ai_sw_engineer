//===-- I4004InstrInfo.td - I4004 Instruction defs ---------*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// The 46 MCS-4 instructions, one record per row of spec/isa.md sec.sec.1-sec.sec.3.
// Encodings are checked byte-for-byte by tests/mc/*.s against the manufacturer
// opcode table.  No selection patterns here (milestone 2 = MC only).
//
//===----------------------------------------------------------------------===//

include "I4004InstrFormats.td"

//===----------------------------------------------------------------------===//
// Operand classes
//===----------------------------------------------------------------------===//

// 4-bit immediate (LDM/BBL data), 0..15.
def Imm4AsmOperand : AsmOperandClass {
  let Name = "Imm4";
  let RenderMethod = "addImmOperands";
  let PredicateMethod = "isImm4";
}
def imm4 : Operand<i8>, ImmLeaf<i8, [{ return Imm >= 0 && Imm < 16; }]> {
  let ParserMatchClass = Imm4AsmOperand;
  let PrintMethod = "printImm4";
}

// 8-bit immediate (FIM data), 0..255.
def Imm8AsmOperand : AsmOperandClass {
  let Name = "Imm8";
  let RenderMethod = "addImmOperands";
  let PredicateMethod = "isImm8";
}
def imm8 : Operand<i8>, ImmLeaf<i8, [{ return Imm >= 0 && Imm < 256; }]> {
  let ParserMatchClass = Imm8AsmOperand;
  let PrintMethod = "printImm8";
  let EncoderMethod = "getImm8OpValue";
}

// JCN condition nibble (0..15), accepts hex or mnemonic combos (Z/C/T/N...).
def CondAsmOperand : AsmOperandClass {
  let Name = "Cond";
  let RenderMethod = "addImmOperands";
  let PredicateMethod = "isCond";
  let ParserMethod = "parseCond";
}
def condcode : Operand<i8> {
  let ParserMatchClass = CondAsmOperand;
  let PrintMethod = "printCond";
}

// 8-bit page-local code address (JCN / ISZ second word).
def Addr8AsmOperand : AsmOperandClass {
  let Name = "Addr8";
  let RenderMethod = "addImmOperands";
  let PredicateMethod = "isAddr8";
}
def addr8 : Operand<i16> {
  let ParserMatchClass = Addr8AsmOperand;
  let PrintMethod = "printAddr8";
  let EncoderMethod = "getPage8OpValue";
  let OperandType = "OPERAND_PCREL";
}

// 12-bit absolute code address (JUN / JMS).
def Addr12AsmOperand : AsmOperandClass {
  let Name = "Addr12";
  let RenderMethod = "addImmOperands";
  let PredicateMethod = "isAddr12";
}
def addr12 : Operand<i16> {
  let ParserMatchClass = Addr12AsmOperand;
  let PrintMethod = "printAddr12";
  let EncoderMethod = "getAddr12OpValue";
  let OperandType = "OPERAND_PCREL";
}

// Index-register and pair operands. Plain register operands: TableGen derives
// the matcher class from the register class, so no custom AsmOperandClass.
def gr4 : RegisterOperand<GR4>;
def pr8 : RegisterOperand<PR8>;

//===----------------------------------------------------------------------===//
// Scheduling / default properties
//===----------------------------------------------------------------------===//

let hasSideEffects = 0 in {

//===----------------------------------------------------------------------===//
// sec.sec.1  Basic instructions (OPR = 0x0 .. 0xD)
//===----------------------------------------------------------------------===//

def NOP : I4004Inst1<0x0, 0x0, (outs), (ins), "nop">;

// JCN cond, addr8  : 0001 C4C3C2C1 + A2A1
def JCN : I4004InstJcn<0x1, (outs), (ins condcode:$cond, addr8:$addr),
                       "jcn\t$cond, $addr">;

// FIM Pn, imm8     : 0010 RRR0 + DDDDDDDD
def FIM : I4004InstFim<0x2, (outs), (ins pr8:$pair, imm8:$imm),
                       "fim\t$pair, $imm">;

// SRC Pn           : 0010 RRR1
def SRC : I4004InstP<0x2, 1, (outs), (ins pr8:$pair), "src\t$pair">;

// FIN Pn           : 0011 RRR0
def FIN : I4004InstP<0x3, 0, (outs), (ins pr8:$pair), "fin\t$pair">;

// JIN Pn           : 0011 RRR1
def JIN : I4004InstP<0x3, 1, (outs), (ins pr8:$pair), "jin\t$pair">;

// JUN addr12       : 0100 AAAA + A2A1
def JUN : I4004InstAddr12<0x4, (outs), (ins addr12:$addr), "jun\t$addr">;

// JMS addr12       : 0101 AAAA + A2A1
def JMS : I4004InstAddr12<0x5, (outs), (ins addr12:$addr), "jms\t$addr">;

// INC Rn           : 0110 RRRR
def INC : I4004InstR<0x6, (outs), (ins gr4:$reg), "inc\t$reg">;

// ISZ Rn, addr8    : 0111 RRRR + A2A1
def ISZ : I4004InstIsz<0x7, (outs), (ins gr4:$reg, addr8:$addr),
                       "isz\t$reg, $addr">;

// ADD Rn           : 1000 RRRR
def ADD : I4004InstR<0x8, (outs), (ins gr4:$reg), "add\t$reg">;

// SUB Rn           : 1001 RRRR
def SUB : I4004InstR<0x9, (outs), (ins gr4:$reg), "sub\t$reg">;

// LD Rn            : 1010 RRRR
def LD  : I4004InstR<0xA, (outs), (ins gr4:$reg), "ld\t$reg">;

// XCH Rn           : 1011 RRRR
def XCH : I4004InstR<0xB, (outs), (ins gr4:$reg), "xch\t$reg">;

// BBL imm4         : 1100 DDDD
def BBL : I4004InstImm4<0xC, (outs), (ins imm4:$imm), "bbl\t$imm">;

// LDM imm4         : 1101 DDDD
def LDM : I4004InstImm4<0xD, (outs), (ins imm4:$imm), "ldm\t$imm">;

//===----------------------------------------------------------------------===//
// sec.sec.2  RAM / ROM port instructions (OPR = 0xE)
//===----------------------------------------------------------------------===//

class IOInst<bits<4> opa, string mnem> : I4004Inst1<0xE, opa, (outs), (ins), mnem>;

def WRM : IOInst<0x0, "wrm">;
def WMP : IOInst<0x1, "wmp">;
def WRR : IOInst<0x2, "wrr">;
def WPM : IOInst<0x3, "wpm">;
def WR0 : IOInst<0x4, "wr0">;
def WR1 : IOInst<0x5, "wr1">;
def WR2 : IOInst<0x6, "wr2">;
def WR3 : IOInst<0x7, "wr3">;
def SBM : IOInst<0x8, "sbm">;
def RDM : IOInst<0x9, "rdm">;
def RDR : IOInst<0xA, "rdr">;
def ADM : IOInst<0xB, "adm">;
def RD0 : IOInst<0xC, "rd0">;
def RD1 : IOInst<0xD, "rd1">;
def RD2 : IOInst<0xE, "rd2">;
def RD3 : IOInst<0xF, "rd3">;

//===----------------------------------------------------------------------===//
// sec.sec.3  Accumulator-group instructions (OPR = 0xF) - 14 defined
//===----------------------------------------------------------------------===//

class AccInst<bits<4> opa, string mnem> : I4004Inst1<0xF, opa, (outs), (ins), mnem>;

def CLB : AccInst<0x0, "clb">;
def CLC : AccInst<0x1, "clc">;
def IAC : AccInst<0x2, "iac">;
def CMC : AccInst<0x3, "cmc">;
def CMA : AccInst<0x4, "cma">;
def RAL : AccInst<0x5, "ral">;
def RAR : AccInst<0x6, "rar">;
def TCC : AccInst<0x7, "tcc">;
def DAC : AccInst<0x8, "dac">;
def TCS : AccInst<0x9, "tcs">;
def STC : AccInst<0xA, "stc">;
def DAA : AccInst<0xB, "daa">;
def KBP : AccInst<0xC, "kbp">;
def DCL : AccInst<0xD, "dcl">;
// 0xFE and 0xFF are undefined (isa.md sec.sec.8): the assembler must not emit them.

} // hasSideEffects = 0

//===----------------------------------------------------------------------===//
// Assembler aliases / pseudos (asm_syntax.md)
//===----------------------------------------------------------------------===//

// JMP addr : unconditional in-page jump == JCN 0x8, addr (invert, no cond).
def : InstAlias<"jmp\t$addr", (JCN 0x8, addr8:$addr)>;

//===----------------------------------------------------------------------===//
// Instruction-set container record (referenced by I4004.td's Target def).
//===----------------------------------------------------------------------===//

def I4004InstrInfo : InstrInfo;
