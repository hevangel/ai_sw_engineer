//===-- I8086InstrCodeGen.td - I8086 CodeGen instructions --*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Instruction selection (spec/codegen.md). The machine instructions of
// I8086InstrInfo.td have no register semantics (every operand is an input, in
// assembly order); code generation needs outputs, tied operands and the flags.
// So selection targets the "P_" instructions below, one for each machine
// instruction it uses, and I8086MCInstLower turns a P_X into the machine
// instruction X: tied operands collapse into one, an immediate picks the
// shortest encoding, and the memory operand is normalised to base/index.
//
//===----------------------------------------------------------------------===//

//===----------------------------------------------------------------------===//
// Target DAG nodes
//===----------------------------------------------------------------------===//

def SDT_I8086Cmp   : SDTypeProfile<0, 2, [SDTCisSameAs<0, 1>]>;
def SDT_I8086BrCC  : SDTypeProfile<0, 2, [SDTCisVT<0, OtherVT>, SDTCisVT<1, i8>]>;
def SDT_I8086SelCC : SDTypeProfile<1, 3,
    [SDTCisSameAs<0, 1>, SDTCisSameAs<1, 2>, SDTCisVT<3, i8>]>;
def SDT_I8086Call  : SDTypeProfile<0, -1, [SDTCisVT<0, iPTR>]>;
def SDT_I8086Ret   : SDTypeProfile<0, 0, []>;
def SDT_I8086Wrap  : SDTypeProfile<1, 1, [SDTCisSameAs<0, 1>, SDTCisPtrTy<0>]>;
def SDT_I8086Push  : SDTypeProfile<0, 1, [SDTCisVT<0, i16>]>;

// (cmp a, b) sets the flags as `cmp a, b`; the result is glue for the branch
// or select that reads them.
def i8086_cmp      : SDNode<"I8086ISD::CMP", SDT_I8086Cmp, [SDNPOutGlue]>;
// (brcc dest, cc): branch on the flags; cc is an I8086CC code.
def i8086_brcc     : SDNode<"I8086ISD::BRCC", SDT_I8086BrCC,
                            [SDNPHasChain, SDNPInGlue]>;
// (selectcc t, f, cc): t when cc holds on the flags, else f.
def i8086_selectcc : SDNode<"I8086ISD::SELECT_CC", SDT_I8086SelCC, [SDNPInGlue]>;
def i8086_call     : SDNode<"I8086ISD::CALL", SDT_I8086Call,
                            [SDNPHasChain, SDNPOptInGlue, SDNPOutGlue,
                             SDNPVariadic]>;
def i8086_ret      : SDNode<"I8086ISD::RET", SDT_I8086Ret,
                            [SDNPHasChain, SDNPOptInGlue, SDNPVariadic]>;
def i8086_wrapper  : SDNode<"I8086ISD::WRAPPER", SDT_I8086Wrap>;
def i8086_push     : SDNode<"I8086ISD::PUSH", SDT_I8086Push,
                            [SDNPHasChain, SDNPMayStore, SDNPSideEffect]>;

// Add and subtract that leave the carry in the flags (glue) for the next
// add-with-carry / subtract-with-borrow of a wider value.
def i8086_addc : SDNode<"I8086ISD::ADDC", SDTIntBinOp,
                        [SDNPCommutative, SDNPOutGlue]>;
def i8086_adde : SDNode<"I8086ISD::ADDE", SDTIntBinOp,
                        [SDNPCommutative, SDNPInGlue, SDNPOutGlue]>;
def i8086_subc : SDNode<"I8086ISD::SUBC", SDTIntBinOp, [SDNPOutGlue]>;
def i8086_sube : SDNode<"I8086ISD::SUBE", SDTIntBinOp,
                        [SDNPInGlue, SDNPOutGlue]>;

// rep movs / rep stos: CX, SI/DI (and AL/AX) are set by copies; the glue
// orders them before the instruction.
def SDT_I8086Rep : SDTypeProfile<0, 0, []>;
def i8086_rep_movsb : SDNode<"I8086ISD::REP_MOVSB", SDT_I8086Rep,
    [SDNPHasChain, SDNPInGlue, SDNPOutGlue, SDNPMayLoad, SDNPMayStore]>;
