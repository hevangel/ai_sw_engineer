//===-- I4004ISelDAGToDAG.cpp - I4004 DAG-to-DAG Instruction Selector ----===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
//
// Defines an instruction selector for the I4004 target. Most selection is
// handled by TableGen patterns (I4004InstrCodeGen.td); this pass covers the
// leftovers (FrameIndex, target constant materialization).
//
//===----------------------------------------------------------------------===//

#include "I4004.h"
#include "I4004Subtarget.h"
#include "I4004TargetMachine.h"
#include "llvm/CodeGen/SelectionDAGISel.h"
#include "llvm/Support/Debug.h"

using namespace llvm;

#define DEBUG_TYPE "i4004-isel"
#define PASS_NAME "I4004 DAG->DAG Pattern Instruction Selection"

#define GET_SDNODE_ENUM
#include "I4004GenSDNodeInfo.inc"

namespace {

class I4004DAGToDAGISel : public SelectionDAGISel {
  const I4004Subtarget *Subtarget = nullptr;

public:
  I4004DAGToDAGISel() = delete;
  explicit I4004DAGToDAGISel(I4004TargetMachine &TM, CodeGenOptLevel OptLevel)
      : SelectionDAGISel(TM, OptLevel) {}

  bool runOnMachineFunction(MachineFunction &MF) override {
    Subtarget = &MF.getSubtarget<I4004Subtarget>();
    return SelectionDAGISel::runOnMachineFunction(MF);
  }

  void Select(SDNode *N) override;

#include "I4004GenDAGISel.inc"
};

class I4004DAGToDAGISelLegacy : public SelectionDAGISelLegacy {
public:
  static char ID;
  explicit I4004DAGToDAGISelLegacy(I4004TargetMachine &TM,
                                   CodeGenOptLevel OptLevel)
      : SelectionDAGISelLegacy(
            ID, std::make_unique<I4004DAGToDAGISel>(TM, OptLevel)) {}
};

} // namespace

char I4004DAGToDAGISelLegacy::ID = 0;

INITIALIZE_PASS(I4004DAGToDAGISelLegacy, DEBUG_TYPE, PASS_NAME, false, false)

void I4004DAGToDAGISel::Select(SDNode *N) {
  SDLoc DL(N);

  if (N->isMachineOpcode()) {
    N->setNodeId(-1);
    return;
  }

  switch (N->getOpcode()) {
  case ISD::FrameIndex: {
    // Materialize a frame-index address into a DR16 pointer via LEA_FI; the
    // backbone's eliminateFrameIndex rewrites the slot to a RAM address.
    int FI = cast<FrameIndexSDNode>(N)->getIndex();
    SDValue TFI = CurDAG->getTargetFrameIndex(FI, MVT::i16);
    CurDAG->SelectNodeTo(N, I4004::LEA_FI, MVT::i16, TFI);
    return;
  }
  default:
    break;
  }

  SelectCode(N);
}

FunctionPass *llvm::createI4004ISelDag(I4004TargetMachine &TM,
                                       CodeGenOptLevel OptLevel) {
  return new I4004DAGToDAGISelLegacy(TM, OptLevel);
}
