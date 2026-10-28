//===-- I8086ISelDAGToDAG.cpp - I8086 DAG-to-DAG Instruction Selector -----===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Instruction selector for the I8086 target. Most selection is TableGen
// patterns (I8086InstrCodeGen.td); this file adds the memory-operand matcher
// and the instructions that use fixed registers (cbw, shifts by CL).
//
//===----------------------------------------------------------------------===//

#include "I8086.h"
#include "I8086Subtarget.h"
#include "I8086TargetMachine.h"
#include "llvm/CodeGen/SelectionDAGISel.h"
#include "llvm/Support/Debug.h"

using namespace llvm;

#define DEBUG_TYPE "i8086-isel"
#define PASS_NAME "I8086 DAG->DAG Pattern Instruction Selection"

#define GET_SDNODE_ENUM
#include "I8086GenSDNodeInfo.inc"

namespace {

/// A decomposed address: [Base + Index + Disp] with at most one of a frame
/// index and a symbol. The register-less base is Base == empty.
struct AddrMode {
  SDValue Base, Index;
  bool HasFI = false;
  int FI = 0;
  SDValue Sym; // a TargetGlobalAddress / TargetExternalSymbol / TargetBlockAddress
  int64_t Disp = 0;
};

class I8086DAGToDAGISel : public SelectionDAGISel {
public:
  I8086DAGToDAGISel() = delete;
  explicit I8086DAGToDAGISel(I8086TargetMachine &TM, CodeGenOptLevel OptLevel)
      : SelectionDAGISel(TM, OptLevel) {}

  void Select(SDNode *N) override;

  bool SelectAddr(SDValue N, SDValue &Base, SDValue &Index, SDValue &Disp,
                  SDValue &Seg);

private:
  bool matchAddr(SDValue N, AddrMode &AM, unsigned Depth);
  SDValue constrain(SDValue V, unsigned RegClassID, const SDLoc &DL);
  void selectCBW(SDNode *N);
  bool selectShift(SDNode *N);
  void selectMul(SDNode *N);
  void selectDivRem(SDNode *N);

#include "I8086GenDAGISel.inc"
};

class I8086DAGToDAGISelLegacy : public SelectionDAGISelLegacy {
public:
  static char ID;
  explicit I8086DAGToDAGISelLegacy(I8086TargetMachine &TM,
                                   CodeGenOptLevel OptLevel)
      : SelectionDAGISelLegacy(
            ID, std::make_unique<I8086DAGToDAGISel>(TM, OptLevel)) {}
};

} // namespace

char I8086DAGToDAGISelLegacy::ID = 0;

INITIALIZE_PASS(I8086DAGToDAGISelLegacy, DEBUG_TYPE, PASS_NAME, false, false)

//===----------------------------------------------------------------------===//
// Addresses
//===----------------------------------------------------------------------===//

bool I8086DAGToDAGISel::matchAddr(SDValue N, AddrMode &AM, unsigned Depth) {
  if (Depth < 4) {
    switch (N.getOpcode()) {
    case ISD::Constant:
      AM.Disp += cast<ConstantSDNode>(N)->getSExtValue();
      return true;
    case I8086ISD::WRAPPER: {
      SDValue T = N.getOperand(0);
      if (AM.Sym.getNode())
        return false;
      AM.Sym = T;
      return true;
    }
    case ISD::FrameIndex:
      if (AM.HasFI || AM.Base.getNode())
        return false;
      AM.HasFI = true;
      AM.FI = cast<FrameIndexSDNode>(N)->getIndex();
      return true;
    case ISD::ADD: {
      AddrMode Saved = AM;
      if (matchAddr(N.getOperand(0), AM, Depth + 1) &&
          matchAddr(N.getOperand(1), AM, Depth + 1))
        return true;
      AM = Saved;
      if (matchAddr(N.getOperand(1), AM, Depth + 1) &&
          matchAddr(N.getOperand(0), AM, Depth + 1))
        return true;
      AM = Saved;
      break; // the whole sum is one register
    }
    default:
      break;
    }
  }
  // A register: the base first, then the index.
  if (!AM.Base.getNode() && !AM.HasFI) {
    AM.Base = N;
    return true;
  }
  if (!AM.Index.getNode()) {
    AM.Index = N;
    return true;
  }
  return false;
}

SDValue I8086DAGToDAGISel::constrain(SDValue V, unsigned RegClassID,
                                     const SDLoc &DL) {
  SDValue RC = CurDAG->getTargetConstant(RegClassID, DL, MVT::i32);
  return SDValue(CurDAG->getMachineNode(TargetOpcode::COPY_TO_REGCLASS, DL,
                                        MVT::i16, V, RC),
                 0);
}

