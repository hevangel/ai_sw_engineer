//===-- I8086InstrInfo.td - I8086 Instruction defs ---------*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// The 8086 integer instruction set in Intel syntax (spec/asm_syntax.md). Every
// MCInst operand is an input, in assembly order. Names: 8/16 is the operand
// size; r = register, m = memory, i = immediate, a = accumulator, s = segment
// register, o = absolute offset; rr/mr/rm are (r/m, reg) / (r/m, reg) / (reg,
// r/m) in assembly order. Forms that only the assembler (NoDecode,
// isAsmParserOnly) or only the decoder and printer (isCodeGenOnly) use are
// marked; the encoding of each is in TSFlags (I8086InstrFormats.td).
//
//===----------------------------------------------------------------------===//

//===----------------------------------------------------------------------===//
// Operands
//===----------------------------------------------------------------------===//

class ImmAsmOp<string name, list<AsmOperandClass> supers = []> : AsmOperandClass {
  let Name = name;
  let PredicateMethod = "is" # name;
  let RenderMethod = "addImmOperands";
  let SuperClasses = supers;
}

def Imm16AsmOperand : ImmAsmOp<"Imm16">;
def Imm8AsmOperand : ImmAsmOp<"Imm8">;
// A constant that fits a sign-extended byte; preferred for the 83h forms.
def SImm8AsmOperand : ImmAsmOp<"SImm8", [Imm8AsmOperand, Imm16AsmOperand]>;
// A constant outside the sign-extended byte range, or a symbol: the only 16-bit
// immediates for which the accumulator form (shorter by a byte) is chosen.
def Imm16BigAsmOperand : ImmAsmOp<"Imm16Big", [Imm16AsmOperand]>;
def ImmOneAsmOperand : ImmAsmOp<"ImmOne", [Imm8AsmOperand, Imm16AsmOperand]>;
def ImmThreeAsmOperand : ImmAsmOp<"ImmThree", [Imm8AsmOperand, Imm16AsmOperand]>;

class ImmOperand<ValueType vt, AsmOperandClass c, string print = "printImm">
    : Operand<vt> {
  let ParserMatchClass = c;
  let PrintMethod = print;
}
def imm8     : ImmOperand<i8, Imm8AsmOperand>;
def simm8    : ImmOperand<i8, SImm8AsmOperand, "printSImm8">;
def imm16    : ImmOperand<i16, Imm16AsmOperand>;
def imm16big : ImmOperand<i16, Imm16BigAsmOperand>;
def immone   : ImmOperand<i8, ImmOneAsmOperand>;
def immthree : ImmOperand<i8, ImmThreeAsmOperand>;

// Branch targets and far pointers.
def BrTargetAsmOperand : AsmOperandClass {
  let Name = "BrTarget";
  let PredicateMethod = "isBrTarget";
  let RenderMethod = "addImmOperands";
}
def brtarget8  : Operand<OtherVT> {
  let ParserMatchClass = BrTargetAsmOperand;
  let PrintMethod = "printBrTarget";
}
def brtarget16 : Operand<OtherVT> {
  let ParserMatchClass = BrTargetAsmOperand;
  let PrintMethod = "printBrTarget";
}
def FarPtrAsmOperand : AsmOperandClass {
  let Name = "FarPtr";
  let PredicateMethod = "isFarPtr";
  let RenderMethod = "addFarOperands";
}
def farptr : Operand<i32> {
  let ParserMatchClass = FarPtrAsmOperand;
  let PrintMethod = "printFarPtr";
  let MIOperandInfo = (ops i16imm:$off, i16imm:$seg);
}

// Memory operands: base register, index register, displacement, segment
// override (each possibly absent). The size keyword (byte/word/dword ptr) is
// part of the operand; an "x" class requires it, the others accept its absence.
class MemAsmOp<string name, list<AsmOperandClass> supers = []> : AsmOperandClass {
  let Name = name;
  let PredicateMethod = "is" # name;
  let RenderMethod = "addMemOperands";
  let SuperClasses = supers;
}
def MemAnyAsmOperand : MemAsmOp<"MemAny">;
def Mem8AsmOperand   : MemAsmOp<"Mem8", [MemAnyAsmOperand]>;
def Mem16AsmOperand  : MemAsmOp<"Mem16", [MemAnyAsmOperand]>;
def Mem32AsmOperand  : MemAsmOp<"Mem32", [MemAnyAsmOperand]>;
def Mem8xAsmOperand  : MemAsmOp<"Mem8x", [Mem8AsmOperand]>;
def Mem16xAsmOperand : MemAsmOp<"Mem16x", [Mem16AsmOperand]>;
def Mem32xAsmOperand : MemAsmOp<"Mem32x", [Mem32AsmOperand]>;
def MemAbs8AsmOperand  : MemAsmOp<"MemAbs8", [Mem8AsmOperand]>;
def MemAbs16AsmOperand : MemAsmOp<"MemAbs16", [Mem16AsmOperand]>;