def i8086_rep_movsw : SDNode<"I8086ISD::REP_MOVSW", SDT_I8086Rep,
    [SDNPHasChain, SDNPInGlue, SDNPOutGlue, SDNPMayLoad, SDNPMayStore]>;
def i8086_rep_stosb : SDNode<"I8086ISD::REP_STOSB", SDT_I8086Rep,
    [SDNPHasChain, SDNPInGlue, SDNPOutGlue, SDNPMayStore]>;
def i8086_rep_stosw : SDNode<"I8086ISD::REP_STOSW", SDT_I8086Rep,
    [SDNPHasChain, SDNPInGlue, SDNPOutGlue, SDNPMayStore]>;

def SDT_I8086CallSeqStart : SDCallSeqStart<[SDTCisVT<0, i16>, SDTCisVT<1, i16>]>;
def SDT_I8086CallSeqEnd   : SDCallSeqEnd<[SDTCisVT<0, i16>, SDTCisVT<1, i16>]>;
def i8086_callseq_start : SDNode<"ISD::CALLSEQ_START", SDT_I8086CallSeqStart,
                                 [SDNPHasChain, SDNPOutGlue]>;
def i8086_callseq_end : SDNode<"ISD::CALLSEQ_END", SDT_I8086CallSeqEnd,
                               [SDNPHasChain, SDNPOptInGlue, SDNPOutGlue]>;

//===----------------------------------------------------------------------===//
// Operands and instruction class
//===----------------------------------------------------------------------===//

// A memory reference: base (BX or BP), index (SI or DI), displacement
// (immediate or symbol), segment override. SelectAddr builds it.
def cgmem : Operand<i16> {
  let MIOperandInfo = (ops GR16_PTR:$base, GR16_INDEX:$index, i16imm:$disp,
                           SEGREG:$seg);
}
def addr : ComplexPattern<iPTR, 4, "SelectAddr", [], []>;

class CGInst<dag outs, dag ins, list<dag> pattern = []> : Instruction {
  let Namespace = "I8086";
  let OutOperandList = outs;
  let InOperandList = ins;
  let AsmString = "";
  let Pattern = pattern;
  let isPseudo = 1;
  let isCodeGenOnly = 1;
}

//===----------------------------------------------------------------------===//
// Frame setup
//===----------------------------------------------------------------------===//

let Defs = [SP, FLAGS], Uses = [SP] in {
  def ADJCALLSTACKDOWN : CGInst<(outs), (ins i16imm:$amt1, i16imm:$amt2),
      [(i8086_callseq_start timm:$amt1, timm:$amt2)]>;
  def ADJCALLSTACKUP : CGInst<(outs), (ins i16imm:$amt1, i16imm:$amt2),
      [(i8086_callseq_end timm:$amt1, timm:$amt2)]>;
}

//===----------------------------------------------------------------------===//
// Data movement
//===----------------------------------------------------------------------===//

let hasSideEffects = 0 in {
  def P_MOV8rr  : CGInst<(outs GR8:$dst),  (ins GR8:$src)>;
  def P_MOV16rr : CGInst<(outs GR16:$dst), (ins GR16:$src)>;
}

let isMoveImm = 1, isReMaterializable = 1, isAsCheapAsAMove = 1 in {
  def P_MOV8ri  : CGInst<(outs GR8:$dst),  (ins i8imm:$imm),
                         [(set GR8:$dst, (i8 imm:$imm))]>;
  def P_MOV16ri : CGInst<(outs GR16:$dst), (ins i16imm:$imm),
                         [(set GR16:$dst, (i16 imm:$imm))]>;
}
def : Pat<(i8086_wrapper tglobaladdr:$s),   (P_MOV16ri tglobaladdr:$s)>;
def : Pat<(i8086_wrapper texternalsym:$s),  (P_MOV16ri texternalsym:$s)>;
def : Pat<(i8086_wrapper tblockaddress:$s), (P_MOV16ri tblockaddress:$s)>;