bool I8086DAGToDAGISel::SelectAddr(SDValue N, SDValue &Base, SDValue &Index,
                                   SDValue &Disp, SDValue &Seg) {
  SDLoc DL(N);
  AddrMode AM;
  if (!matchAddr(N, AM, 0) || (AM.Sym.getNode() && AM.HasFI)) {
    AM = AddrMode();
    AM.Base = N; // the pointer is just a register
  }
  // A displacement is 16 bits wide and wraps; a symbol takes the offset only
  // if it can carry one.
  if (!isInt<32>(AM.Disp)) {
    AM = AddrMode();
    AM.Base = N;
  }
  if (AM.Sym.getNode() && AM.Disp != 0 && !isa<GlobalAddressSDNode>(AM.Sym)) {
    AM = AddrMode();
    AM.Base = N;
  }

  SDValue NoReg = CurDAG->getRegister(0, MVT::i16);
  bool Pair = AM.Index.getNode() && (AM.Base.getNode() || AM.HasFI);
  if (AM.HasFI)
    Base = CurDAG->getTargetFrameIndex(AM.FI, MVT::i16);
  else if (AM.Base.getNode())
    Base = constrain(AM.Base,
                     Pair ? I8086::GR16_BASERegClassID : I8086::GR16_PTRRegClassID, DL);
  else
    Base = NoReg;
  Index = AM.Index.getNode()
              ? (Pair ? constrain(AM.Index, I8086::GR16_INDEXRegClassID, DL)
                      : AM.Index)
              : NoReg;
  if (AM.Sym.getNode()) {
    if (auto *G = dyn_cast<GlobalAddressSDNode>(AM.Sym))
      Disp = CurDAG->getTargetGlobalAddress(G->getGlobal(), DL, MVT::i16,
                                            G->getOffset() + AM.Disp);
    else
      Disp = AM.Sym;
  } else {
    // Addresses wrap inside the 64 KiB segment: the displacement is 16 bits.
    Disp = CurDAG->getSignedTargetConstant(int16_t(AM.Disp), DL, MVT::i16);
  }
  Seg = NoReg;
  return true;
}

//===----------------------------------------------------------------------===//
// Fixed-register instructions
//===----------------------------------------------------------------------===//

// i8 -> i16 sign extension: the byte is moved to AL, `cbw`, and AX is read.
void I8086DAGToDAGISel::selectCBW(SDNode *N) {
  SDLoc DL(N);
  SDValue Chain = CurDAG->getCopyToReg(CurDAG->getEntryNode(), DL, I8086::AL,
                                       N->getOperand(0), SDValue());
  SDValue Glue = Chain.getValue(1);
  SDNode *Cbw = CurDAG->getMachineNode(I8086::P_CBW, DL, MVT::Other,
                                       MVT::Glue, Chain, Glue);
  SDValue Res = CurDAG->getCopyFromReg(SDValue(Cbw, 0), DL, I8086::AX,
                                       MVT::i16, SDValue(Cbw, 1));
  ReplaceUses(SDValue(N, 0), Res);
  CurDAG->RemoveDeadNode(N);
}

// Shifts by a count other than 1: a few single shifts, else the count in CL.
bool I8086DAGToDAGISel::selectShift(SDNode *N) {
  EVT VT = N->getValueType(0);
  if (VT != MVT::i8 && VT != MVT::i16)
    return false;
  SDValue Amt = N->getOperand(1);
  auto *C = dyn_cast<ConstantSDNode>(Amt);
  if (C && C->getZExtValue() == 1)
    return false; // a pattern
  if (C && C->getZExtValue() == 8 && VT == MVT::i16 &&
      N->getOpcode() != ISD::SRA)
    return false; // a byte move (patterns)
  bool Wide = VT == MVT::i16;
  unsigned One, ByCL;
  switch (N->getOpcode()) {
  case ISD::SHL:
    One = Wide ? I8086::P_SHL16r1 : I8086::P_SHL8r1;
    ByCL = Wide ? I8086::P_SHL16rCL : I8086::P_SHL8rCL;
    break;
  case ISD::SRL:
    One = Wide ? I8086::P_SHR16r1 : I8086::P_SHR8r1;
    ByCL = Wide ? I8086::P_SHR16rCL : I8086::P_SHR8rCL;
    break;
  default:
    One = Wide ? I8086::P_SAR16r1 : I8086::P_SAR8r1;
    ByCL = Wide ? I8086::P_SAR16rCL : I8086::P_SAR8rCL;
    break;
  }
  SDLoc DL(N);
  SDValue V = N->getOperand(0);
  if (C && C->getZExtValue() == 0) {
    ReplaceUses(SDValue(N, 0), V);
    CurDAG->RemoveDeadNode(N);
    return true;
  }
  if (C && C->getZExtValue() == 2) {
    for (int I = 0; I < 2; ++I)
      V = SDValue(CurDAG->getMachineNode(One, DL, VT, V), 0);
    ReplaceUses(SDValue(N, 0), V);
    CurDAG->RemoveDeadNode(N);
    return true;
  }
  if (C)
    Amt = SDValue(CurDAG->getMachineNode(
                      I8086::P_MOV8ri, DL, MVT::i8,
                      CurDAG->getTargetConstant(C->getZExtValue(), DL, MVT::i8)),
                  0);
  SDValue Chain = CurDAG->getCopyToReg(CurDAG->getEntryNode(), DL, I8086::CL,
                                       Amt, SDValue());
  SDNode *Sh = CurDAG->getMachineNode(ByCL, DL, VT, V, Chain.getValue(1));
  ReplaceUses(SDValue(N, 0), SDValue(Sh, 0));
  CurDAG->RemoveDeadNode(N);
  return true;
}