class MemOperand<AsmOperandClass c, string print> : Operand<i16> {
  let ParserMatchClass = c;
  let PrintMethod = print;
  let MIOperandInfo = (ops GR16:$base, GR16:$index, i16imm:$disp, SEGREG:$seg);
}
def lea_mem  : MemOperand<MemAnyAsmOperand, "printMemNoSize">;
def i8mem    : MemOperand<Mem8AsmOperand, "printMem8">;
def i16mem   : MemOperand<Mem16AsmOperand, "printMem16">;
def i32mem   : MemOperand<Mem32AsmOperand, "printMem32">;
def i8memx   : MemOperand<Mem8xAsmOperand, "printMem8">;
def i16memx  : MemOperand<Mem16xAsmOperand, "printMem16">;
def i32memx  : MemOperand<Mem32xAsmOperand, "printMem32">;
def i8moffs  : MemOperand<MemAbs8AsmOperand, "printMem8">;
def i16moffs : MemOperand<MemAbs16AsmOperand, "printMem16">;

//===----------------------------------------------------------------------===//
// Arithmetic and logic
//===----------------------------------------------------------------------===//

// ADD OR ADC SBB AND SUB XOR CMP: base opcode b, group digit d.
multiclass ALU<bits<8> b, bits<3> d, string m> {
  def 8rr  : I8086Inst<(outs), (ins GR8:$dst, GR8:$src),  m # "\t$dst, $src", b, FModRM, INone, RmReg>;
  def 8mr  : I8086Inst<(outs), (ins i8mem:$dst, GR8:$src), m # "\t$dst, $src", b, FModRM>;
  def 16rr : I8086Inst<(outs), (ins GR16:$dst, GR16:$src), m # "\t$dst, $src", !add(b, 1), FModRM, INone, RmReg>;
  def 16mr : I8086Inst<(outs), (ins i16mem:$dst, GR16:$src), m # "\t$dst, $src", !add(b, 1), FModRM>;
  def 8rm  : I8086Inst<(outs), (ins GR8:$dst, i8mem:$src), m # "\t$dst, $src", !add(b, 2), FModRM, INone, RegFirst>;
  def 16rm : I8086Inst<(outs), (ins GR16:$dst, i16mem:$src), m # "\t$dst, $src", !add(b, 3), FModRM, INone, RegFirst>;
  let isCodeGenOnly = 1 in {
    def 8rr_rev  : I8086Inst<(outs), (ins GR8:$dst, GR8:$src),  m # "\t$dst, $src", !add(b, 2), FModRM, INone, !or(RegFirst, RmReg)>;
    def 16rr_rev : I8086Inst<(outs), (ins GR16:$dst, GR16:$src), m # "\t$dst, $src", !add(b, 3), FModRM, INone, !or(RegFirst, RmReg)>;
  }
  def 8ai   : I8086Inst<(outs), (ins GR8AL:$dst, imm8:$imm), m # "\t$dst, $imm", !add(b, 4), FNone, I8>;
  def 16ai  : I8086Inst<(outs), (ins GR16AX:$dst, imm16big:$imm), m # "\t$dst, $imm", !add(b, 5), FNone, I16>;
  def 8ri   : I8086Inst<(outs), (ins GR8:$dst, imm8:$imm), m # "\t$dst, $imm", 0x80, FDigit, I8, RmReg, d>;
  def 16ri  : I8086Inst<(outs), (ins GR16:$dst, imm16big:$imm), m # "\t$dst, $imm", 0x81, FDigit, I16, RmReg, d>;
  def 16ri8 : I8086Inst<(outs), (ins GR16:$dst, simm8:$imm), m # "\t$dst, $imm", 0x83, FDigit, I8, !or(RmReg, SExtImm), d>;
  def 8mi   : I8086Inst<(outs), (ins i8memx:$dst, imm8:$imm), m # "\t$dst, $imm", 0x80, FDigit, I8, 0, d>;
  def 16mi  : I8086Inst<(outs), (ins i16memx:$dst, imm16big:$imm), m # "\t$dst, $imm", 0x81, FDigit, I16, 0, d>;
  def 16mi8 : I8086Inst<(outs), (ins i16memx:$dst, simm8:$imm), m # "\t$dst, $imm", 0x83, FDigit, I8, SExtImm, d>;
}
defm ADD : ALU<0x00, 0, "add">;
defm OR  : ALU<0x08, 1, "or">;
defm ADC : ALU<0x10, 2, "adc">;
defm SBB : ALU<0x18, 3, "sbb">;
defm AND : ALU<0x20, 4, "and">;
defm SUB : ALU<0x28, 5, "sub">;
defm XOR : ALU<0x30, 6, "xor">;
defm CMP : ALU<0x38, 7, "cmp">;

