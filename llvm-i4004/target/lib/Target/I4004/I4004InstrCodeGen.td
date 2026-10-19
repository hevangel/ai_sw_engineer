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

// Compare-and-branch: (brcc lhs, rhs, ccimm, dest). Custom-lowered from
// ISD::BR_CC. ccimm is an i8 immediate carrying the condition code (see
// I4004CC below). The expander does a SUB chain over lhs,rhs and a JCN keyed on
// the condition.
def SDT_I4004BrCC : SDTypeProfile<0, 4,
    [SDTCisSameAs<0, 1>, SDTCisVT<2, i8>, SDTCisVT<3, OtherVT>]>;
def i4004_brcc : SDNode<"I4004ISD::BRCC", SDT_I4004BrCC,
                        [SDNPHasChain]>;

// select_cc: (selectcc lhs, rhs, tval, fval, ccimm). lhs/rhs are the compare
// operands (i8 or i16, same type); tval/fval match the result type. The custom
// inserter picks BRCCi8/BRCCi16 from the compare width.
def SDT_I4004SelCC : SDTypeProfile<1, 5,
    [SDTCisSameAs<0, 3>, SDTCisSameAs<3, 4>, SDTCisSameAs<1, 2>,
     SDTCisVT<5, i8>]>;
def i4004_selectcc : SDNode<"I4004ISD::SELECT_CC", SDT_I4004SelCC>;

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

// Materialize a 16-bit immediate into a DR16 (expands to FIM into both pairs).
let isMoveImm = 1, isReMaterializable = 1, isAsCheapAsAMove = 1 in
def MOVi16 : Pseudo<(outs DR16:$dst), (ins i16imm:$imm),
                    [(set DR16:$dst, (i16 imm:$imm))]>;

// Register copy between pairs (expands to two LD/XCH nibble moves). Used by
// copyPhysReg; also matched for plain i8 copies.
def COPYpr : Pseudo<(outs PR8:$dst), (ins PR8:$src), []>;

// Spill / reload to the 4002-RAM spill region (abi.md sec.2.1). The frame index
// is resolved to a concrete RAM char offset by the post-RA expander.
let mayStore = 1, Defs = [ACC, CY, P0] in {
  def SPILL8  : Pseudo<(outs), (ins PR8:$src, i16imm:$fi), []>;
  def SPILL16 : Pseudo<(outs), (ins DR16:$src, i16imm:$fi), []>;
}
let mayLoad = 1, Defs = [ACC, CY, P0] in {
  def RELOAD8  : Pseudo<(outs PR8:$dst), (ins i16imm:$fi), []>;
  def RELOAD16 : Pseudo<(outs DR16:$dst), (ins i16imm:$fi), []>;
}

// Materialize the RAM address of a stack slot into a DR16 pointer.
let isPseudo = 1, hasSideEffects = 0 in
def LEA_FI : Pseudo<(outs DR16:$dst), (ins i16imm:$fi), []>;

// Materialize a global/symbol address into a DR16 pointer (I4004ISD::WRAPPER).
// The low byte carries an R_I4004_IMM8 relocation to the symbol; the high byte
// is 0 (backbone limitation: globals must lie in the low 256 ROM/RAM bytes --
// documented; a %hi relocation + linker lands with milestone 5).
let isPseudo = 1, hasSideEffects = 0 in
def LEA_GA : Pseudo<(outs DR16:$dst), (ins i16imm:$sym), []>;

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

// Call to a known address -> JMS (hardware stack, depth <= 3; the software
// return stack for deeper graphs is a later step). The callee is an addr12
// symbol. Variadic to carry the argument-register uses/defs the DAG attaches.
//
// Defs cover the caller-clobbered set AND every register that can hold a
// RETURN value. RetCC_I4004 returns an i16/aggregate in D1 (P2:P3) then D2
// (P4:P5) -- a Rust `{ i16, ptr }` return occupies both -- so P4:P5 must be
// listed as defined by the call, or LiveIntervals rejects the `COPY $d2` that
// reads the second return slot ("Using an undefined physical register $d2").
// Consequently D1 and D2 are the two caller-saved i16 arg/return pairs; only
// P6 remains callee-saved (abi.md sec.4.3).
let isCall = 1, Uses = [P7], Defs = [ACC, CY, P0, P1, P2, P3, P4, P5] in {
  def CALL : Pseudo<(outs), (ins addr12:$dst, variable_ops),
                    [(i4004_call (i16 tglobaladdr:$dst))]>;
}

} // isPseudo

// External-symbol callee (libcalls like __orhi3) uses the same JMS path.
def : Pat<(i4004_call (i16 texternalsym:$dst)), (CALL texternalsym:$dst)>;

