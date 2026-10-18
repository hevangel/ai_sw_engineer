//===-- I4004InstrCodeGen.td - I4004 CodeGen patterns -----*- tablegen -*-===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Instruction-selection layer for the Intel 4004 (milestone 3b). The real MCS-4
// instructions in I4004InstrInfo.td are accumulator-centric and have implicit
// ACC/CY operands that the register allocator cannot reason about directly.
// So selection targets register-to-register PSEUDO-instructions over PR8 (the
// i8 pair class); a post-RA pass (I4004ExpandPseudos) lowers each pseudo into
// the real LD/ADD/XCH/... nibble sequence. This keeps the allocator honest and
// matches the brief's "pseudo-instructions expanded post-RA are fine" guidance.
//
//===----------------------------------------------------------------------===//

//===----------------------------------------------------------------------===//
// Target-specific DAG nodes.
//===----------------------------------------------------------------------===//

def SDT_I4004Ret   : SDTypeProfile<0, 0, []>;
def SDT_I4004Call  : SDTypeProfile<0, -1, [SDTCisVT<0, iPTR>]>;
def SDT_I4004Wrap  : SDTypeProfile<1, 1, [SDTCisSameAs<0, 1>, SDTCisPtrTy<0>]>;

def i4004_ret : SDNode<"I4004ISD::RET", SDT_I4004Ret,
                       [SDNPHasChain, SDNPOptInGlue, SDNPVariadic]>;
def i4004_call : SDNode<"I4004ISD::CALL", SDT_I4004Call,
                        [SDNPHasChain, SDNPOptInGlue, SDNPOutGlue,
                         SDNPVariadic]>;
def i4004_wrapper : SDNode<"I4004ISD::WRAPPER", SDT_I4004Wrap>;

// Call-frame setup/destroy chains (required even with a reserved call frame).
def SDT_I4004CallSeqStart : SDCallSeqStart<[SDTCisVT<0, i16>, SDTCisVT<1, i16>]>;
def SDT_I4004CallSeqEnd   : SDCallSeqEnd<[SDTCisVT<0, i16>, SDTCisVT<1, i16>]>;
def i4004_callseq_start : SDNode<"ISD::CALLSEQ_START", SDT_I4004CallSeqStart,
                                 [SDNPHasChain, SDNPOutGlue]>;
def i4004_callseq_end : SDNode<"ISD::CALLSEQ_END", SDT_I4004CallSeqEnd,
                               [SDNPHasChain, SDNPOptInGlue, SDNPOutGlue]>;

//===----------------------------------------------------------------------===//
// Pseudo-instructions (selected by ISel, expanded post-RA).
//===----------------------------------------------------------------------===//

let isPseudo = 1, hasSideEffects = 0 in {

// Call-frame bookkeeping. Erased by eliminateCallFramePseudoInstr.
let Defs = [ACC, CY], Uses = [P7], isCodeGenOnly = 1 in {
  def ADJCALLSTACKDOWN : Pseudo<(outs), (ins i16imm:$amt1, i16imm:$amt2),
                                [(i4004_callseq_start timm:$amt1, timm:$amt2)]>;
  def ADJCALLSTACKUP   : Pseudo<(outs), (ins i16imm:$amt1, i16imm:$amt2),
                                [(i4004_callseq_end timm:$amt1, timm:$amt2)]>;
}

// Materialize an 8-bit immediate into a pair (expands to FIM Pn, imm8).
let isMoveImm = 1, isReMaterializable = 1, isAsCheapAsAMove = 1 in
def MOVi8 : Pseudo<(outs PR8:$dst), (ins i8imm:$imm),
                   [(set PR8:$dst, (i8 imm:$imm))]>;

// Register copy between pairs (expands to two LD/XCH nibble moves). Used by
// copyPhysReg; also matched for plain i8 copies.
def COPYpr : Pseudo<(outs PR8:$dst), (ins PR8:$src), []>;

// 8-bit add/sub, result in $dst (expands to CLC;LD;ADD;XCH nibble sequences
// with carry propagation across the two nibbles).
let Defs = [ACC, CY] in {
  def ADDi8 : Pseudo<(outs PR8:$dst), (ins PR8:$lhs, PR8:$rhs),
                     [(set PR8:$dst, (i8 (add PR8:$lhs, PR8:$rhs)))]>;
  def SUBi8 : Pseudo<(outs PR8:$dst), (ins PR8:$lhs, PR8:$rhs),
                     [(set PR8:$dst, (i8 (sub PR8:$lhs, PR8:$rhs)))]>;
}

// Return (expands to BBL 0 on the hardware stack).
let isReturn = 1, isTerminator = 1, isBarrier = 1 in
def RETpseudo : Pseudo<(outs), (ins), [(i4004_ret)]>;

} // isPseudo

//===----------------------------------------------------------------------===//
// Extra selection patterns.
//===----------------------------------------------------------------------===//

// A small immediate (0..15) can also be materialized; MOVi8 covers 0..255 and
// is sufficient for the backbone, so no separate i4 path yet.