// TEST has no (reg, r/m) direction and no sign-extended form.
def TEST8rr  : I8086Inst<(outs), (ins GR8:$dst, GR8:$src), "test\t$dst, $src", 0x84, FModRM, INone, RmReg>;
def TEST8mr  : I8086Inst<(outs), (ins i8mem:$dst, GR8:$src), "test\t$dst, $src", 0x84, FModRM>;
def TEST16rr : I8086Inst<(outs), (ins GR16:$dst, GR16:$src), "test\t$dst, $src", 0x85, FModRM, INone, RmReg>;
def TEST16mr : I8086Inst<(outs), (ins i16mem:$dst, GR16:$src), "test\t$dst, $src", 0x85, FModRM>;
def TEST8ai  : I8086Inst<(outs), (ins GR8AL:$dst, imm8:$imm), "test\t$dst, $imm", 0xA8, FNone, I8>;
def TEST16ai : I8086Inst<(outs), (ins GR16AX:$dst, imm16:$imm), "test\t$dst, $imm", 0xA9, FNone, I16>;
def TEST8ri  : I8086Inst<(outs), (ins GR8:$dst, imm8:$imm), "test\t$dst, $imm", 0xF6, FDigit, I8, RmReg, 0>;
def TEST16ri : I8086Inst<(outs), (ins GR16:$dst, imm16:$imm), "test\t$dst, $imm", 0xF7, FDigit, I16, RmReg, 0>;
def TEST8mi  : I8086Inst<(outs), (ins i8memx:$dst, imm8:$imm), "test\t$dst, $imm", 0xF6, FDigit, I8, 0, 0>;
def TEST16mi : I8086Inst<(outs), (ins i16memx:$dst, imm16:$imm), "test\t$dst, $imm", 0xF7, FDigit, I16, 0, 0>;
// The assembler also takes (reg, r/m) order for TEST.
let isAsmParserOnly = 1 in {
  def TEST8rm  : I8086Inst<(outs), (ins GR8:$src, i8mem:$dst), "test\t$src, $dst", 0x84, FModRM, INone, !or(RegFirst, NoDecode)>;
  def TEST16rm : I8086Inst<(outs), (ins GR16:$src, i16mem:$dst), "test\t$src, $dst", 0x85, FModRM, INone, !or(RegFirst, NoDecode)>;
}

// NOT NEG MUL IMUL DIV IDIV (F6/F7 group) take one operand.
multiclass Unary<bits<3> d, string m> {
  def 8r  : I8086Inst<(outs), (ins GR8:$dst), m # "\t$dst", 0xF6, FDigit, INone, RmReg, d>;
  def 8m  : I8086Inst<(outs), (ins i8memx:$dst), m # "\t$dst", 0xF6, FDigit, INone, 0, d>;
  def 16r : I8086Inst<(outs), (ins GR16:$dst), m # "\t$dst", 0xF7, FDigit, INone, RmReg, d>;
  def 16m : I8086Inst<(outs), (ins i16memx:$dst), m # "\t$dst", 0xF7, FDigit, INone, 0, d>;
}
defm NOT  : Unary<2, "not">;
defm NEG  : Unary<3, "neg">;
defm MUL  : Unary<4, "mul">;
defm IMUL : Unary<5, "imul">;
defm DIV  : Unary<6, "div">;
defm IDIV : Unary<7, "idiv">;

