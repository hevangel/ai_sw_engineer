//===-- I8008InstrCodeGen.td - I8008 CodeGen patterns -----*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Instruction selection (spec/codegen.md sec.2). Every 8008 ALU instruction
// works on A and every memory access goes through H:L, so selection targets
// pseudo-instructions over GR8/GR16; I8008ExpandPseudos lowers them post-RA
// using A, H, L and the flags as scratch. A pseudo's Defs list the scratch it
// clobbers.
//
//===----------------------------------------------------------------------===//

//===----------------------------------------------------------------------===//
// Target DAG nodes
//===----------------------------------------------------------------------===//

def SDT_I8008Ret  : SDTypeProfile<0, 0, []>;
def SDT_I8008Call : SDTypeProfile<0, -1, [SDTCisVT<0, iPTR>]>;
def SDT_I8008Wrap : SDTypeProfile<1, 1, [SDTCisSameAs<0, 1>, SDTCisPtrTy<0>]>;

def i8008_ret  : SDNode<"I8008ISD::RET", SDT_I8008Ret,
                        [SDNPHasChain, SDNPOptInGlue, SDNPVariadic]>;
def i8008_call : SDNode<"I8008ISD::CALL", SDT_I8008Call,
                        [SDNPHasChain, SDNPOptInGlue, SDNPOutGlue, SDNPVariadic]>;
def i8008_wrapper : SDNode<"I8008ISD::WRAPPER", SDT_I8008Wrap>;

// (brcc lhs, rhs, cc, dest): compare and branch; cc is an I8008CC code.
def SDT_I8008BrCC : SDTypeProfile<0, 4,
    [SDTCisSameAs<0, 1>, SDTCisVT<2, i8>, SDTCisVT<3, OtherVT>]>;
def i8008_brcc : SDNode<"I8008ISD::BRCC", SDT_I8008BrCC, [SDNPHasChain]>;

// (selectcc lhs, rhs, tval, fval, cc)
def SDT_I8008SelCC : SDTypeProfile<1, 5,
    [SDTCisSameAs<0, 3>, SDTCisSameAs<3, 4>, SDTCisSameAs<1, 2>,
     SDTCisVT<5, i8>]>;
def i8008_selectcc : SDNode<"I8008ISD::SELECT_CC", SDT_I8008SelCC>;

def SDT_I8008CallSeqStart : SDCallSeqStart<[SDTCisVT<0, i16>, SDTCisVT<1, i16>]>;
def SDT_I8008CallSeqEnd   : SDCallSeqEnd<[SDTCisVT<0, i16>, SDTCisVT<1, i16>]>;
def i8008_callseq_start : SDNode<"ISD::CALLSEQ_START", SDT_I8008CallSeqStart,
                                 [SDNPHasChain, SDNPOutGlue]>;
def i8008_callseq_end : SDNode<"ISD::CALLSEQ_END", SDT_I8008CallSeqEnd,
                               [SDNPHasChain, SDNPOptInGlue, SDNPOutGlue]>;

//===----------------------------------------------------------------------===//
// Operands
//===----------------------------------------------------------------------===//

// An absolute address known at link time: a global / external symbol /
// frame index plus a constant offset. Frame indices become
// `__frame_<fn> + off` in eliminateFrameIndex.
def memabs : Operand<i16> {
  let MIOperandInfo = (ops i16imm:$base, i16imm:$off);
  let PrintMethod = "printMemAbs";
}
def AbsAddr : ComplexPattern<iPTR, 2, "SelectAbs",
                             [i8008_wrapper, frameindex, add], []>;

def brtarget : Operand<OtherVT> {
  let PrintMethod = "printAddr16";
}

//===----------------------------------------------------------------------===//
// Pseudos
//===----------------------------------------------------------------------===//