let mayLoad = 1 in {
  def P_MOV8rm  : CGInst<(outs GR8:$dst),  (ins cgmem:$src),
                         [(set GR8:$dst, (load addr:$src))]>;
  def P_MOV16rm : CGInst<(outs GR16:$dst), (ins cgmem:$src),
                         [(set GR16:$dst, (load addr:$src))]>;
}
let mayStore = 1 in {
  def P_MOV8mr  : CGInst<(outs), (ins cgmem:$dst, GR8:$src),
                         [(store GR8:$src, addr:$dst)]>;
  def P_MOV16mr : CGInst<(outs), (ins cgmem:$dst, GR16:$src),
                         [(store GR16:$src, addr:$dst)]>;
  def P_MOV8mi  : CGInst<(outs), (ins cgmem:$dst, i8imm:$imm),
                         [(store (i8 imm:$imm), addr:$dst)]>;
  def P_MOV16mi : CGInst<(outs), (ins cgmem:$dst, i16imm:$imm),
                         [(store (i16 imm:$imm), addr:$dst)]>;
}

// The address of a frame object (the only address LEA is used for).
let hasSideEffects = 0 in
def P_LEA16r : CGInst<(outs GR16:$dst), (ins cgmem:$src)>;

//===----------------------------------------------------------------------===//
// Arithmetic and logic
//===----------------------------------------------------------------------===//

multiclass CGAlu<SDNode node, bit comm> {
  let Defs = [FLAGS], Constraints = "$dst = $src1" in {
    let isCommutable = comm in {
      def NAME # "8rr"  : CGInst<(outs GR8:$dst), (ins GR8:$src1, GR8:$src2),
          [(set GR8:$dst, (node GR8:$src1, GR8:$src2))]>;
      def NAME # "16rr" : CGInst<(outs GR16:$dst), (ins GR16:$src1, GR16:$src2),
          [(set GR16:$dst, (node GR16:$src1, GR16:$src2))]>;
    }
    def NAME # "8ri"  : CGInst<(outs GR8:$dst), (ins GR8:$src1, i8imm:$imm),
        [(set GR8:$dst, (node GR8:$src1, (i8 imm:$imm)))]>;
    def NAME # "16ri" : CGInst<(outs GR16:$dst), (ins GR16:$src1, i16imm:$imm),
        [(set GR16:$dst, (node GR16:$src1, (i16 imm:$imm)))]>;
    let mayLoad = 1 in {
      def NAME # "8rm"  : CGInst<(outs GR8:$dst), (ins GR8:$src1, cgmem:$src2),
          [(set GR8:$dst, (node GR8:$src1, (load addr:$src2)))]>;
      def NAME # "16rm" : CGInst<(outs GR16:$dst), (ins GR16:$src1, cgmem:$src2),
          [(set GR16:$dst, (node GR16:$src1, (load addr:$src2)))]>;
    }
  }
}
defm P_ADD : CGAlu<add, 1>;
defm P_SUB : CGAlu<sub, 0>;
defm P_AND : CGAlu<and, 1>;
defm P_OR  : CGAlu<or,  1>;
defm P_XOR : CGAlu<xor, 1>;

// Add and subtract with the carry left in the flags: the same instructions.
def : Pat<(i8086_addc GR16:$a, GR16:$b), (P_ADD16rr GR16:$a, GR16:$b)>;
def : Pat<(i8086_addc GR16:$a, (i16 imm:$i)), (P_ADD16ri GR16:$a, imm:$i)>;
def : Pat<(i8086_addc GR16:$a, (load addr:$b)), (P_ADD16rm GR16:$a, addr:$b)>;
def : Pat<(i8086_addc GR8:$a, (i8 imm:$i)), (P_ADD8ri GR8:$a, imm:$i)>;
def : Pat<(i8086_subc GR16:$a, GR16:$b), (P_SUB16rr GR16:$a, GR16:$b)>;
def : Pat<(i8086_subc GR16:$a, (i16 imm:$i)), (P_SUB16ri GR16:$a, imm:$i)>;
def : Pat<(i8086_subc GR16:$a, (load addr:$b)), (P_SUB16rm GR16:$a, addr:$b)>;