// INC and DEC: one-byte register forms and the ModRM forms.
multiclass IncDec<bits<8> regopc, bits<3> d, string m> {
  def 16r  : I8086Inst<(outs), (ins GR16:$dst), m # "\t$dst", regopc, FRegOpc>;
  def 8r   : I8086Inst<(outs), (ins GR8:$dst), m # "\t$dst", 0xFE, FDigit, INone, RmReg, d>;
  def 8m   : I8086Inst<(outs), (ins i8memx:$dst), m # "\t$dst", 0xFE, FDigit, INone, 0, d>;
  def 16m  : I8086Inst<(outs), (ins i16memx:$dst), m # "\t$dst", 0xFF, FDigit, INone, 0, d>;
  let isCodeGenOnly = 1 in
  def 16r_modrm : I8086Inst<(outs), (ins GR16:$dst), m # "\t$dst", 0xFF, FDigit, INone, RmReg, d>;
}
defm INC : IncDec<0x40, 0, "inc">;
defm DEC : IncDec<0x48, 1, "dec">;

// Shifts and rotates by one (D0/D1) or by CL (D2/D3); /6 is not an instruction.
multiclass Shift<bits<3> d, string m> {
  def 8r1    : I8086Inst<(outs), (ins GR8:$dst, immone:$cnt), m # "\t$dst, $cnt", 0xD0, FDigit, INone, RmReg, d>;
  def 8m1    : I8086Inst<(outs), (ins i8memx:$dst, immone:$cnt), m # "\t$dst, $cnt", 0xD0, FDigit, INone, 0, d>;
  def 16r1   : I8086Inst<(outs), (ins GR16:$dst, immone:$cnt), m # "\t$dst, $cnt", 0xD1, FDigit, INone, RmReg, d>;
  def 16m1   : I8086Inst<(outs), (ins i16memx:$dst, immone:$cnt), m # "\t$dst, $cnt", 0xD1, FDigit, INone, 0, d>;
  def 8rCL   : I8086Inst<(outs), (ins GR8:$dst, GR8CL:$cnt), m # "\t$dst, $cnt", 0xD2, FDigit, INone, RmReg, d>;
  def 8mCL   : I8086Inst<(outs), (ins i8memx:$dst, GR8CL:$cnt), m # "\t$dst, $cnt", 0xD2, FDigit, INone, 0, d>;
  def 16rCL  : I8086Inst<(outs), (ins GR16:$dst, GR8CL:$cnt), m # "\t$dst, $cnt", 0xD3, FDigit, INone, RmReg, d>;
  def 16mCL  : I8086Inst<(outs), (ins i16memx:$dst, GR8CL:$cnt), m # "\t$dst, $cnt", 0xD3, FDigit, INone, 0, d>;
}
defm ROL : Shift<0, "rol">;
defm ROR : Shift<1, "ror">;
defm RCL : Shift<2, "rcl">;
defm RCR : Shift<3, "rcr">;
defm SHL : Shift<4, "shl">;
defm SHR : Shift<5, "shr">;
defm SAR : Shift<7, "sar">;
def : MnemonicAlias<"sal", "shl">;

// Decimal and ASCII adjust.
def DAA : I8086Inst<(outs), (ins), "daa", 0x27, FNone>;
def DAS : I8086Inst<(outs), (ins), "das", 0x2F, FNone>;
def AAA : I8086Inst<(outs), (ins), "aaa", 0x37, FNone>;
def AAS : I8086Inst<(outs), (ins), "aas", 0x3F, FNone>;
def AAM : I8086InstFixed<(outs), (ins), "aam", 0xD4, 0x0A>;
def AAD : I8086InstFixed<(outs), (ins), "aad", 0xD5, 0x0A>;
def AAMi : I8086Inst<(outs), (ins imm8:$imm), "aam\t$imm", 0xD4, FNone, I8>;
def AADi : I8086Inst<(outs), (ins imm8:$imm), "aad\t$imm", 0xD5, FNone, I8>;

def CBW  : I8086Inst<(outs), (ins), "cbw", 0x98, FNone>;
def CWD  : I8086Inst<(outs), (ins), "cwd", 0x99, FNone>;

//===----------------------------------------------------------------------===//
// Data movement
//===----------------------------------------------------------------------===//

