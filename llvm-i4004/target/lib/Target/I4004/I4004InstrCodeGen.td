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

// 16-bit add/sub over a DR16 (four nibbles, carry chained low->high). Lowered
// post-RA into the LD/ADD/XCH nibble sequence across both sub-pairs.
let Defs = [ACC, CY] in {
  def ADDi16 : Pseudo<(outs DR16:$dst), (ins DR16:$lhs, DR16:$rhs),
                      [(set DR16:$dst, (i16 (add DR16:$lhs, DR16:$rhs)))]>;
  def SUBi16 : Pseudo<(outs DR16:$dst), (ins DR16:$lhs, DR16:$rhs),
                      [(set DR16:$dst, (i16 (sub DR16:$lhs, DR16:$rhs)))]>;
}

// Return (expands to BBL 0 on the hardware stack).
let isReturn = 1, isTerminator = 1, isBarrier = 1 in
def RETpseudo : Pseudo<(outs), (ins), [(i4004_ret)]>;

} // isPseudo

//===----------------------------------------------------------------------===//
// Control flow (milestone 3b: unconditional + zero/nonzero conditional).
//===----------------------------------------------------------------------===//

// A basic-block branch target. Printed as a 12-bit absolute address; lowered to
// JUN (unconditional) or JCN (conditional) by the expander, with the MC layer's
// addr12/addr8 relocations resolving the label.
def brtarget : Operand<OtherVT> {
  let PrintMethod = "printAddr12";
}

// Unconditional branch -> JUN target.
let isPseudo = 1, hasSideEffects = 0, isBranch = 1, isTerminator = 1,
    isBarrier = 1 in
def BR : Pseudo<(outs), (ins brtarget:$dst), [(br bb:$dst)]>;

// Conditional branch on an i8 being zero / nonzero. The 4004 JCN tests ACC==0
// (Z). The expander loads the operand's nibbles into ACC and emits JCN Z / NZ.
let isPseudo = 1, hasSideEffects = 0, isBranch = 1, isTerminator = 1,
    Defs = [ACC, CY], Uses = [ACC] in {
  def BRZ  : Pseudo<(outs), (ins PR8:$val, brtarget:$dst), []>;
  def BRNZ : Pseudo<(outs), (ins PR8:$val, brtarget:$dst), []>;
}

// brcond selection (BRZ/BRNZ on a boolean i8) will be added together with the
// SETCC lowering step; for now only the unconditional BR carries a pattern.
// BRZ/BRNZ are defined above so the expander and a later custom BR_CC lowering
// can emit them, but no isel Pat references them yet (that needs working
// setcc, a documented later milestone).

//===----------------------------------------------------------------------===//
// Extra selection patterns.
//===----------------------------------------------------------------------===//

// addrspace-0 (4002 RAM) i8 load/store. The pointer is an i16 in a DR16
// double-pair; its low sub-pair holds the SRC address byte [chip:2][reg:2]
// [char:4] (abi.md sec.1.3). The expander emits SRC <lo-pair>; RDM/WRM for the
// low nibble, advances the char, and RDM/WRM for the high nibble (two 4002
// cells per byte, low first -- abi.md sec.1.2).
let isPseudo = 1, mayLoad = 1, hasSideEffects = 0, Defs = [ACC, CY] in
def LDRAMi8 : Pseudo<(outs PR8:$dst), (ins DR16:$ptr),
                     [(set PR8:$dst, (i8 (load DR16:$ptr)))]>;

let isPseudo = 1, mayStore = 1, hasSideEffects = 0, Defs = [ACC, CY] in
def STRAMi8 : Pseudo<(outs), (ins PR8:$val, DR16:$ptr),
                     [(store (i8 PR8:$val), DR16:$ptr)]>;