// adc / sbb: the carry (or borrow) in is the flag the previous add left.
multiclass CGAluC<SDNode node> {
  let Defs = [FLAGS], Uses = [FLAGS], Constraints = "$dst = $src1" in {
    def NAME # "8rr"  : CGInst<(outs GR8:$dst), (ins GR8:$src1, GR8:$src2)>;
    def NAME # "8ri"  : CGInst<(outs GR8:$dst), (ins GR8:$src1, i8imm:$imm)>;
    def NAME # "16rr" : CGInst<(outs GR16:$dst), (ins GR16:$src1, GR16:$src2),
        [(set GR16:$dst, (node GR16:$src1, GR16:$src2))]>;
    def NAME # "16ri" : CGInst<(outs GR16:$dst), (ins GR16:$src1, i16imm:$imm),
        [(set GR16:$dst, (node GR16:$src1, (i16 imm:$imm)))]>;
    let mayLoad = 1 in
    def NAME # "16rm" : CGInst<(outs GR16:$dst), (ins GR16:$src1, cgmem:$src2),
        [(set GR16:$dst, (node GR16:$src1, (load addr:$src2)))]>;
  }
}
defm P_ADC : CGAluC<i8086_adde>;
defm P_SBB : CGAluC<i8086_sube>;

// Multiply and divide use AX and DX. mul and imul give the same low half;
// the one-operand forms put the high half in DX. div and idiv take DX:AX.
let Defs = [AX, DX, FLAGS], Uses = [AX], hasSideEffects = 0 in {
  def P_MUL16r  : CGInst<(outs), (ins GR16:$src)>;
  def P_IMUL16r : CGInst<(outs), (ins GR16:$src)>;
}
let Defs = [AX, DX, FLAGS], Uses = [AX, DX], hasSideEffects = 1 in {
  def P_DIV16r  : CGInst<(outs), (ins GR16:$src)>;
  def P_IDIV16r : CGInst<(outs), (ins GR16:$src)>;
}
// cwd: DX = sign extension of AX.
let Defs = [DX], Uses = [AX], hasSideEffects = 0 in
def P_CWD : CGInst<(outs), (ins)>;

// rep movsb/movsw and rep stosb/stosw (the direction flag is clear: abi.md).
let Defs = [CX, SI, DI], Uses = [CX, SI, DI], mayLoad = 1, mayStore = 1 in {
  def P_REP_MOVSB : CGInst<(outs), (ins), [(i8086_rep_movsb)]>;
  def P_REP_MOVSW : CGInst<(outs), (ins), [(i8086_rep_movsw)]>;
}
let Defs = [CX, DI], Uses = [CX, DI, AL], mayStore = 1 in
def P_REP_STOSB : CGInst<(outs), (ins), [(i8086_rep_stosb)]>;
let Defs = [CX, DI], Uses = [CX, DI, AX], mayStore = 1 in
def P_REP_STOSW : CGInst<(outs), (ins), [(i8086_rep_stosw)]>;