def MOV8rr  : I8086Inst<(outs), (ins GR8:$dst, GR8:$src), "mov\t$dst, $src", 0x88, FModRM, INone, RmReg>;
def MOV8mr  : I8086Inst<(outs), (ins i8mem:$dst, GR8:$src), "mov\t$dst, $src", 0x88, FModRM>;
def MOV16rr : I8086Inst<(outs), (ins GR16:$dst, GR16:$src), "mov\t$dst, $src", 0x89, FModRM, INone, RmReg>;
def MOV16mr : I8086Inst<(outs), (ins i16mem:$dst, GR16:$src), "mov\t$dst, $src", 0x89, FModRM>;
def MOV8rm  : I8086Inst<(outs), (ins GR8:$dst, i8mem:$src), "mov\t$dst, $src", 0x8A, FModRM, INone, RegFirst>;
def MOV16rm : I8086Inst<(outs), (ins GR16:$dst, i16mem:$src), "mov\t$dst, $src", 0x8B, FModRM, INone, RegFirst>;
let isCodeGenOnly = 1 in {
  def MOV8rr_rev  : I8086Inst<(outs), (ins GR8:$dst, GR8:$src), "mov\t$dst, $src", 0x8A, FModRM, INone, !or(RegFirst, RmReg)>;
  def MOV16rr_rev : I8086Inst<(outs), (ins GR16:$dst, GR16:$src), "mov\t$dst, $src", 0x8B, FModRM, INone, !or(RegFirst, RmReg)>;
}
// Register immediates use the one-byte-opcode forms; C6/C7 are for memory.
def MOV8ri  : I8086Inst<(outs), (ins GR8:$dst, imm8:$imm), "mov\t$dst, $imm", 0xB0, FRegOpc, I8>;
def MOV16ri : I8086Inst<(outs), (ins GR16:$dst, imm16:$imm), "mov\t$dst, $imm", 0xB8, FRegOpc, I16>;
def MOV8mi  : I8086Inst<(outs), (ins i8memx:$dst, imm8:$imm), "mov\t$dst, $imm", 0xC6, FDigit, I8, 0, 0>;
def MOV16mi : I8086Inst<(outs), (ins i16memx:$dst, imm16:$imm), "mov\t$dst, $imm", 0xC7, FDigit, I16, 0, 0>;
let isCodeGenOnly = 1 in {
  def MOV8ri_modrm  : I8086Inst<(outs), (ins GR8:$dst, imm8:$imm), "mov\t$dst, $imm", 0xC6, FDigit, I8, RmReg, 0>;
  def MOV16ri_modrm : I8086Inst<(outs), (ins GR16:$dst, imm16:$imm), "mov\t$dst, $imm", 0xC7, FDigit, I16, RmReg, 0>;
}
// Accumulator and absolute offset (A0-A3).
def MOV8ao  : I8086Inst<(outs), (ins GR8AL:$dst, i8moffs:$src), "mov\t$dst, $src", 0xA0, FMoffs>;
def MOV16ao : I8086Inst<(outs), (ins GR16AX:$dst, i16moffs:$src), "mov\t$dst, $src", 0xA1, FMoffs>;
def MOV8oa  : I8086Inst<(outs), (ins i8moffs:$dst, GR8AL:$src), "mov\t$dst, $src", 0xA2, FMoffs>;
def MOV16oa : I8086Inst<(outs), (ins i16moffs:$dst, GR16AX:$src), "mov\t$dst, $src", 0xA3, FMoffs>;
// Segment registers (8C store, 8E load; the load never targets CS).
def MOV16rs : I8086Inst<(outs), (ins GR16:$dst, SEGREG:$src), "mov\t$dst, $src", 0x8C, FModRMSeg, INone, RmReg>;
def MOV16ms : I8086Inst<(outs), (ins i16mem:$dst, SEGREG:$src), "mov\t$dst, $src", 0x8C, FModRMSeg>;
def MOV16sr : I8086Inst<(outs), (ins SEGREGNC:$dst, GR16:$src), "mov\t$dst, $src", 0x8E, FModRMSeg, INone, !or(RegFirst, RmReg)>;
def MOV16sm : I8086Inst<(outs), (ins SEGREGNC:$dst, i16mem:$src), "mov\t$dst, $src", 0x8E, FModRMSeg, INone, RegFirst>;

def XCHG8rr  : I8086Inst<(outs), (ins GR8:$dst, GR8:$src), "xchg\t$dst, $src", 0x86, FModRM, INone, RmReg>;
def XCHG8mr  : I8086Inst<(outs), (ins i8mem:$dst, GR8:$src), "xchg\t$dst, $src", 0x86, FModRM>;
def XCHG16rr : I8086Inst<(outs), (ins GR16:$dst, GR16:$src), "xchg\t$dst, $src", 0x87, FModRM, INone, RmReg>;
def XCHG16mr : I8086Inst<(outs), (ins i16mem:$dst, GR16:$src), "xchg\t$dst, $src", 0x87, FModRM>;
def XCHG16ar : I8086Inst<(outs), (ins GR16AX:$dst, GR16:$src), "xchg\t$dst, $src", 0x90, FRegOpc, INone, RegOp1>;
let isAsmParserOnly = 1 in {
  def XCHG8rm  : I8086Inst<(outs), (ins GR8:$src, i8mem:$dst), "xchg\t$src, $dst", 0x86, FModRM, INone, !or(RegFirst, NoDecode)>;
  def XCHG16rm : I8086Inst<(outs), (ins GR16:$src, i16mem:$dst), "xchg\t$src, $dst", 0x87, FModRM, INone, !or(RegFirst, NoDecode)>;
}