let isPseudo = 1, hasSideEffects = 0 in {

let Defs = [A, F], isCodeGenOnly = 1 in {
  def ADJCALLSTACKDOWN : Pseudo<(outs), (ins i16imm:$amt1, i16imm:$amt2),
                                [(i8008_callseq_start timm:$amt1, timm:$amt2)]>;
  def ADJCALLSTACKUP   : Pseudo<(outs), (ins i16imm:$amt1, i16imm:$amt2),
                                [(i8008_callseq_end timm:$amt1, timm:$amt2)]>;
}

// Constants: LdI (one or two).
let isMoveImm = 1, isReMaterializable = 1, isAsCheapAsAMove = 1 in {
  def MOV8ri  : Pseudo<(outs GR8:$dst), (ins i8imm:$imm),
                       [(set GR8:$dst, (i8 imm:$imm))]>;
  def MOV16ri : Pseudo<(outs GR16:$dst), (ins i16imm:$imm),
                       [(set GR16:$dst, (i16 imm:$imm))]>;
}

// Address of a symbol / frame slot: lXi %lo(..); lYi %hi(..).
let isReMaterializable = 1, isAsCheapAsAMove = 1 in
def LEA16 : Pseudo<(outs GR16:$dst), (ins memabs:$addr),
                   [(set GR16:$dst, AbsAddr:$addr)]>;

// ALU, register and immediate forms. 8-bit: `la x; op y; ld a`.
multiclass ALU<SDNode node, bit commutes> {
  let Defs = [A, F] in {
    let isCommutable = commutes in {
      def NAME # "8rr"  : Pseudo<(outs GR8:$dst), (ins GR8:$a, GR8:$b),
                        [(set GR8:$dst, (node GR8:$a, GR8:$b))]>;
      def NAME # "16rr" : Pseudo<(outs GR16:$dst), (ins GR16:$a, GR16:$b),
                        [(set GR16:$dst, (node GR16:$a, GR16:$b))]>;
    }
    def NAME # "8ri"  : Pseudo<(outs GR8:$dst), (ins GR8:$a, i8imm:$k),
                      [(set GR8:$dst, (node GR8:$a, (i8 imm:$k)))]>;
    def NAME # "16ri" : Pseudo<(outs GR16:$dst), (ins GR16:$a, i16imm:$k),
                      [(set GR16:$dst, (node GR16:$a, (i16 imm:$k)))]>;
  }
}
defm ADD : ALU<add, 1>;
defm SUB : ALU<sub, 0>;
defm AND : ALU<and, 1>;
defm OR : ALU<or,  1>;
defm XOR : ALU<xor, 1>;

// k - x
let Defs = [A, F] in {
  def RSUB8ri  : Pseudo<(outs GR8:$dst), (ins GR8:$a, i8imm:$k),
                        [(set GR8:$dst, (sub (i8 imm:$k), GR8:$a))]>;
  def RSUB16ri : Pseudo<(outs GR16:$dst), (ins GR16:$a, i16imm:$k),
                        [(set GR16:$dst, (sub (i16 imm:$k), GR16:$a))]>;
}

// Shifts by a constant (the custom lowering keeps only the cheap ones).
let Defs = [A, F] in {
  def SHL8ri  : Pseudo<(outs GR8:$dst), (ins GR8:$a, i8imm:$k),
                       [(set GR8:$dst, (shl GR8:$a, (i8 imm:$k)))]>;
  def SRL8ri  : Pseudo<(outs GR8:$dst), (ins GR8:$a, i8imm:$k),
                       [(set GR8:$dst, (srl GR8:$a, (i8 imm:$k)))]>;
  def SRA8ri  : Pseudo<(outs GR8:$dst), (ins GR8:$a, i8imm:$k),
                       [(set GR8:$dst, (sra GR8:$a, (i8 imm:$k)))]>;
  def SHL16ri : Pseudo<(outs GR16:$dst), (ins GR16:$a, i8imm:$k),
                       [(set GR16:$dst, (shl GR16:$a, (i8 imm:$k)))]>;
  def SRL16ri : Pseudo<(outs GR16:$dst), (ins GR16:$a, i8imm:$k),
                       [(set GR16:$dst, (srl GR16:$a, (i8 imm:$k)))]>;
  def SRA16ri : Pseudo<(outs GR16:$dst), (ins GR16:$a, i8imm:$k),
                       [(set GR16:$dst, (sra GR16:$a, (i8 imm:$k)))]>;
  // i8 -> i16 sign extension: `la x; rlc; sba; l<hi>a`.
  def SEXT8 : Pseudo<(outs GR16:$dst), (ins GR8:$a),
                     [(set GR16:$dst, (sext GR8:$a))]>;
}

// Memory through a pointer pair: `lh<hi>; ll<lo>; ...m`.
let Defs = [A, F, H, L] in {
  let mayLoad = 1 in {
    def LD8  : Pseudo<(outs GR8:$dst),  (ins GR16:$p), [(set GR8:$dst, (load GR16:$p))]>;
    def LD16 : Pseudo<(outs GR16:$dst), (ins GR16:$p), [(set GR16:$dst, (load GR16:$p))]>;
  }
  let mayStore = 1 in {
    def ST8  : Pseudo<(outs), (ins GR8:$v, GR16:$p),  [(store GR8:$v, GR16:$p)]>;
    def ST8i : Pseudo<(outs), (ins i8imm:$v, GR16:$p), [(store (i8 imm:$v), GR16:$p)]>;
    def ST16 : Pseudo<(outs), (ins GR16:$v, GR16:$p), [(store GR16:$v, GR16:$p)]>;
  }
}

// Memory at a link-time address (globals, frame slots, spills):
// `lhi %hi(a); lli %lo(a); ...m`.
let Defs = [A, F, H, L], AddedComplexity = 10 in {
  let mayLoad = 1 in {
    def LDA8  : Pseudo<(outs GR8:$dst),  (ins memabs:$a), [(set GR8:$dst, (load AbsAddr:$a))]>;
    def LDA16 : Pseudo<(outs GR16:$dst), (ins memabs:$a), [(set GR16:$dst, (load AbsAddr:$a))]>;
  }
  let mayStore = 1 in {
    def STA8  : Pseudo<(outs), (ins GR8:$v, memabs:$a),   [(store GR8:$v, AbsAddr:$a)]>;
    def STA8i : Pseudo<(outs), (ins i8imm:$v, memabs:$a), [(store (i8 imm:$v), AbsAddr:$a)]>;
    def STA16 : Pseudo<(outs), (ins GR16:$v, memabs:$a),  [(store GR16:$v, AbsAddr:$a)]>;
  }
}

// Calls. The register mask (no saved registers) comes from LowerCall.
let isCall = 1, Defs = [A, F, H, L] in {
  def CALL    : Pseudo<(outs), (ins addr16:$dst, variable_ops),
                       [(i8008_call (i16 tglobaladdr:$dst))]>;
  def CALLind : Pseudo<(outs), (ins GR16:$p, variable_ops),
                       [(i8008_call GR16:$p)]>;
}

let isReturn = 1, isTerminator = 1, isBarrier = 1 in
def RETpseudo : Pseudo<(outs), (ins), [(i8008_ret)]>;

let isBranch = 1, isTerminator = 1, isBarrier = 1 in
def BR : Pseudo<(outs), (ins brtarget:$dst), [(br bb:$dst)]>;

// Compare and branch. $cc is an I8008CC code (I8008ISelLowering.h).
let isBranch = 1, isTerminator = 1, Defs = [A, F, H, L] in {
  def BRCC8rr  : Pseudo<(outs), (ins GR8:$a, GR8:$b, i8imm:$cc, brtarget:$dst), []>;
  def BRCC8ri  : Pseudo<(outs), (ins GR8:$a, i8imm:$k, i8imm:$cc, brtarget:$dst), []>;
  def BRCC16rr : Pseudo<(outs), (ins GR16:$a, GR16:$b, i8imm:$cc, brtarget:$dst), []>;
  def BRCC16ri : Pseudo<(outs), (ins GR16:$a, i16imm:$k, i8imm:$cc, brtarget:$dst), []>;
}

let usesCustomInserter = 1 in {
  def SELECT_c8_v8 : Pseudo<(outs GR8:$dst),
      (ins GR8:$a, GR8:$b, i8imm:$cc, GR8:$t, GR8:$f),
      [(set GR8:$dst, (i8008_selectcc GR8:$a, GR8:$b, GR8:$t, GR8:$f, timm:$cc))]>;
  def SELECT_c8_v16 : Pseudo<(outs GR16:$dst),
      (ins GR8:$a, GR8:$b, i8imm:$cc, GR16:$t, GR16:$f),
      [(set GR16:$dst, (i8008_selectcc GR8:$a, GR8:$b, GR16:$t, GR16:$f, timm:$cc))]>;
  def SELECT_c16_v8 : Pseudo<(outs GR8:$dst),
      (ins GR16:$a, GR16:$b, i8imm:$cc, GR8:$t, GR8:$f),
      [(set GR8:$dst, (i8008_selectcc GR16:$a, GR16:$b, GR8:$t, GR8:$f, timm:$cc))]>;
  def SELECT_c16_v16 : Pseudo<(outs GR16:$dst),
      (ins GR16:$a, GR16:$b, i8imm:$cc, GR16:$t, GR16:$f),
      [(set GR16:$dst, (i8008_selectcc GR16:$a, GR16:$b, GR16:$t, GR16:$f, timm:$cc))]>;
}

} // isPseudo