// Compare: the flags of a - b, no result.
let Defs = [FLAGS] in {
  def P_CMP8rr  : CGInst<(outs), (ins GR8:$a, GR8:$b),
                         [(i8086_cmp GR8:$a, GR8:$b)]>;
  def P_CMP16rr : CGInst<(outs), (ins GR16:$a, GR16:$b),
                         [(i8086_cmp GR16:$a, GR16:$b)]>;
  def P_CMP8ri  : CGInst<(outs), (ins GR8:$a, i8imm:$imm),
                         [(i8086_cmp GR8:$a, (i8 imm:$imm))]>;
  def P_CMP16ri : CGInst<(outs), (ins GR16:$a, i16imm:$imm),
                         [(i8086_cmp GR16:$a, (i16 imm:$imm))]>;
  let mayLoad = 1 in {
    def P_CMP8rm  : CGInst<(outs), (ins GR8:$a, cgmem:$b),
                           [(i8086_cmp GR8:$a, (load addr:$b))]>;
    def P_CMP16rm : CGInst<(outs), (ins GR16:$a, cgmem:$b),
                           [(i8086_cmp GR16:$a, (load addr:$b))]>;
  }
  // `test a, a` has the same flags as `cmp a, 0` and is shorter.
  def P_TEST8rr  : CGInst<(outs), (ins GR8:$a, GR8:$b)>;
  def P_TEST16rr : CGInst<(outs), (ins GR16:$a, GR16:$b)>;
}
def : Pat<(i8086_cmp GR8:$a,  (i8 0)),  (P_TEST8rr GR8:$a, GR8:$a)>;
def : Pat<(i8086_cmp GR16:$a, (i16 0)), (P_TEST16rr GR16:$a, GR16:$a)>;

let Defs = [FLAGS], Constraints = "$dst = $src1" in {
  def P_NEG8r  : CGInst<(outs GR8:$dst),  (ins GR8:$src1),
                        [(set GR8:$dst, (ineg GR8:$src1))]>;
  def P_NEG16r : CGInst<(outs GR16:$dst), (ins GR16:$src1),
                        [(set GR16:$dst, (ineg GR16:$src1))]>;
  def P_INC16r : CGInst<(outs GR16:$dst), (ins GR16:$src1)>;
  def P_DEC16r : CGInst<(outs GR16:$dst), (ins GR16:$src1)>;
}
let Constraints = "$dst = $src1" in {
  def P_NOT8r  : CGInst<(outs GR8:$dst),  (ins GR8:$src1),
                        [(set GR8:$dst, (not GR8:$src1))]>;
  def P_NOT16r : CGInst<(outs GR16:$dst), (ins GR16:$src1),
                        [(set GR16:$dst, (not GR16:$src1))]>;
}
let AddedComplexity = 1 in {
  def : Pat<(add GR16:$a, (i16 1)),  (P_INC16r GR16:$a)>;
  def : Pat<(add GR16:$a, (i16 -1)), (P_DEC16r GR16:$a)>;
}

// Shifts by one, or by CL (selected in I8086DAGToDAGISel::Select, which
// puts the count in CL).
multiclass CGShift<SDNode node> {
  let Defs = [FLAGS], Constraints = "$dst = $src1" in {
    def NAME # "8r1"  : CGInst<(outs GR8:$dst), (ins GR8:$src1),
        [(set GR8:$dst, (node GR8:$src1, (i8 1)))]>;
    def NAME # "16r1" : CGInst<(outs GR16:$dst), (ins GR16:$src1),
        [(set GR16:$dst, (node GR16:$src1, (i8 1)))]>;
    let Uses = [CL] in {
      def NAME # "8rCL"  : CGInst<(outs GR8:$dst), (ins GR8:$src1)>;
      def NAME # "16rCL" : CGInst<(outs GR16:$dst), (ins GR16:$src1)>;
    }
  }
}
defm P_SHL : CGShift<shl>;
defm P_SHR : CGShift<srl>;
defm P_SAR : CGShift<sra>;

// cbw: AX = sign extension of AL.
let Defs = [AX], Uses = [AL], hasSideEffects = 0 in
def P_CBW : CGInst<(outs), (ins)>;

//===----------------------------------------------------------------------===//
// Conversions
//===----------------------------------------------------------------------===//

def : Pat<(i8 (trunc GR16:$x)),
          (EXTRACT_SUBREG GR16:$x, sub_8bit)>;
def : Pat<(i16 (anyext GR8:$x)),
          (INSERT_SUBREG (i16 (IMPLICIT_DEF)), GR8:$x, sub_8bit)>;
def : Pat<(i16 (zext GR8:$x)),
          (INSERT_SUBREG (INSERT_SUBREG (i16 (IMPLICIT_DEF)), GR8:$x, sub_8bit),
                         (P_MOV8ri 0), sub_8bit_hi)>;