def LEA16r : I8086Inst<(outs), (ins GR16:$dst, lea_mem:$src), "lea\t$dst, $src", 0x8D, FModRM, INone, RegFirst>;
def LES16rm : I8086Inst<(outs), (ins GR16:$dst, i32mem:$src), "les\t$dst, $src", 0xC4, FModRM, INone, RegFirst>;
def LDS16rm : I8086Inst<(outs), (ins GR16:$dst, i32mem:$src), "lds\t$dst, $src", 0xC5, FModRM, INone, RegFirst>;

def XLAT  : I8086Inst<(outs), (ins), "xlat", 0xD7, FNone>;
def : MnemonicAlias<"xlatb", "xlat">;

//===----------------------------------------------------------------------===//
// Stack and flags
//===----------------------------------------------------------------------===//

def PUSH16r  : I8086Inst<(outs), (ins GR16:$r), "push\t$r", 0x50, FRegOpc>;
def POP16r   : I8086Inst<(outs), (ins GR16:$r), "pop\t$r", 0x58, FRegOpc>;
def PUSH16s  : I8086Inst<(outs), (ins SEGREG:$r), "push\t$r", 0x06, FSegOpc>;
def POP16s   : I8086Inst<(outs), (ins SEGREGNC:$r), "pop\t$r", 0x07, FSegOpc>;
def PUSH16m  : I8086Inst<(outs), (ins i16mem:$src), "push\t$src", 0xFF, FDigit, INone, 0, 6>;
def POP16m   : I8086Inst<(outs), (ins i16mem:$dst), "pop\t$dst", 0x8F, FDigit, INone, 0, 0>;
let isCodeGenOnly = 1 in {
  def PUSH16r_modrm : I8086Inst<(outs), (ins GR16:$r), "push\t$r", 0xFF, FDigit, INone, RmReg, 6>;
  def POP16r_modrm  : I8086Inst<(outs), (ins GR16:$r), "pop\t$r", 0x8F, FDigit, INone, RmReg, 0>;
}
def PUSHF : I8086Inst<(outs), (ins), "pushf", 0x9C, FNone>;
def POPF  : I8086Inst<(outs), (ins), "popf", 0x9D, FNone>;
def SAHF  : I8086Inst<(outs), (ins), "sahf", 0x9E, FNone>;
def LAHF  : I8086Inst<(outs), (ins), "lahf", 0x9F, FNone>;

def CLC : I8086Inst<(outs), (ins), "clc", 0xF8, FNone>;
def STC : I8086Inst<(outs), (ins), "stc", 0xF9, FNone>;
def CLI : I8086Inst<(outs), (ins), "cli", 0xFA, FNone>;
def STI : I8086Inst<(outs), (ins), "sti", 0xFB, FNone>;
def CLD : I8086Inst<(outs), (ins), "cld", 0xFC, FNone>;
def STD : I8086Inst<(outs), (ins), "std", 0xFD, FNone>;
def CMC : I8086Inst<(outs), (ins), "cmc", 0xF5, FNone>;

//===----------------------------------------------------------------------===//
// Control transfer
//===----------------------------------------------------------------------===//