// 16-bit multiply: AX times the operand, the product in DX:AX. mul and imul
// give the same low half; the full product is signed or unsigned as the node
// says.
void I8086DAGToDAGISel::selectMul(SDNode *N) {
  SDLoc DL(N);
  unsigned Opc = N->getOpcode();
  SDValue Chain = CurDAG->getCopyToReg(CurDAG->getEntryNode(), DL, I8086::AX,
                                       N->getOperand(0), SDValue());
  SDNode *M = CurDAG->getMachineNode(
      Opc == ISD::SMUL_LOHI ? I8086::P_IMUL16r : I8086::P_MUL16r, DL,
      MVT::Glue, N->getOperand(1), Chain.getValue(1));
  SDValue Lo = CurDAG->getCopyFromReg(CurDAG->getEntryNode(), DL, I8086::AX,
                                      MVT::i16, SDValue(M, 0));
  ReplaceUses(SDValue(N, 0), Lo);
  if (Opc != ISD::MUL) {
    SDValue Hi = CurDAG->getCopyFromReg(Lo.getValue(1), DL, I8086::DX,
                                        MVT::i16, Lo.getValue(2));
    ReplaceUses(SDValue(N, 1), Hi);
  }
  CurDAG->RemoveDeadNode(N);
}

// 16-bit divide: DX:AX divided by the operand, quotient in AX, remainder in
// DX. The dividend is extended into DX by `cwd` (signed) or by zero.
void I8086DAGToDAGISel::selectDivRem(SDNode *N) {
  SDLoc DL(N);
  bool Signed = N->getOpcode() == ISD::SDIVREM;
  SDValue Chain = CurDAG->getCopyToReg(CurDAG->getEntryNode(), DL, I8086::AX,
                                       N->getOperand(0), SDValue());
  SDValue Glue = Chain.getValue(1);
  if (Signed) {
    Glue = SDValue(CurDAG->getMachineNode(I8086::P_CWD, DL, MVT::Glue, Glue), 0);
  } else {
    SDValue Zero = SDValue(CurDAG->getMachineNode(
                               I8086::P_MOV16ri, DL, MVT::i16,
                               CurDAG->getTargetConstant(0, DL, MVT::i16)),
                           0);
    Chain = CurDAG->getCopyToReg(Chain, DL, I8086::DX, Zero, Glue);
    Glue = Chain.getValue(1);
  }
  SDNode *D = CurDAG->getMachineNode(Signed ? I8086::P_IDIV16r : I8086::P_DIV16r,
                                     DL, MVT::Glue, N->getOperand(1), Glue);
  SDValue Q = CurDAG->getCopyFromReg(CurDAG->getEntryNode(), DL, I8086::AX,
                                     MVT::i16, SDValue(D, 0));
  SDValue R = CurDAG->getCopyFromReg(Q.getValue(1), DL, I8086::DX, MVT::i16,
                                     Q.getValue(2));
  ReplaceUses(SDValue(N, 0), Q);
  ReplaceUses(SDValue(N, 1), R);
  CurDAG->RemoveDeadNode(N);
}

void I8086DAGToDAGISel::Select(SDNode *N) {
  if (N->isMachineOpcode()) {
    N->setNodeId(-1);
    return;
  }
  switch (N->getOpcode()) {
  case ISD::FrameIndex: {
    SDLoc DL(N);
    int FI = cast<FrameIndexSDNode>(N)->getIndex();
    SDValue NoReg = CurDAG->getRegister(0, MVT::i16);
    SDValue Ops[] = {CurDAG->getTargetFrameIndex(FI, MVT::i16), NoReg,
                     CurDAG->getTargetConstant(0, DL, MVT::i16), NoReg};
    CurDAG->SelectNodeTo(N, I8086::P_LEA16r, MVT::i16, Ops);
    return;
  }
  case ISD::MUL:
  case ISD::UMUL_LOHI:
  case ISD::SMUL_LOHI:
    if (N->getValueType(0) == MVT::i16) {
      selectMul(N);
      return;
    }
    break;
  case ISD::UDIVREM:
  case ISD::SDIVREM:
    if (N->getValueType(0) == MVT::i16) {
      selectDivRem(N);
      return;
    }
    break;
  case ISD::SIGN_EXTEND:
    if (N->getValueType(0) == MVT::i16 &&
        N->getOperand(0).getValueType() == MVT::i8) {
      selectCBW(N);
      return;
    }
    break;
  case ISD::SHL:
  case ISD::SRL:
  case ISD::SRA:
    if (selectShift(N))
      return;
    break;
  default:
    break;
  }
  SelectCode(N);
}

FunctionPass *llvm::createI8086ISelDag(I8086TargetMachine &TM,
                                       CodeGenOptLevel OptLevel) {
  return new I8086DAGToDAGISelLegacy(TM, OptLevel);
}