//===----------------------------------------------------------------------===//
// Stack, calls, control flow
//===----------------------------------------------------------------------===//

let Defs = [SP], Uses = [SP], mayStore = 1 in
def P_PUSH16r : CGInst<(outs), (ins GR16:$src), [(i8086_push GR16:$src)]>;
let Defs = [SP], Uses = [SP], mayLoad = 1 in
def P_POP16r : CGInst<(outs GR16:$dst), (ins)>;

let isCall = 1 in {
  def P_CALL  : CGInst<(outs), (ins i16imm:$dst, variable_ops),
                       [(i8086_call (i16 tglobaladdr:$dst))]>;
  def P_CALLr : CGInst<(outs), (ins GR16:$dst, variable_ops),
                       [(i8086_call GR16:$dst)]>;
}
def : Pat<(i8086_call (i16 texternalsym:$dst)), (P_CALL texternalsym:$dst)>;

let isReturn = 1, isTerminator = 1, isBarrier = 1 in
def P_RET : CGInst<(outs), (ins variable_ops), [(i8086_ret)]>;

let isBranch = 1, isTerminator = 1, isBarrier = 1 in
def P_JMP : CGInst<(outs), (ins brtarget16:$dst), [(br bb:$dst)]>;

let isBranch = 1, isTerminator = 1, Uses = [FLAGS] in
def P_JCC : CGInst<(outs), (ins brtarget16:$dst, i8imm:$cc),
                   [(i8086_brcc bb:$dst, timm:$cc)]>;

// A conditional move has no instruction; the custom inserter turns this into
// a diamond (I8086TargetLowering::EmitInstrWithCustomInserter).
let usesCustomInserter = 1, Uses = [FLAGS] in {
  def P_SELECT8  : CGInst<(outs GR8:$dst), (ins GR8:$t, GR8:$f, i8imm:$cc),
      [(set GR8:$dst, (i8086_selectcc GR8:$t, GR8:$f, timm:$cc))]>;
  def P_SELECT16 : CGInst<(outs GR16:$dst), (ins GR16:$t, GR16:$f, i8imm:$cc),
      [(set GR16:$dst, (i8086_selectcc GR16:$t, GR16:$f, timm:$cc))]>;
}

// The high byte of a word is a register, not a shift; masking to the low
// byte is a zero-extension of it.
def : Pat<(i8 (trunc (srl GR16:$x, (i8 8)))), (EXTRACT_SUBREG GR16:$x, sub_8bit_hi)>;
def : Pat<(i8 (trunc (sra GR16:$x, (i8 8)))), (EXTRACT_SUBREG GR16:$x, sub_8bit_hi)>;
let AddedComplexity = 2 in
def : Pat<(and GR16:$x, (i16 255)),
          (INSERT_SUBREG (INSERT_SUBREG (i16 (IMPLICIT_DEF)),
                                        (i8 (EXTRACT_SUBREG GR16:$x, sub_8bit)),
                                        sub_8bit),
                         (P_MOV8ri 0), sub_8bit_hi)>;

// A shift by eight moves a byte: x >> 8 and x << 8 (logical) are two byte
// moves, much cheaper than a shift by CL. The arithmetic shift stays a shift.
def : Pat<(srl GR16:$x, (i8 8)),
          (INSERT_SUBREG (INSERT_SUBREG (i16 (IMPLICIT_DEF)),
                                        (i8 (EXTRACT_SUBREG GR16:$x, sub_8bit_hi)),
                                        sub_8bit),
                         (P_MOV8ri 0), sub_8bit_hi)>;
def : Pat<(shl GR16:$x, (i8 8)),
          (INSERT_SUBREG (INSERT_SUBREG (i16 (IMPLICIT_DEF)),
                                        (P_MOV8ri 0), sub_8bit),
                         (i8 (EXTRACT_SUBREG GR16:$x, sub_8bit)), sub_8bit_hi)>;