// Conditional jumps reach only +-127 bytes. "jcc short L" is the fixed
// two-byte form; bare "jcc L" is relaxed by the assembler to the five-byte
// "j!cc +3; jmp near L" when L is out of range (JccF, emitted by the backend).
multiclass Jcc<bits<8> cc, string m, string inv> {
  def _S : I8086Inst<(outs), (ins brtarget8:$t), m # "\tshort $t", cc, FRel8>;
  let isAsmParserOnly = 1 in
  def _R : I8086Inst<(outs), (ins brtarget8:$t), m # "\t$t", cc, FRel8, INone, NoDecode>;
  let isCodeGenOnly = 1 in
  def _F : I8086Inst<(outs), (ins brtarget16:$t), inv # "\tshort 3\n\tjmp\tnear $t", cc, FJccFar>;
}
defm JO  : Jcc<0x70, "jo",  "jno">;
defm JNO : Jcc<0x71, "jno", "jo">;
defm JB  : Jcc<0x72, "jb",  "jae">;
defm JAE : Jcc<0x73, "jae", "jb">;
defm JE  : Jcc<0x74, "je",  "jne">;
defm JNE : Jcc<0x75, "jne", "je">;
defm JBE : Jcc<0x76, "jbe", "ja">;
defm JA  : Jcc<0x77, "ja",  "jbe">;
defm JS  : Jcc<0x78, "js",  "jns">;
defm JNS : Jcc<0x79, "jns", "js">;
defm JP  : Jcc<0x7A, "jp",  "jnp">;
defm JNP : Jcc<0x7B, "jnp", "jp">;
defm JL  : Jcc<0x7C, "jl",  "jge">;
defm JGE : Jcc<0x7D, "jge", "jl">;
defm JLE : Jcc<0x7E, "jle", "jg">;
defm JG  : Jcc<0x7F, "jg",  "jle">;
def : MnemonicAlias<"jc", "jb">;
def : MnemonicAlias<"jnae", "jb">;
def : MnemonicAlias<"jnc", "jae">;
def : MnemonicAlias<"jnb", "jae">;
def : MnemonicAlias<"jz", "je">;
def : MnemonicAlias<"jnz", "jne">;
def : MnemonicAlias<"jna", "jbe">;
def : MnemonicAlias<"jnbe", "ja">;
def : MnemonicAlias<"jpe", "jp">;
def : MnemonicAlias<"jpo", "jnp">;
def : MnemonicAlias<"jnge", "jl">;
def : MnemonicAlias<"jnl", "jge">;
def : MnemonicAlias<"jng", "jle">;
def : MnemonicAlias<"jnle", "jg">;

def JMP_S : I8086Inst<(outs), (ins brtarget8:$t), "jmp\tshort $t", 0xEB, FRel8>;
let isAsmParserOnly = 1 in
def JMP_R : I8086Inst<(outs), (ins brtarget8:$t), "jmp\t$t", 0xEB, FRel8, INone, NoDecode>;
def JMP_N : I8086Inst<(outs), (ins brtarget16:$t), "jmp\tnear $t", 0xE9, FRel16>;
def CALL_N : I8086Inst<(outs), (ins brtarget16:$t), "call\t$t", 0xE8, FRel16>;
def JMPF  : I8086Inst<(outs), (ins farptr:$p), "jmp\t$p", 0xEA, FFar>;
def CALLF : I8086Inst<(outs), (ins farptr:$p), "call\t$p", 0x9A, FFar>;
def JMP16r  : I8086Inst<(outs), (ins GR16:$t), "jmp\t$t", 0xFF, FDigit, INone, RmReg, 4>;
def JMP16m  : I8086Inst<(outs), (ins i16mem:$t), "jmp\t$t", 0xFF, FDigit, INone, 0, 4>;
def CALL16r : I8086Inst<(outs), (ins GR16:$t), "call\t$t", 0xFF, FDigit, INone, RmReg, 2>;
def CALL16m : I8086Inst<(outs), (ins i16mem:$t), "call\t$t", 0xFF, FDigit, INone, 0, 2>;
def JMPFm   : I8086Inst<(outs), (ins i32memx:$t), "jmp\t$t", 0xFF, FDigit, INone, 0, 5>;
def CALLFm  : I8086Inst<(outs), (ins i32memx:$t), "call\t$t", 0xFF, FDigit, INone, 0, 3>;

def RET   : I8086Inst<(outs), (ins), "ret", 0xC3, FNone>;
def RETF  : I8086Inst<(outs), (ins), "retf", 0xCB, FNone>;
def RETi  : I8086Inst<(outs), (ins imm16:$imm), "ret\t$imm", 0xC2, FNone, I16>;
def RETFi : I8086Inst<(outs), (ins imm16:$imm), "retf\t$imm", 0xCA, FNone, I16>;
def : MnemonicAlias<"retn", "ret">;