// Global / external-symbol address -> LEA_GA (I4004ISD::WRAPPER).
def : Pat<(i4004_wrapper (i16 tglobaladdr:$sym)), (LEA_GA tglobaladdr:$sym)>;
def : Pat<(i4004_wrapper (i16 texternalsym:$sym)), (LEA_GA texternalsym:$sym)>;

//===----------------------------------------------------------------------===//
// Extend / truncate between i8 and i16.
//===----------------------------------------------------------------------===//

// trunc i16 -> i8: take the low sub-pair.
def : Pat<(i8 (trunc DR16:$x)), (EXTRACT_SUBREG DR16:$x, sub_pair_lo)>;

// zext/anyext i8 -> i16: low sub-pair = x, high sub-pair = 0. A pseudo builds
// the DR16 (post-RA: a COPY into the low pair + FIM 0 into the high pair).
let isPseudo = 1, hasSideEffects = 0 in {
  def ZEXT8TO16  : Pseudo<(outs DR16:$dst), (ins PR8:$x),
                          [(set DR16:$dst, (i16 (zext PR8:$x)))]>;
  def ANYEXT8TO16 : Pseudo<(outs DR16:$dst), (ins PR8:$x),
                           [(set DR16:$dst, (i16 (anyext PR8:$x)))]>;
}

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

// Compare-and-branch pseudos (selected from I4004ISD::BRCC). $cc is the
// condition-code immediate; the expander does a SUB chain and a JCN.
let isPseudo = 1, hasSideEffects = 0, isBranch = 1, isTerminator = 1,
    Defs = [ACC, CY] in {
  def BRCCi8  : Pseudo<(outs), (ins PR8:$lhs, PR8:$rhs, i8imm:$cc,
                                    brtarget:$dst), []>;
  def BRCCi16 : Pseudo<(outs), (ins DR16:$lhs, DR16:$rhs, i8imm:$cc,
                                    brtarget:$dst), []>;
}

def : Pat<(i4004_brcc PR8:$lhs, PR8:$rhs, timm:$cc, bb:$dst),
          (BRCCi8 PR8:$lhs, PR8:$rhs, timm:$cc, brtarget:$dst)>;
def : Pat<(i4004_brcc DR16:$lhs, DR16:$rhs, timm:$cc, bb:$dst),
          (BRCCi16 DR16:$lhs, DR16:$rhs, timm:$cc, brtarget:$dst)>;

// SELECT pseudos: expanded to a compare + branch CFG diamond by
// I4004TargetLowering::EmitInstrWithCustomInserter. $lhs/$rhs feed the compare,
// $cc the condition; $t is chosen when the condition holds, else $f.
let usesCustomInserter = 1 in {
  // Naming: SELECT_c<cmpWidth>_v<valWidth>.
  def SELECT_c8_v8   : Pseudo<(outs PR8:$dst),
                         (ins PR8:$lhs, PR8:$rhs, i8imm:$cc, PR8:$t, PR8:$f),
                         [(set PR8:$dst,
                           (i4004_selectcc PR8:$lhs, PR8:$rhs, PR8:$t, PR8:$f,
                                           timm:$cc))]>;
  def SELECT_c8_v16  : Pseudo<(outs DR16:$dst),
                         (ins PR8:$lhs, PR8:$rhs, i8imm:$cc, DR16:$t, DR16:$f),
                         [(set DR16:$dst,
                           (i4004_selectcc PR8:$lhs, PR8:$rhs, DR16:$t, DR16:$f,
                                           timm:$cc))]>;
  def SELECT_c16_v8  : Pseudo<(outs PR8:$dst),
                         (ins DR16:$lhs, DR16:$rhs, i8imm:$cc, PR8:$t, PR8:$f),
                         [(set PR8:$dst,
                           (i4004_selectcc DR16:$lhs, DR16:$rhs, PR8:$t, PR8:$f,
                                           timm:$cc))]>;
  def SELECT_c16_v16 : Pseudo<(outs DR16:$dst),
                         (ins DR16:$lhs, DR16:$rhs, i8imm:$cc, DR16:$t, DR16:$f),
                         [(set DR16:$dst,
                           (i4004_selectcc DR16:$lhs, DR16:$rhs, DR16:$t,
                                           DR16:$f, timm:$cc))]>;
}

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

// i16 RAM load/store: four 4002 chars (low byte first). Pointer in DR16.
let isPseudo = 1, mayLoad = 1, hasSideEffects = 0, Defs = [ACC, CY, P0] in
def LDRAMi16 : Pseudo<(outs DR16:$dst), (ins DR16:$ptr),
                      [(set DR16:$dst, (i16 (load DR16:$ptr)))]>;

let isPseudo = 1, mayStore = 1, hasSideEffects = 0, Defs = [ACC, CY, P0] in
def STRAMi16 : Pseudo<(outs), (ins DR16:$val, DR16:$ptr),
                      [(store (i16 DR16:$val), DR16:$ptr)]>;