//===----------------------------------------------------------------------===//
// Patterns
//===----------------------------------------------------------------------===//

def : Pat<(i8008_call (i16 texternalsym:$dst)), (CALL texternalsym:$dst)>;

def : Pat<(i8008_brcc GR8:$a, (i8 imm:$k), timm:$cc, bb:$dst),
          (BRCC8ri GR8:$a, imm:$k, timm:$cc, brtarget:$dst)>;
def : Pat<(i8008_brcc GR8:$a, GR8:$b, timm:$cc, bb:$dst),
          (BRCC8rr GR8:$a, GR8:$b, timm:$cc, brtarget:$dst)>;
def : Pat<(i8008_brcc GR16:$a, (i16 imm:$k), timm:$cc, bb:$dst),
          (BRCC16ri GR16:$a, imm:$k, timm:$cc, brtarget:$dst)>;
def : Pat<(i8008_brcc GR16:$a, GR16:$b, timm:$cc, bb:$dst),
          (BRCC16rr GR16:$a, GR16:$b, timm:$cc, brtarget:$dst)>;

def : Pat<(i8 (trunc GR16:$x)), (EXTRACT_SUBREG GR16:$x, sub_lo)>;
def : Pat<(i16 (zext GR8:$x)),
          (REG_SEQUENCE GR16, GR8:$x, sub_lo, (MOV8ri 0), sub_hi)>;
def : Pat<(i16 (anyext GR8:$x)),
          (INSERT_SUBREG (i16 (IMPLICIT_DEF)), GR8:$x, sub_lo)>;