// loop and jcxz are also produced by the code generator (I8086LoopPeephole).
let isBranch = 1, isTerminator = 1, Uses = [CX] in {
let Defs = [CX] in
def LOOP   : I8086Inst<(outs), (ins brtarget8:$t), "loop\t$t", 0xE2, FRel8>;
}
def LOOPE  : I8086Inst<(outs), (ins brtarget8:$t), "loope\t$t", 0xE1, FRel8>;
def LOOPNE : I8086Inst<(outs), (ins brtarget8:$t), "loopne\t$t", 0xE0, FRel8>;
let isBranch = 1, isTerminator = 1, Uses = [CX] in
def JCXZ   : I8086Inst<(outs), (ins brtarget8:$t), "jcxz\t$t", 0xE3, FRel8>;
def : MnemonicAlias<"loopz", "loope">;
def : MnemonicAlias<"loopnz", "loopne">;

def INT3 : I8086Inst<(outs), (ins), "int3", 0xCC, FNone>;
def INTi : I8086Inst<(outs), (ins imm8:$imm), "int\t$imm", 0xCD, FNone, I8>;
def INTO : I8086Inst<(outs), (ins), "into", 0xCE, FNone>;
def IRET : I8086Inst<(outs), (ins), "iret", 0xCF, FNone>;
def HLT  : I8086Inst<(outs), (ins), "hlt", 0xF4, FNone>;
def NOP  : I8086Inst<(outs), (ins), "nop", 0x90, FNone>;
def WAIT : I8086Inst<(outs), (ins), "wait", 0x9B, FNone>;

//===----------------------------------------------------------------------===//
// I/O, string instructions, prefixes
//===----------------------------------------------------------------------===//

def IN8i    : I8086Inst<(outs), (ins GR8AL:$d, imm8:$p), "in\t$d, $p", 0xE4, FNone, I8>;
def IN16i   : I8086Inst<(outs), (ins GR16AX:$d, imm8:$p), "in\t$d, $p", 0xE5, FNone, I8>;
def OUT8i   : I8086Inst<(outs), (ins imm8:$p, GR8AL:$s), "out\t$p, $s", 0xE6, FNone, I8>;
def OUT16i  : I8086Inst<(outs), (ins imm8:$p, GR16AX:$s), "out\t$p, $s", 0xE7, FNone, I8>;
def IN8dx   : I8086Inst<(outs), (ins GR8AL:$d, GR16DX:$p), "in\t$d, $p", 0xEC, FNone>;
def IN16dx  : I8086Inst<(outs), (ins GR16AX:$d, GR16DX:$p), "in\t$d, $p", 0xED, FNone>;
def OUT8dx  : I8086Inst<(outs), (ins GR16DX:$p, GR8AL:$s), "out\t$p, $s", 0xEE, FNone>;
def OUT16dx : I8086Inst<(outs), (ins GR16DX:$p, GR16AX:$s), "out\t$p, $s", 0xEF, FNone>;

def MOVSB : I8086Inst<(outs), (ins), "movsb", 0xA4, FNone>;
def MOVSW : I8086Inst<(outs), (ins), "movsw", 0xA5, FNone>;
def CMPSB : I8086Inst<(outs), (ins), "cmpsb", 0xA6, FNone>;
def CMPSW : I8086Inst<(outs), (ins), "cmpsw", 0xA7, FNone>;
def STOSB : I8086Inst<(outs), (ins), "stosb", 0xAA, FNone>;
def STOSW : I8086Inst<(outs), (ins), "stosw", 0xAB, FNone>;
def LODSB : I8086Inst<(outs), (ins), "lodsb", 0xAC, FNone>;
def LODSW : I8086Inst<(outs), (ins), "lodsw", 0xAD, FNone>;
def SCASB : I8086Inst<(outs), (ins), "scasb", 0xAE, FNone>;
def SCASW : I8086Inst<(outs), (ins), "scasw", 0xAF, FNone>;

// Prefixes are instructions of their own; "rep movsb" on one line is two.
def LOCK  : I8086Inst<(outs), (ins), "lock", 0xF0, FNone>;
def REPNE : I8086Inst<(outs), (ins), "repne", 0xF2, FNone>;
def REP   : I8086Inst<(outs), (ins), "rep", 0xF3, FNone>;
def SEG_ES : I8086Inst<(outs), (ins), "es", 0x26, FNone>;
def SEG_CS : I8086Inst<(outs), (ins), "cs", 0x2E, FNone>;
def SEG_SS : I8086Inst<(outs), (ins), "ss", 0x36, FNone>;
def SEG_DS : I8086Inst<(outs), (ins), "ds", 0x3E, FNone>;
def : MnemonicAlias<"repe", "rep">;
def : MnemonicAlias<"repz", "rep">;
def : MnemonicAlias<"repnz", "repne">;

def I8086InstrInfo : InstrInfo;
