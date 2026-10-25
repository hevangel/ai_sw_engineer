//===-- I8008ISelDAGToDAG.cpp - I8008 DAG-to-DAG Instruction Selector ----===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Defines an instruction selector for the I8008 target. Most selection is
// handled by TableGen patterns (I8008InstrCodeGen.td); this pass covers the
// leftovers: FrameIndex and the link-time-address operand (SelectAbs).
//
//===----------------------------------------------------------------------===//

#include "I8008.h"
#include "I8008Subtarget.h"
#include "I8008TargetMachine.h"
#include "llvm/CodeGen/SelectionDAGISel.h"
#include "llvm/Support/Debug.h"

using namespace llvm;

#define DEBUG_TYPE "i8008-isel"
#define PASS_NAME "I8008 DAG->DAG Pattern Instruction Selection"

#define GET_SDNODE_ENUM
#include "I8008GenSDNodeInfo.inc"

namespace {

class I8008DAGToDAGISel : public SelectionDAGISel {
  const I8008Subtarget *Subtarget = nullptr;

public:
  I8008DAGToDAGISel() = delete;
  explicit I8008DAGToDAGISel(I8008TargetMachine &TM, CodeGenOptLevel OptLevel)
      : SelectionDAGISel(TM, OptLevel) {}

  bool runOnMachineFunction(MachineFunction &MF) override {
    Subtarget = &MF.getSubtarget<I8008Subtarget>();
    return SelectionDAGISel::runOnMachineFunction(MF);
  }

  void Select(SDNode *N) override;

  /// A link-time address: (wrapper sym), a frame index, or either plus a
  /// constant. Base = the symbol / TargetFrameIndex, Off = the constant.
  bool SelectAbs(SDValue Addr, SDValue &Base, SDValue &Off);

#include "I8008GenDAGISel.inc"
};

class I8008DAGToDAGISelLegacy : public SelectionDAGISelLegacy {
public:
  static char ID;
  explicit I8008DAGToDAGISelLegacy(I8008TargetMachine &TM,
                                   CodeGenOptLevel OptLevel)
      : SelectionDAGISelLegacy(
            ID, std::make_unique<I8008DAGToDAGISel>(TM, OptLevel)) {}
};

} // namespace

char I8008DAGToDAGISelLegacy::ID = 0;

INITIALIZE_PASS(I8008DAGToDAGISelLegacy, DEBUG_TYPE, PASS_NAME, false, false)

bool I8008DAGToDAGISel::SelectAbs(SDValue Addr, SDValue &Base, SDValue &Off) {
  SDLoc DL(Addr);
  int64_t K = 0;
  if (Addr.getOpcode() == ISD::ADD) {
    auto *C = dyn_cast<ConstantSDNode>(Addr.getOperand(1));
    if (!C)
      return false;
    K = C->getSExtValue();
    Addr = Addr.getOperand(0);
  }
  if (auto *FI = dyn_cast<FrameIndexSDNode>(Addr)) {
    Base = CurDAG->getTargetFrameIndex(FI->getIndex(), MVT::i16);
    Off = CurDAG->getTargetConstant(K, DL, MVT::i16);
    return true;
  }
  if (Addr.getOpcode() != I8008ISD::WRAPPER)
    return false;
  SDValue T = Addr.getOperand(0);
  if (auto *G = dyn_cast<GlobalAddressSDNode>(T))
    Base = CurDAG->getTargetGlobalAddress(G->getGlobal(), DL, MVT::i16,
                                          G->getOffset() + K);
  else if (K == 0)
    Base = T; // external symbol / block address
  else
    return false;
  Off = CurDAG->getTargetConstant(0, DL, MVT::i16);
  return true;
}

void I8008DAGToDAGISel::Select(SDNode *N) {
  if (N->isMachineOpcode()) {
    N->setNodeId(-1);
    return;
  }
  if (N->getOpcode() == ISD::FrameIndex) {
    SDLoc DL(N);
    int FI = cast<FrameIndexSDNode>(N)->getIndex();
    SDValue Ops[] = {CurDAG->getTargetFrameIndex(FI, MVT::i16),
                     CurDAG->getTargetConstant(0, DL, MVT::i16)};
    CurDAG->SelectNodeTo(N, I8008::LEA16, MVT::i16, Ops);
    return;
  }
  SelectCode(N);
}

FunctionPass *llvm::createI8008ISelDag(I8008TargetMachine &TM,
                                       CodeGenOptLevel OptLevel) {
  return new I8008DAGToDAGISelLegacy(TM, OptLevel);
}
